use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use comfy_table::{Cell, Table};
use rusqlite::Connection;

use crate::db::get_connection;
use crate::error::{NigelError, Result};
use crate::fmt::money;
use crate::invoicing::clients::get_client;
use crate::invoicing::import_invoiceshelf::import as import_invoiceshelf;
use crate::invoicing::invoices::{
    create_invoice, ensure_not_void, ensure_voidable, get_invoice, get_invoice_by_number, is_void,
    line_items, list_invoices, paid_amount, payment_amount, record_payment, update_invoice,
    InvoiceListRow, InvoiceUpdate, NewLineItem,
};
use crate::invoicing::mailgun::{
    from_address_domain_warning, validate_bare_address, validate_header_value, EmailEnvelope,
    MailgunClient,
};
use crate::invoicing::r2::{public_base_url_warning, validate_public_base_url, R2Publisher};
use crate::invoicing::render::{render_invoice, RenderedInvoice};
use crate::invoicing::render_html::{load_template, template_path, Branding, DEFAULT_TEMPLATE};
use crate::invoicing::republish::republish_invoice;
use crate::invoicing::send::send_invoice;
use crate::invoicing::stripe::StripeClient;
use crate::invoicing::sync::sync_all_report;
use crate::invoicing::void::void_invoice_with_teardown;
use crate::models::{Client, Invoice, InvoiceLineItem};
use crate::settings::{get_data_dir, invoicing_config, InvoicingConfig};

fn parse_item(s: &str) -> Result<NewLineItem> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return Err(NigelError::Other(format!(
            "bad --item '{s}', want desc:qty:unit"
        )));
    }
    Ok(NewLineItem {
        description: parts[0].to_string(),
        quantity: parts[1].parse().map_err(|_| {
            NigelError::Other(format!("bad quantity '{}' in --item '{s}'", parts[1]))
        })?,
        unit_amount: parts[2].parse().map_err(|_| {
            NigelError::Other(format!("bad unit amount '{}' in --item '{s}'", parts[2]))
        })?,
    })
}

fn parse_items(items: &[String]) -> Result<Vec<NewLineItem>> {
    if items.is_empty() {
        return Err(NigelError::Other(
            "an invoice needs at least one --item \"desc:qty:unit\"".into(),
        ));
    }
    items.iter().map(|s| parse_item(s)).collect()
}

/// `--item` on an edit is all-or-nothing: none supplied leaves the existing
/// lines alone, any supplied replaces the whole set.
fn optional_items(items: &[String]) -> Result<Option<Vec<NewLineItem>>> {
    if items.is_empty() {
        Ok(None)
    } else {
        parse_items(items).map(Some)
    }
}

fn void_summary(invoice: &Invoice, client_name: &str) -> String {
    format!(
        "Invoice #{} — {client_name}, {:.2} {}, {} {}.",
        invoice.number, invoice.total, invoice.currency, invoice.status, invoice.issue_date
    )
}

/// The data layer already answers an absent invoice as `NotFound`; this only
/// rewrites the sentence, because a terminal can be told what to run next.
fn find_invoice(conn: &Connection, number: i64) -> Result<Invoice> {
    get_invoice_by_number(conn, number).map_err(|e| match e {
        NigelError::NotFound(_) => NigelError::NotFound(format!(
            "No invoice #{number}. Run `nigel invoice list` to see invoice numbers."
        )),
        other => other,
    })
}

/// What voiding a published invoice will do, before it does it — the sentence
/// the CLI's confirmation and the TUI's dialog both show.
pub(crate) use crate::invoicing::void::PUBLISHED_VOID_NOTICE;

fn require(value: Option<String>, what: &str) -> Result<String> {
    value.ok_or_else(|| {
        NigelError::Other(format!(
            "missing invoicing config: {what} (set it in settings.json or the matching NIGEL_ env var)"
        ))
    })
}

/// The business name the settings screen writes, as the email subject and the
/// report headers want it: a plain string, empty when nobody has set one.
///
/// Kept beside `company_profile` rather than folded into it: the nine report
/// exporters, the text reports and `/api/status` want a name, not a letterhead.
pub(crate) fn company_name(conn: &Connection) -> String {
    crate::db::get_metadata(conn, "company_name").unwrap_or_default()
}

/// The whole letterhead, read from the one place it lives.
///
/// Owned, because `Branding` borrows and the values come out of the database.
/// Resolved here rather than at each `Branding` site: the fields are only ever
/// correct together, and six hand-built literals each doing their own
/// `get_metadata` calls is how a document ends up with an address and no phone.
pub(crate) struct CompanyProfile {
    pub name: String,
    pub address: String,
    pub phone: String,
    pub logo: String,
    pub payment_instructions: String,
}

pub(crate) fn company_profile(conn: &Connection) -> CompanyProfile {
    let read = |key: &str| crate::db::get_metadata(conn, key).unwrap_or_default();
    CompanyProfile {
        name: read("company_name"),
        address: read("company_address"),
        phone: read("company_phone"),
        logo: read("company_logo"),
        payment_instructions: read("payment_instructions"),
    }
}

impl CompanyProfile {
    /// The branding for this profile, with the template and contact address the
    /// caller resolved. One constructor, so no site can forget a field.
    pub(crate) fn branding<'a>(
        &'a self,
        template: &'a str,
        contact_email: &'a str,
    ) -> Branding<'a> {
        Branding {
            template,
            company: &self.name,
            company_address: &self.address,
            company_phone: &self.phone,
            logo: &self.logo,
            payment_instructions: &self.payment_instructions,
            contact_email,
        }
    }
}

fn build_gateway(cfg: &InvoicingConfig) -> Result<StripeClient> {
    Ok(StripeClient {
        secret_key: require(cfg.stripe_secret_key.clone(), "stripe_secret_key")?,
    })
}

/// The gateway if this installation has one, rather than a refusal.
///
/// Void is the one invoicing command that has to work on a machine with nothing
/// configured — an invoice can be drafted and cancelled without Stripe ever
/// being involved — so its teardown asks what is available instead of demanding
/// the nine keys `send` needs.
pub(crate) fn optional_gateway(cfg: &InvoicingConfig) -> Option<StripeClient> {
    Some(StripeClient {
        secret_key: cfg.stripe_secret_key.clone()?,
    })
}

/// The publisher, when every key it takes is set. All five or none: a publisher
/// missing its bucket is not a publisher that works for four fifths of a page.
pub(crate) fn optional_publisher(cfg: &InvoicingConfig) -> Option<R2Publisher> {
    Some(R2Publisher {
        account_id: cfg.r2_account_id.clone()?,
        access_key: cfg.r2_access_key.clone()?,
        secret_key: cfg.r2_secret_key.clone()?,
        bucket: cfg.r2_bucket.clone()?,
        public_base_url: cfg.public_base_url.clone()?,
    })
}

/// The three network clients a send needs, the sender identity its email
/// carries, and anything about the configuration worth saying out loud.
///
/// `warnings` are configuration Nigel will send with but wants the operator to
/// look at. They travel as data, the way `VoidOutcome::warnings()` does, so
/// each surface renders them where it can be read: a terminal prints them, the
/// TUI puts them on its status line, the API answers them as a field. Printing
/// them here would corrupt ratatui's alternate screen and would fire once per
/// call rather than once per send.
pub(crate) struct SendClients {
    pub stripe: StripeClient,
    pub r2: R2Publisher,
    pub mail: MailgunClient,
    pub warnings: Vec<String>,
}

/// The clients a send needs, or the first refusal the configuration earns.
///
/// `company` is the business name from the database, which an unset `from_name`
/// falls back to — the same value the subject line already uses. An empty
/// company and no `from_name` means a bare address, which is what this app sent
/// before these keys existed.
pub(crate) fn build_clients(cfg: InvoicingConfig, company: &str) -> Result<SendClients> {
    let stripe = build_gateway(&cfg)?;
    let account_id = require(cfg.r2_account_id, "r2_account_id")?;
    let access_key = require(cfg.r2_access_key, "r2_access_key")?;
    let secret_key = require(cfg.r2_secret_key, "r2_secret_key")?;
    let bucket = require(cfg.r2_bucket, "r2_bucket")?;
    let public_base_url = require(cfg.public_base_url, "public_base_url")?;
    // The single constructor both send paths use, and the last moment before any
    // client exists: a base URL that cannot produce a working link is refused
    // here, so nothing is published, nothing is emailed and no Stripe link is
    // created. `optional_publisher` stays lenient — void and republish only need
    // the upload to happen and never print the address.
    validate_public_base_url(&public_base_url)?;
    let r2 = R2Publisher {
        account_id,
        access_key,
        secret_key,
        bucket,
        public_base_url,
    };

    let api_key = require(cfg.mailgun_api_key, "mailgun_api_key")?;
    let domain = require(cfg.mailgun_domain, "mailgun_domain")?;

    // The from address is composed into a header like every other value here,
    // so it is guarded like one — and it must be a bare address, because
    // `format_address` is what puts the display name on.
    let from_address = require(cfg.from_email, "from_email")?;
    validate_header_value(&from_address, "from_email")?;
    validate_bare_address(&from_address, "from_email")?;

    // An unset `from_name` falls back to the business name, and the refusal
    // has to name whichever of the two the bad value actually came from: an
    // operator who never set `from_name` cannot fix `from_name`.
    let (from_name, name_source) = match cfg.from_name {
        Some(name) => (Some(name), "from_name"),
        None => (
            Some(company.trim().to_string()).filter(|c| !c.is_empty()),
            "the business name",
        ),
    };
    if let Some(name) = &from_name {
        validate_header_value(name, name_source)?;
    }
    // The reply-to gets no domain check: Mailgun constrains what a message is
    // sent from, not where a human replies to it.
    if let Some(reply_to) = &cfg.reply_to_email {
        validate_header_value(reply_to, "reply_to_email")?;
    }

    // Config cautions travel as data on the one channel, so a terminal, the
    // TUI and a browser all say the same things about the same installation.
    // The `/i` one is a caution and not a refusal, because an edge rewrite can
    // map that prefix onto the domain root.
    let warnings = from_address_domain_warning(&from_address, &domain)
        .into_iter()
        .chain(public_base_url_warning(&r2.public_base_url).map(str::to_string))
        .collect();

    let mail = MailgunClient {
        api_key,
        domain,
        envelope: EmailEnvelope {
            from_address,
            from_name,
            reply_to: cfg.reply_to_email,
        },
    };
    Ok(SendClients {
        stripe,
        r2,
        mail,
        warnings,
    })
}

pub fn new(
    client_id: i64,
    issue_date: &str,
    due_date: Option<&str>,
    currency: &str,
    items: &[String],
    notes: Option<&str>,
    terms: Option<&str>,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let parsed = parse_items(items)?;
    let id = create_invoice(
        &conn, client_id, issue_date, due_date, currency, &parsed, notes, terms,
    )?;
    let invoice = get_invoice(&conn, id)?;
    println!(
        "Created draft invoice #{} for {:.2} {}",
        invoice.number, invoice.total, invoice.currency
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn edit(
    number: i64,
    issue_date: Option<String>,
    due_date: Option<String>,
    clear_due: bool,
    currency: Option<String>,
    notes: Option<String>,
    terms: Option<String>,
    items: &[String],
    today: &str,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let invoice = find_invoice(&conn, number)?;
    let had_link = invoice.stripe_payment_link_id.is_some();

    let update = InvoiceUpdate {
        issue_date,
        due_date: if clear_due {
            Some(None)
        } else {
            due_date.map(Some)
        },
        currency,
        notes: notes.map(Some),
        terms: terms.map(Some),
        items: optional_items(items)?,
    };
    update_invoice(&conn, invoice.id, &update, today)?;

    let updated = get_invoice(&conn, invoice.id)?;
    println!(
        "Updated draft invoice #{number} — {:.2} {}",
        updated.total, updated.currency
    );
    if had_link && updated.stripe_payment_link_id.is_none() {
        println!(
            "Cleared the stale Stripe payment link; `nigel invoice send {number}` will create a new one."
        );
    }
    Ok(())
}

pub fn void(number: i64, yes: bool, today: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let invoice = find_invoice(&conn, number)?;
    ensure_voidable(&conn, &invoice)?;
    let client = get_client(&conn, invoice.client_id)?;

    println!("{}", void_summary(&invoice, &client.name));
    if invoice.published_at.is_some() {
        println!("{PUBLISHED_VOID_NOTICE}");
    }
    if !confirm_void(&invoice, yes)? {
        println!("Aborted.");
        return Ok(());
    }

    // Whatever this installation can reach. The teardown is best-effort by
    // construction, so an unset key is a warning at the end, never a refusal
    // here — see `invoicing::void`'s matrix.
    let cfg = invoicing_config();
    let outcome = void_invoice_with_teardown(
        &conn,
        invoice.id,
        today,
        optional_gateway(&cfg).as_ref(),
        optional_publisher(&cfg).as_ref(),
    )?;

    println!("Voided invoice #{number}.");
    for warning in outcome.warnings() {
        println!("{warning}");
    }
    Ok(())
}

fn confirm_void(invoice: &Invoice, yes: bool) -> Result<bool> {
    crate::cli::confirm_or_refuse(
        "Void it? [y/N]",
        &format!(
            "Refusing to void invoice #{} without confirmation. Pass --yes.",
            invoice.number
        ),
        yes,
    )
}

/// `nigel invoice list`, as text. Pure, so the parity fixtures can call it
/// without a terminal — the same shape `cli/report/text.rs` uses.
pub fn format_invoice_list(rows: &[InvoiceListRow]) -> String {
    let mut table = Table::new();
    table.set_header(vec!["#", "Status", "Client", "Total", "Due"]);
    for row in rows {
        table.add_row(vec![
            Cell::new(row.number),
            Cell::new(&row.status),
            // An invoice whose client row is gone still belongs on the list.
            Cell::new(row.client_name.as_deref().unwrap_or("\u{2014}")),
            Cell::new(money(row.total)),
            Cell::new(row.due_date.as_deref().unwrap_or_default()),
        ]);
    }
    format!("Invoices\n{table}")
}

/// `nigel invoice show`, as text. Ends in a newline, so callers `print!` it.
pub fn format_invoice_show(
    invoice: &Invoice,
    client: &Client,
    items: &[InvoiceLineItem],
    paid: f64,
) -> String {
    let mut out = format!(
        "Invoice #{}  [{}]  {} {}\n",
        invoice.number,
        invoice.status,
        invoice.currency,
        money(invoice.total)
    );
    out.push_str(&format!("Client:   {}\n", client.name));
    out.push_str(&format!("Issued:   {}\n", invoice.issue_date));
    out.push_str(&format!(
        "Due:      {}\n",
        invoice.due_date.as_deref().unwrap_or("-")
    ));

    let mut table = Table::new();
    table.set_header(vec!["Description", "Qty", "Unit", "Amount"]);
    for item in items {
        table.add_row(vec![
            Cell::new(&item.description),
            // Quantity is a count, not an amount — it keeps its plain decimals.
            Cell::new(format!("{:.2}", item.quantity)),
            Cell::new(money(item.unit_amount)),
            Cell::new(money(item.line_total)),
        ]);
    }
    out.push_str(&format!("{table}\n"));

    out.push_str(&format!("Paid:     {}\n", money(paid)));
    out.push_str(&format!("Balance:  {}\n", money(invoice.total - paid)));
    if let Some(url) = invoice.stripe_payment_link_url.as_deref() {
        out.push_str(&format!("Pay:      {url}\n"));
    }
    out
}

pub fn list() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    println!(
        "{}",
        format_invoice_list(&list_invoices(&conn, None, None)?)
    );
    Ok(())
}

pub fn show(number: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let invoice = find_invoice(&conn, number)?;
    let client = get_client(&conn, invoice.client_id)?;
    let items = line_items(&conn, invoice.id)?;
    let paid = paid_amount(&conn, invoice.id)?;

    print!("{}", format_invoice_show(&invoice, &client, &items, paid));
    Ok(())
}

/// What `{{CONTACT}}` prints when neither `contact_email` nor `from_email` is
/// configured. Preview is the one invoicing command that runs without any
/// configuration, so it renders a visible stand-in rather than refusing.
const PREVIEW_CONTACT_PLACEHOLDER: &str = "(contact_email not configured)";

/// The placeholder a template uses to print the contact address. The stock page
/// no longer does; a custom one may.
const CONTACT_PLACEHOLDER_KEY: &str = "{{CONTACT}}";

/// The notice a document with no way to pay on it earns, or nothing.
///
/// The stock page used to hardcode a bank-transfer paragraph. It does not any
/// more, which is the point — but an operator who never set
/// `payment_instructions` would otherwise send an amount owed with nothing on
/// the document about how to settle it, and hear nothing about it. This is the
/// old placeholder notice's job, moved to the thing that is now missing.
///
/// Silent for a **custom template**: that page is the operator's, it may say
/// whatever it likes about paying, and Nigel cannot read it.
pub(crate) fn payment_instructions_notice(
    payment_instructions: &str,
    has_template_override: bool,
) -> Option<&'static str> {
    (payment_instructions.trim().is_empty() && !has_template_override).then_some(
        "notice: no payment_instructions are set, so neither document says how to pay \
         — set them in Settings, or leave them unset deliberately",
    )
}

fn preview_dir(output_dir: Option<String>) -> (PathBuf, bool) {
    match output_dir {
        Some(dir) => (
            PathBuf::from(crate::settings::shellexpand_path(&dir)),
            false,
        ),
        None => (get_data_dir().join("previews"), true),
    }
}

fn preview_paths(dir: &Path, number: i64) -> (PathBuf, PathBuf) {
    (
        dir.join(format!("invoice-{number}.html")),
        dir.join(format!("invoice-{number}.pdf")),
    )
}

/// Which pay element a rendered invoice carries. It lives beside the render
/// seam, so `preview`, `send`, a republish and the API's preview routes cannot
/// disagree about the same invoice; this is the name the CLI and the server
/// already import it by.
pub(crate) use crate::invoicing::render::pay_button_for;

/// Republish one invoice's published page after a payment. Returns the
/// sentences to print; never fails.
///
/// `src/invoicing/` reads no settings and loads no template, so the branding,
/// the publisher and the rows are resolved here and passed down. A broken custom
/// template, an unreadable data directory and an R2 outage are all *warnings*:
/// the payment is committed, and nothing at this end may read as its failure.
///
/// The config and the data directory arrive as arguments, the way `begin_send`
/// takes them: each front end resolves its own settings at its own call site, so
/// nothing below here can reach an ambient one — and a test that does not pass a
/// config cannot reach a configured bucket.
pub(crate) fn republish_after_payment(
    conn: &Connection,
    invoice_id: i64,
    cfg: &InvoicingConfig,
    data_dir: &Path,
) -> Vec<String> {
    republish_with(
        conn,
        invoice_id,
        cfg,
        data_dir,
        optional_publisher(cfg).as_ref(),
    )
}

/// The same with the publisher supplied, which is what lets the HTTP layer drive
/// a republish against a fake and reach no network — the `send_with`/`void_with`
/// seam, and the one place the sentences a failed republish earns are written.
pub(crate) fn republish_with<P: crate::invoicing::gateway::AssetPublisher>(
    conn: &Connection,
    invoice_id: i64,
    cfg: &InvoicingConfig,
    data_dir: &Path,
    publisher: Option<&P>,
) -> Vec<String> {
    let warn = |what: &str, e: NigelError| {
        vec![format!(
            "Warning: the payment is recorded, but the published page could not be republished \
             ({what}: {e})."
        )]
    };

    let invoice = match get_invoice(conn, invoice_id) {
        Ok(invoice) => invoice,
        Err(e) => return warn("reading the invoice", e),
    };
    // The ordinary case, and the one that must cost nothing: most payments land
    // on invoices that were never published.
    if invoice.published_at.is_none() {
        return Vec::new();
    }
    let client = match get_client(conn, invoice.client_id) {
        Ok(client) => client,
        Err(e) => return warn("reading the client", e),
    };
    let template = match load_template(data_dir) {
        Ok(template) => template,
        Err(e) => return warn("loading the invoice template", e),
    };

    // The preview fallback, not `require`: a republish must not depend on an
    // address being configured, since the page it is correcting is already up.
    let (contact_email, _) = contact_email_for_preview(cfg);
    let profile = company_profile(conn);
    let branding = profile.branding(&template, &contact_email);
    republish_invoice(conn, &invoice, &client, &branding, publisher).warnings()
}

/// The same, for every invoice a sync recorded a payment against.
pub fn republish_all(
    conn: &Connection,
    numbers: &[i64],
    cfg: &InvoicingConfig,
    data_dir: &Path,
) -> Vec<String> {
    numbers
        .iter()
        .flat_map(|number| match get_invoice_by_number(conn, *number) {
            Ok(invoice) => republish_after_payment(conn, invoice.id, cfg, data_dir),
            Err(e) => vec![format!(
                "Warning: could not republish invoice #{number}'s page ({e})."
            )],
        })
        .collect()
}

/// The address the published page's direct-deposit line prints. Falls back to
/// the send address, so an installation that never sets `contact_email` renders
/// exactly the page it rendered before the key existed.
pub(crate) fn contact_address(cfg: &InvoicingConfig) -> Option<String> {
    cfg.contact_email.clone().or_else(|| cfg.from_email.clone())
}

pub(crate) fn contact_email_for_preview(cfg: &InvoicingConfig) -> (String, bool) {
    match contact_address(cfg) {
        Some(email) => (email, false),
        None => (PREVIEW_CONTACT_PLACEHOLDER.to_string(), true),
    }
}

pub fn preview(number: i64, output_dir: Option<String>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let invoice = find_invoice(&conn, number)?;
    let client = get_client(&conn, invoice.client_id)?;

    if is_void(&invoice) {
        eprintln!("notice: invoice #{number} is void — this preview is for reference only.");
    }
    let (contact_email, is_placeholder) = contact_email_for_preview(&invoicing_config());

    let template = load_template(&get_data_dir())?;
    let has_override = matches!(template, std::borrow::Cow::Owned(_));
    // The stock page does not print `{{CONTACT}}` any more — payment
    // instructions are the operator's own text — so the notice is about the
    // template actually in use rather than about the setting in the abstract.
    if is_placeholder && template.contains(CONTACT_PLACEHOLDER_KEY) {
        eprintln!(
            "notice: neither contact_email nor from_email is configured — this template's {CONTACT_PLACEHOLDER_KEY} is a placeholder"
        );
    }
    let profile = company_profile(&conn);
    if let Some(notice) = payment_instructions_notice(&profile.payment_instructions, has_override) {
        eprintln!("{notice}");
    }
    let branding = profile.branding(&template, &contact_email);

    // Both artifacts are rendered before either is written, so a PDF failure
    // cannot leave fresh HTML beside a stale PDF.
    let rendered = render_invoice(
        &conn,
        &invoice,
        &client,
        pay_button_for(&invoice),
        &branding,
    )?;

    write_preview(&rendered, number, output_dir)
}

/// Write a rendered invoice beside itself and say where it went.
///
/// The one place the preview paths, the permissions and the `Wrote`/no-PDF
/// wording live, so `preview` and the confirmation `send` shows cannot differ
/// about what was written or where.
fn write_preview(
    rendered: &RenderedInvoice,
    number: i64,
    output_dir: Option<String>,
) -> Result<()> {
    let (dir, is_default) = preview_dir(output_dir);
    std::fs::create_dir_all(&dir)?;
    if is_default {
        // Only the directory Nigel chose. A directory the user named may be
        // shared on purpose, and tightening it would be a surprise.
        crate::settings::restrict_dir_permissions(&dir)?;
    }
    let (html_path, pdf_path) = preview_paths(&dir, number);

    std::fs::write(&html_path, &rendered.html)?;
    crate::settings::restrict_file_permissions(&html_path)?;
    println!("Wrote {}", html_path.display());

    match &rendered.pdf {
        Some(bytes) => {
            std::fs::write(&pdf_path, bytes)?;
            crate::settings::restrict_file_permissions(&pdf_path)?;
            println!("Wrote {}", pdf_path.display());
        }
        None => eprintln!("notice: {}", crate::cli::report::PDF_DISABLED_MESSAGE),
    }
    Ok(())
}

/// The line the operator reads before answering: which invoice, whose, how
/// much, and the address it is going to.
fn send_summary(invoice: &Invoice, client: &Client) -> String {
    format!(
        "Invoice #{} — {}, {} {}, issued {}. To {}.",
        invoice.number,
        client.name,
        money(invoice.total),
        invoice.currency,
        invoice.issue_date,
        // A client with no address is refused by the precheck a moment later;
        // the summary states the fact rather than printing "None".
        client
            .email
            .as_deref()
            .unwrap_or("no email address on file")
    )
}

/// What sending will do, in one paragraph. The TUI's `send_confirmation` in
/// prose, worded for a first send or a re-send, naming the publish host when
/// there is one.
fn send_consequences(invoice: &Invoice, client: &Client, publish_host: Option<&str>) -> String {
    let to = match client.email.as_deref() {
        Some(email) => format!("emails {email}"),
        None => "emails the client".to_string(),
    };
    let host = match publish_host {
        Some(host) => format!("publishes the page and PDF to {host}"),
        None => "publishes the page and PDF".to_string(),
    };
    match invoice.published_at {
        Some(_) => format!(
            "The existing payment link is reused; Nigel re-{host} and {to} again. \
             This cannot be undone."
        ),
        None => format!(
            "Sending creates a Stripe payment link, {host}, and {to}. \
             This cannot be undone."
        ),
    }
}

/// `confirm_void`'s twin. `--yes` is the answer; a non-TTY without it is a
/// refusal rather than a send nobody saw.
fn confirm_send(invoice: &Invoice, yes: bool) -> Result<bool> {
    if yes {
        return Ok(true);
    }
    refuse_unconfirmed_send(invoice)?;
    print!("Send it? [y/N] ");
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(answer.trim().eq_ignore_ascii_case("y"))
}

/// Nobody is there to answer. Checked before the summary is printed and before
/// the preview files are written, so a scripted send that cannot be confirmed
/// leaves nothing behind.
fn refuse_unconfirmed_send(invoice: &Invoice) -> Result<()> {
    if std::io::stdin().is_terminal() {
        return Ok(());
    }
    Err(NigelError::Other(format!(
        "Refusing to send invoice #{} without confirmation. Pass --yes.",
        invoice.number
    )))
}

/// The host a published page is served from, for the consequence sentence.
/// Just the authority — the operator recognizes `billing.example.com`, not a path.
fn publish_host(base: &str) -> Option<String> {
    let rest = base
        .strip_prefix("https://")
        .or_else(|| base.strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    (!host.is_empty()).then(|| host.to_string())
}

pub fn send(number: i64, today: &str, yes: bool) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let invoice = find_invoice(&conn, number)?;
    ensure_not_void(&invoice, "sent")?;
    let client = get_client(&conn, invoice.client_id)?;
    // The template is loaded before anything is built or created, so a broken
    // one fails the send with no Stripe link made and nothing published.
    let template = load_template(&get_data_dir())?;
    let cfg = invoicing_config();
    let contact_email = contact_email_for_preview(&cfg).0;
    let profile = company_profile(&conn);
    if let Some(notice) = payment_instructions_notice(
        &profile.payment_instructions,
        matches!(template, std::borrow::Cow::Owned(_)),
    ) {
        eprintln!("{notice}");
    }
    let branding = profile.branding(&template, &contact_email);

    // Rendered before the decision and before any client is built, through the
    // seam `send` publishes through — so what the operator looks at is the
    // document the client will get, and a broken template costs no Stripe link.
    let rendered = render_invoice(
        &conn,
        &invoice,
        &client,
        pay_button_for(&invoice),
        &branding,
    )?;

    if !yes {
        refuse_unconfirmed_send(&invoice)?;
        println!("{}", send_summary(&invoice, &client));
        // The same bytes the send will publish, at the paths `invoice preview`
        // writes. A terminal cannot show a document; it can hand over a path.
        write_preview(&rendered, number, None)?;
        let host = cfg.public_base_url.as_deref().and_then(publish_host);
        println!("{}", send_consequences(&invoice, &client, host.as_deref()));
        if !confirm_send(&invoice, yes)? {
            println!("Aborted.");
            return Ok(());
        }
    }

    let clients = build_clients(cfg, &profile.name)?;
    for warning in &clients.warnings {
        eprintln!("notice: {warning}");
    }
    let url = send_invoice(
        &conn,
        invoice.id,
        today,
        &branding,
        &clients.stripe,
        &clients.r2,
        &clients.mail,
    )?;
    println!("Sent invoice #{number}: {url}");
    Ok(())
}

pub fn sync(today: &str) -> Result<()> {
    let data_dir = get_data_dir();
    let conn = get_connection(&data_dir.join("nigel.db"))?;
    let cfg = invoicing_config();
    let stripe = build_gateway(&cfg)?;
    let report = sync_all_report(&conn, today, &stripe)?;
    // Printing is the front end's job: the data layer hands back the per-invoice
    // failures so a browser can render the same ones a terminal prints.
    for failure in &report.failures {
        eprintln!(
            "notice: invoice sync failed for #{}: {}",
            failure.number, failure.message
        );
    }
    println!("Recorded {} new payment(s)", report.recorded);
    for warning in republish_all(&conn, &report.recorded_invoices, &cfg, &data_dir) {
        println!("{warning}");
    }
    Ok(())
}

pub fn pay(number: i64, amount: Option<f64>, date: &str, method: &str) -> Result<()> {
    let data_dir = get_data_dir();
    let conn = get_connection(&data_dir.join("nigel.db"))?;
    let invoice = find_invoice(&conn, number)?;
    ensure_not_void(&invoice, "paid")?;
    let paid = paid_amount(&conn, invoice.id)?;
    let amount = payment_amount(&invoice, paid, amount)?;
    record_payment(&conn, invoice.id, amount, date, method, None)?;
    let invoice = get_invoice(&conn, invoice.id)?;
    println!(
        "Recorded {amount:.2} against invoice #{number} ({})",
        invoice.status
    );
    // After the payment is committed, and never able to undo it: the page a
    // client bookmarked is corrected, or a warning says why it was not.
    for warning in republish_after_payment(&conn, invoice.id, &invoicing_config(), &data_dir) {
        println!("{warning}");
    }
    Ok(())
}

pub fn aging(today: &str) -> Result<()> {
    println!("{}", crate::cli::report::text::aging(today)?);
    Ok(())
}

pub fn template_export(output: Option<&str>, force: bool) -> Result<()> {
    let destination = match output {
        Some(path) => PathBuf::from(crate::settings::shellexpand_path(path)),
        None => template_path(&get_data_dir()),
    };

    if destination.exists() && !force {
        return Err(NigelError::Invalid(format!(
            "{} already exists. Pass --force to overwrite it.",
            destination.display()
        )));
    }
    let write_error = |e: std::io::Error| {
        NigelError::Invalid(format!(
            "Cannot write invoice template to {}: {e}",
            destination.display()
        ))
    };
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(write_error)?;
    }
    std::fs::write(&destination, DEFAULT_TEMPLATE).map_err(write_error)?;

    println!("Wrote invoice template to {}", destination.display());
    println!(
        "Edit it, then check it with `nigel invoice preview <number>` — see docs/invoicing.md."
    );
    Ok(())
}

pub fn template_show_path() -> Result<()> {
    let path = template_path(&get_data_dir());
    println!("{}", path.display());

    if !path.exists() {
        println!("No custom template — the built-in one is in use.");
        return Ok(());
    }
    load_template(&get_data_dir())?;
    println!("Custom template in effect.");
    Ok(())
}

pub fn import(db: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let summary = import_invoiceshelf(&conn, Path::new(db))?;
    println!(
        "Imported {} clients, {} invoices, {} payments. Next invoice number: {}",
        summary.clients, summary.invoices, summary.payments, summary.next_number
    );
    if summary.unparsed_dates > 0 {
        eprintln!(
            "Warning: {} date(s) were not in YYYY-MM-DD form and were copied as they stand. \
             They will not sort or bucket as dates until corrected.",
            summary.unparsed_dates
        );
    }
    if summary.unusable_emails > 0 {
        eprintln!(
            "Warning: {} email address(es) carry a character a mail header may not, and were \
             copied as they stand. Sending to those clients will refuse until they are corrected.",
            summary.unusable_emails
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::invoicing::clients::add_client;
    use crate::invoicing::invoices::create_invoice;
    use crate::migrations::run_migrations;

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    /// Insert one 100.00 draft invoice (number 1248) and return its row id.
    fn seed_invoice(conn: &Connection) -> i64 {
        let cid = add_client(conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem {
            description: "W".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        create_invoice(conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap()
    }

    fn list_row(
        number: i64,
        status: &str,
        client: &str,
        total: f64,
        due: Option<&str>,
    ) -> InvoiceListRow {
        InvoiceListRow {
            id: number - 1000,
            number,
            status: status.into(),
            client_id: 1,
            client_name: Some(client.into()),
            issue_date: "2026-03-01".into(),
            due_date: due.map(str::to_string),
            currency: "USD".into(),
            total,
            paid: 0.0,
            balance: total,
        }
    }

    /// Byte-for-byte what `nigel invoice list` prints. Money goes through
    /// `fmt::money`, the same format `format_aging` and `wc-money` use.
    #[test]
    fn format_invoice_list_prints_money_the_way_every_other_report_does() {
        let out = format_invoice_list(&[
            list_row(1250, "partial", "Acme Co", 3200.0, Some("2026-08-20")),
            list_row(1249, "overdue", "Globex", 960.0, Some("2026-06-30")),
            list_row(1248, "draft", "Northwind Traders", 1850.5, None),
        ]);
        assert_eq!(
            out,
            concat!(
                "Invoices\n",
                "+------+---------+-------------------+-----------+------------+\n",
                "| #    | Status  | Client            | Total     | Due        |\n",
                "+=============================================================+\n",
                "| 1250 | partial | Acme Co           | $3,200.00 | 2026-08-20 |\n",
                "|------+---------+-------------------+-----------+------------|\n",
                "| 1249 | overdue | Globex            | $960.00   | 2026-06-30 |\n",
                "|------+---------+-------------------+-----------+------------|\n",
                "| 1248 | draft   | Northwind Traders | $1,850.50 |            |\n",
                "+------+---------+-------------------+-----------+------------+",
            )
        );
    }

    #[test]
    fn format_invoice_list_shows_an_orphaned_invoice_rather_than_hiding_it() {
        let mut row = list_row(1247, "void", "gone", 500.0, None);
        row.client_name = None;
        let out = format_invoice_list(&[row]);
        assert!(out.contains("1247"), "got: {out}");
        assert!(out.contains('\u{2014}'), "want an em dash, got: {out}");
    }

    /// Byte-for-byte what `nigel invoice show` prints. Quantity keeps its plain
    /// decimals — it is a count, not an amount.
    #[test]
    fn format_invoice_show_prints_money_the_way_every_other_report_does() {
        let invoice = Invoice {
            id: 7,
            number: 1250,
            client_id: 3,
            issue_date: "2026-03-01".into(),
            due_date: Some("2026-08-20".into()),
            status: "partial".into(),
            currency: "USD".into(),
            subtotal: 3200.0,
            tax: 0.0,
            total: 3200.0,
            notes: None,
            terms: None,
            token: "aBc123".into(),
            stripe_payment_link_id: Some("pl_1".into()),
            stripe_payment_link_url: Some("https://pay/x".into()),
            published_at: Some("2026-03-01".into()),
            voided_at: None,
        };
        let client = Client {
            id: 3,
            name: "Acme Co".into(),
            email: Some("ap@acme.test".into()),
            billing_address: None,
            notes: None,
            archived_at: None,
        };
        let item = |description: &str, quantity: f64, unit_amount: f64| InvoiceLineItem {
            id: None,
            invoice_id: Some(7),
            description: description.into(),
            quantity,
            unit_amount,
            line_total: quantity * unit_amount,
            position: 0,
        };

        let out = format_invoice_show(
            &invoice,
            &client,
            &[
                item("Consulting — August", 10.0, 150.0),
                item("Hosting", 1.0, 1700.0),
            ],
            2000.0,
        );
        assert_eq!(
            out,
            concat!(
                "Invoice #1250  [partial]  USD $3,200.00\n",
                "Client:   Acme Co\n",
                "Issued:   2026-03-01\n",
                "Due:      2026-08-20\n",
                "+---------------------+-------+-----------+-----------+\n",
                "| Description         | Qty   | Unit      | Amount    |\n",
                "+=====================================================+\n",
                "| Consulting — August | 10.00 | $150.00   | $1,500.00 |\n",
                "|---------------------+-------+-----------+-----------|\n",
                "| Hosting             | 1.00  | $1,700.00 | $1,700.00 |\n",
                "+---------------------+-------+-----------+-----------+\n",
                "Paid:     $2,000.00\n",
                "Balance:  $1,200.00\n",
                "Pay:      https://pay/x\n",
            )
        );

        // No due date reads as a dash; no payment link prints no Pay line.
        let mut plain = invoice.clone();
        plain.due_date = None;
        plain.stripe_payment_link_url = None;
        let out = format_invoice_show(&plain, &client, &[], 0.0);
        assert!(out.contains("Due:      -\n"), "got: {out}");
        assert!(!out.contains("Pay:"), "got: {out}");
    }

    #[test]
    fn parse_item_splits_desc_qty_unit() {
        let item = parse_item("Design:2:150.50").unwrap();
        assert_eq!(item.description, "Design");
        assert_eq!(item.quantity, 2.0);
        assert_eq!(item.unit_amount, 150.50);
    }

    #[test]
    fn parse_item_rejects_wrong_shape_and_bad_numbers() {
        assert!(parse_item("Design:2").is_err());
        assert!(parse_item("Design:two:150").is_err());
        assert!(parse_item("Design:2:free").is_err());
    }

    #[test]
    fn an_invoice_needs_at_least_one_item() {
        let err = parse_items(&[]).map(|_| ()).unwrap_err().to_string();
        assert!(err.contains("--item"), "got: {err}");
        assert_eq!(parse_items(&["W:1:10".to_string()]).unwrap().len(), 1);
    }

    #[test]
    fn parse_items_is_optional_for_an_edit() {
        assert!(optional_items(&[]).unwrap().is_none());

        let parsed = optional_items(&["Rework:2:250".to_string()])
            .unwrap()
            .unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].description, "Rework");
        assert_eq!(parsed[0].unit_amount, 250.0);

        assert!(optional_items(&["Rework:2".to_string()]).is_err());
    }

    #[test]
    fn confirm_prompt_names_the_invoice() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn);
        let invoice = find_invoice(&conn, 1248).unwrap();

        let line = void_summary(&invoice, "Acme Co");
        assert!(line.contains("#1248"), "got: {line}");
        assert!(line.contains("Acme Co"), "got: {line}");
        assert!(line.contains("100.00 USD"), "got: {line}");
        assert!(line.contains("draft"), "got: {line}");
    }

    fn test_config() -> InvoicingConfig {
        InvoicingConfig {
            stripe_secret_key: None,
            mailgun_api_key: None,
            mailgun_domain: None,
            from_email: None,
            from_name: None,
            reply_to_email: None,
            contact_email: None,
            r2_account_id: None,
            r2_access_key: None,
            r2_secret_key: None,
            r2_bucket: None,
            public_base_url: None,
        }
    }

    /// Every key a send needs, with a from address on its own Mailgun domain.
    fn configured() -> InvoicingConfig {
        InvoicingConfig {
            mailgun_domain: Some("mg.example.test".into()),
            from_email: Some("billing@mg.example.test".into()),
            ..config_up_to_mailgun()
        }
    }

    /// The published invoice these three work on.
    fn seed_published(conn: &Connection) -> i64 {
        let id = seed_invoice(conn);
        crate::invoicing::invoices::mark_published(conn, id, "2026-08-04").unwrap();
        crate::invoicing::invoices::record_payment(conn, id, 40.0, "2026-08-05", "other", None)
            .unwrap();
        id
    }

    /// The seam takes the publisher as well as the config, so the whole
    /// republish runs against a fake: a fully-configured `InvoicingConfig` is
    /// still not a bucket anything in this module can reach.
    #[derive(Default)]
    struct CapturePub {
        pages: std::cell::RefCell<Vec<String>>,
    }
    impl crate::invoicing::gateway::AssetPublisher for CapturePub {
        fn publish(&self, token: &str, html: &[u8], _pdf: &[u8]) -> Result<String> {
            self.publish_page(token, html)
        }
        fn publish_page(&self, token: &str, html: &[u8]) -> Result<String> {
            self.pages
                .borrow_mut()
                .push(String::from_utf8(html.to_vec()).unwrap());
            Ok(format!("https://billing.example.test/i/{token}/index.html"))
        }
    }

    #[test]
    fn a_republish_uploads_through_the_publisher_it_was_handed() {
        let (_d, conn) = test_conn();
        let id = seed_published(&conn);
        let publisher = CapturePub::default();

        let warnings = republish_with(&conn, id, &configured(), _d.path(), Some(&publisher));

        assert!(warnings.is_empty(), "{warnings:?}");
        let pages = publisher.pages.borrow();
        assert_eq!(pages.len(), 1, "the corrected page, and nothing else");
        assert!(pages[0].contains("Balance due"), "got: {}", pages[0]);
    }

    #[test]
    fn republishing_with_nothing_configured_warns_and_records_nothing() {
        let (_d, conn) = test_conn();
        let id = seed_published(&conn);

        let warnings = republish_after_payment(&conn, id, &test_config(), _d.path());

        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("#1248"), "{warnings:?}");
        assert!(warnings[0].contains("old balance"), "{warnings:?}");
        // The payment is money that was received; nothing here touched it.
        assert_eq!(paid_amount(&conn, id).unwrap(), 40.0);
    }

    #[test]
    fn republishing_an_unpublished_invoice_says_nothing() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn);
        record_payment(&conn, id, 40.0, "2026-08-05", "other", None).unwrap();

        assert!(republish_after_payment(&conn, id, &test_config(), _d.path()).is_empty());
    }

    #[test]
    fn a_broken_custom_template_is_a_warning_not_a_failure() {
        let (_d, conn) = test_conn();
        let id = seed_published(&conn);

        let path = template_path(_d.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "<p>{{NUMBER}} {{CLIENT}} {{ROWS}} {{TOTAL}} {{TOTL}}</p>",
        )
        .unwrap();

        let warnings = republish_after_payment(&conn, id, &test_config(), _d.path());

        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("{{TOTL}}"), "{warnings:?}");
        assert!(
            warnings[0].contains("payment is recorded"),
            "a template typo may not read as a failed payment: {warnings:?}"
        );
    }

    fn acme() -> Client {
        Client {
            id: 1,
            name: "Acme Co".into(),
            email: Some("ap@acme.test".into()),
            billing_address: None,
            notes: None,
            archived_at: None,
        }
    }

    #[test]
    fn the_summary_names_the_client_the_total_and_the_recipient() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn);
        let invoice = find_invoice(&conn, 1248).unwrap();

        let line = send_summary(&invoice, &acme());
        assert!(line.contains("#1248"), "got: {line}");
        assert!(line.contains("Acme Co"), "got: {line}");
        assert!(line.contains("$100.00"), "got: {line}");
        assert!(line.contains("USD"), "got: {line}");
        assert!(line.contains("2026-08-04"), "got: {line}");
        assert!(line.contains("ap@acme.test"), "the address it is going to");
    }

    #[test]
    fn a_client_with_no_email_is_still_summarised() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn);
        let invoice = find_invoice(&conn, 1248).unwrap();
        let mut client = acme();
        client.email = None;

        let line = send_summary(&invoice, &client);
        assert!(line.contains("Acme Co"), "got: {line}");
        assert!(!line.contains("None"), "never Debug output: {line}");
    }

    #[test]
    fn a_resend_says_the_link_is_reused_and_the_page_republished() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn);
        crate::invoicing::invoices::mark_published(&conn, id, "2026-08-04").unwrap();
        let invoice = find_invoice(&conn, 1248).unwrap();

        let first = send_consequences(&invoice, &acme(), None);
        assert!(
            first.contains("existing payment link is reused"),
            "got: {first}"
        );
        assert!(first.contains("cannot be undone"), "got: {first}");
    }

    #[test]
    fn a_first_send_says_a_link_is_created() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn);
        let invoice = find_invoice(&conn, 1248).unwrap();

        let line = send_consequences(&invoice, &acme(), None);
        assert!(
            line.contains("creates a Stripe payment link"),
            "got: {line}"
        );
        assert!(line.contains("ap@acme.test"), "got: {line}");
    }

    #[test]
    fn the_consequences_name_the_publish_host_when_there_is_one() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn);
        let invoice = find_invoice(&conn, 1248).unwrap();

        let host = publish_host("https://billing.example.com/i");
        assert_eq!(host.as_deref(), Some("billing.example.com"));
        let line = send_consequences(&invoice, &acme(), host.as_deref());
        assert!(line.contains("to billing.example.com"), "got: {line}");

        // Nothing configured: the sentence still reads, without a dangling "to".
        let bare = send_consequences(&invoice, &acme(), None);
        assert!(bare.contains("publishes the page and PDF,"), "got: {bare}");
        assert_eq!(publish_host("billing.example.com"), None);
    }

    /// `--yes` is the answer, so nothing is read and nothing is written.
    #[test]
    fn yes_confirms_without_asking() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn);
        let invoice = find_invoice(&conn, 1248).unwrap();
        assert!(confirm_send(&invoice, true).unwrap());
    }

    #[test]
    fn missing_secret_names_the_setting() {
        let err = build_clients(test_config(), "Bluepeak")
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(err.contains("stripe_secret_key"), "got: {err}");
    }

    #[test]
    fn the_display_name_falls_back_to_the_company_name() {
        let cfg = InvoicingConfig {
            from_name: None,
            ..configured()
        };
        let mail = build_clients(cfg, "Bluepeak").expect("built").mail;
        assert_eq!(mail.envelope.from_name.as_deref(), Some("Bluepeak"));

        let cfg = InvoicingConfig {
            from_name: Some("Bluepeak Books".into()),
            ..configured()
        };
        let mail = build_clients(cfg, "Bluepeak").expect("built").mail;
        assert_eq!(
            mail.envelope.from_name.as_deref(),
            Some("Bluepeak Books"),
            "from_name wins over the business name"
        );
    }

    #[test]
    fn no_company_and_no_from_name_means_no_display_name() {
        let cfg = InvoicingConfig {
            from_name: None,
            ..configured()
        };
        let mail = build_clients(cfg, "").expect("built").mail;
        assert!(mail.envelope.from_name.is_none());
    }

    #[test]
    fn a_reply_to_reaches_the_envelope_and_is_not_domain_checked() {
        let cfg = InvoicingConfig {
            reply_to_email: Some("sam@elsewhere.test".into()),
            ..configured()
        };
        let mail = build_clients(cfg, "Bluepeak").expect("built").mail;
        assert_eq!(
            mail.envelope.reply_to.as_deref(),
            Some("sam@elsewhere.test")
        );
    }

    /// A from address off the sending domain is a deliverable setup Nigel
    /// cannot verify, so it warns and sends — and the warning is data the
    /// caller renders, never a print into a terminal ratatui owns.
    #[test]
    fn a_from_address_off_the_mailgun_domain_warns_as_data_and_still_builds() {
        let cfg = InvoicingConfig {
            from_email: Some("billing@elsewhere.test".into()),
            ..configured()
        };
        let built = build_clients(cfg, "Bluepeak").expect("built");
        assert_eq!(built.mail.envelope.from_address, "billing@elsewhere.test");
        assert_eq!(built.warnings.len(), 1, "got: {:?}", built.warnings);
        assert!(built.warnings[0].contains("mailgun_domain"));

        let on_domain = build_clients(configured(), "Bluepeak").expect("built");
        assert!(
            on_domain.warnings.is_empty(),
            "got: {:?}",
            on_domain.warnings
        );
    }

    #[test]
    fn a_from_address_with_a_control_character_is_refused_by_name() {
        // Header injection through `NIGEL_FROM_EMAIL`: the from address is
        // composed into a header like every other value, so it is guarded like
        // one.
        let cfg = InvoicingConfig {
            from_email: Some("billing@mg.example.test\r\nBcc: attacker@evil.test".into()),
            ..configured()
        };
        let err = build_clients(cfg, "Bluepeak")
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(err.contains("from_email"), "got: {err}");
        assert!(!err.contains("attacker@evil.test"), "got: {err}");
    }

    #[test]
    fn a_from_email_that_already_carries_a_display_name_is_refused_before_any_call() {
        // The old way to get a display name. Left alone it would compose a
        // nested `name-addr` Mailgun rejects — after the Stripe link and the
        // upload.
        let cfg = InvoicingConfig {
            from_email: Some("Acme LLC <billing@mg.example.test>".into()),
            ..configured()
        };
        let err = build_clients(cfg, "Bluepeak")
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("from_email") && err.contains("from_name"),
            "got: {err}"
        );
    }

    /// The refusal has to name where the value came from: an operator who never
    /// set `from_name` cannot fix `from_name`.
    #[test]
    fn a_bad_business_name_is_refused_by_its_own_name_not_by_from_name() {
        let cfg = InvoicingConfig {
            from_name: None,
            ..configured()
        };
        let err = build_clients(cfg, "Bluepeak\r\nBcc: x@y.test")
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(err.contains("business name"), "got: {err}");
        assert!(!err.contains("from_name"), "got: {err}");
    }

    #[test]
    fn a_display_name_with_a_line_break_is_refused_by_name() {
        let cfg = InvoicingConfig {
            from_name: Some("Bluepeak\r\nBcc: x@y.test".into()),
            ..configured()
        };
        let err = build_clients(cfg, "").map(|_| ()).unwrap_err().to_string();
        assert!(err.contains("from_name"), "got: {err}");
        assert!(!err.contains("x@y.test"), "got: {err}");
    }

    #[test]
    fn a_reply_to_with_a_line_break_is_refused_by_name() {
        let cfg = InvoicingConfig {
            reply_to_email: Some("sam@example.com\nBcc: x@y.test".into()),
            ..configured()
        };
        let err = build_clients(cfg, "Bluepeak")
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(err.contains("reply_to_email"), "got: {err}");
    }

    #[test]
    fn the_page_contact_falls_back_to_the_from_address() {
        let cfg = InvoicingConfig {
            contact_email: None,
            from_email: Some("billing@mg.example.com".into()),
            ..test_config()
        };
        assert_eq!(
            contact_address(&cfg).as_deref(),
            Some("billing@mg.example.com")
        );

        let cfg = InvoicingConfig {
            contact_email: Some("accounts@example.com".into()),
            ..cfg
        };
        assert_eq!(
            contact_address(&cfg).as_deref(),
            Some("accounts@example.com"),
            "contact_email is what the page prints, whatever the email is sent from"
        );
    }

    #[test]
    fn missing_public_base_url_names_the_setting() {
        let cfg = InvoicingConfig {
            stripe_secret_key: Some("sk_test".into()),
            r2_account_id: Some("acct".into()),
            r2_access_key: Some("ak".into()),
            r2_secret_key: Some("sk".into()),
            r2_bucket: Some("billing".into()),
            ..test_config()
        };
        let err = build_clients(cfg, "Bluepeak")
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(err.contains("public_base_url"), "got: {err}");
    }

    fn fully_configured_test_config() -> InvoicingConfig {
        InvoicingConfig {
            mailgun_domain: Some("mail.example.test".into()),
            from_email: Some("billing@example.test".into()),
            ..config_up_to_mailgun()
        }
    }

    #[test]
    fn a_scheme_less_public_base_url_is_refused_before_any_client_is_built() {
        let mut cfg = fully_configured_test_config();
        cfg.public_base_url = Some("billing.example.com".into());
        let err = build_clients(cfg, "").map(|_| ()).unwrap_err().to_string();
        assert!(err.contains("public_base_url"), "got: {err}");
    }

    #[test]
    fn a_configured_base_url_that_can_produce_a_link_still_builds() {
        assert!(build_clients(fully_configured_test_config(), "").is_ok());
    }

    fn config_up_to_mailgun() -> InvoicingConfig {
        InvoicingConfig {
            stripe_secret_key: Some("sk_test".into()),
            r2_account_id: Some("acct".into()),
            r2_access_key: Some("ak".into()),
            r2_secret_key: Some("sk".into()),
            r2_bucket: Some("billing".into()),
            public_base_url: Some("https://billing.example.test/i".into()),
            mailgun_api_key: Some("key".into()),
            ..test_config()
        }
    }

    #[test]
    fn missing_mailgun_domain_names_the_setting() {
        let err = build_clients(config_up_to_mailgun(), "Bluepeak")
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(err.contains("mailgun_domain"), "got: {err}");
    }

    #[test]
    fn missing_from_email_names_the_setting() {
        let cfg = InvoicingConfig {
            mailgun_domain: Some("mail.example.test".into()),
            ..config_up_to_mailgun()
        };
        let err = build_clients(cfg, "Bluepeak")
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(err.contains("from_email"), "got: {err}");
    }

    #[test]
    fn unknown_invoice_number_gets_a_readable_error() {
        let (_d, conn) = test_conn();
        let err = find_invoice(&conn, 9999).map(|_| ()).unwrap_err();
        assert!(matches!(err, NigelError::NotFound(_)), "got: {err:?}");
        assert!(err.to_string().contains("No invoice #9999"), "got: {err}");
    }

    #[test]
    fn find_invoice_returns_the_matching_invoice() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn);

        let invoice = find_invoice(&conn, 1248).unwrap();
        assert_eq!(invoice.number, 1248);
        assert_eq!(invoice.total, 100.0);
    }

    #[test]
    fn void_invoices_are_refused_before_any_network_call_or_payment() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn);
        conn.execute("UPDATE invoices SET status='void' WHERE id=?1", [id])
            .unwrap();
        let invoice = find_invoice(&conn, 1248).unwrap();

        let send_err = ensure_not_void(&invoice, "sent").unwrap_err();
        assert!(
            matches!(send_err, NigelError::Conflict { code: "void", .. }),
            "got: {send_err:?}"
        );
        assert!(
            send_err.to_string().contains("void and cannot be sent"),
            "got: {send_err}"
        );
        let pay_err = ensure_not_void(&invoice, "paid").unwrap_err();
        assert!(
            matches!(pay_err, NigelError::Conflict { code: "void", .. }),
            "got: {pay_err:?}"
        );
        assert!(
            pay_err.to_string().contains("void and cannot be paid"),
            "got: {pay_err}"
        );
    }

    #[test]
    fn a_voided_at_row_whose_status_is_stale_is_still_refused() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn);
        // A void whose status write did not land: the timestamp is the fact.
        conn.execute(
            "UPDATE invoices SET voided_at='2026-08-06', status='draft' WHERE id=?1",
            [id],
        )
        .unwrap();
        let invoice = find_invoice(&conn, 1248).unwrap();

        for action in ["sent", "paid"] {
            let err = ensure_not_void(&invoice, action).unwrap_err();
            assert!(
                matches!(err, NigelError::Conflict { code: "void", .. }),
                "got: {err:?}"
            );
        }
    }

    #[test]
    fn draft_invoices_are_sendable_and_payable() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn);
        let invoice = find_invoice(&conn, 1248).unwrap();
        assert!(ensure_not_void(&invoice, "sent").is_ok());
        assert!(ensure_not_void(&invoice, "paid").is_ok());
    }

    #[test]
    fn preview_paths_are_stable_and_undated() {
        let (html, pdf) = preview_paths(Path::new("/tmp/p"), 1248);
        assert_eq!(html, Path::new("/tmp/p/invoice-1248.html"));
        assert_eq!(pdf, Path::new("/tmp/p/invoice-1248.pdf"));
    }

    #[test]
    fn explicit_output_dir_wins_and_is_not_the_default() {
        let (dir, is_default) = preview_dir(Some("/tmp/elsewhere".into()));
        assert_eq!(dir, PathBuf::from("/tmp/elsewhere"));
        assert!(
            !is_default,
            "a directory the user named is not re-permissioned"
        );

        let (dir, is_default) = preview_dir(None);
        assert!(is_default && dir.ends_with("previews"), "got: {dir:?}");
    }

    #[test]
    fn neither_contact_key_becomes_a_flagged_placeholder() {
        let (value, placeholder) = contact_email_for_preview(&test_config());
        assert!(
            placeholder && value.contains("contact_email"),
            "got: {value}"
        );

        let cfg = InvoicingConfig {
            from_email: Some("billing@example.test".into()),
            ..test_config()
        };
        assert_eq!(
            contact_email_for_preview(&cfg),
            ("billing@example.test".to_string(), false)
        );
    }

    #[test]
    fn preview_requires_no_invoicing_config_at_all() {
        assert!(build_clients(test_config(), "Bluepeak").is_err()); // send cannot run
        assert!(!contact_email_for_preview(&test_config()).0.is_empty()); // preview can
    }

    /// Void's teardown asks what is configured instead of demanding everything:
    /// an installation that never set a key still voids, and the warning it
    /// prints is `invoicing::void`'s.
    #[test]
    fn nothing_configured_yields_no_gateway_and_no_publisher() {
        let cfg = test_config();
        assert!(optional_gateway(&cfg).is_none());
        assert!(optional_publisher(&cfg).is_none());
    }

    #[test]
    fn the_stripe_key_alone_yields_a_gateway_but_no_publisher() {
        let cfg = InvoicingConfig {
            stripe_secret_key: Some("sk_test".into()),
            ..test_config()
        };
        assert!(optional_gateway(&cfg).is_some());
        assert!(
            optional_publisher(&cfg).is_none(),
            "a bucketless publisher is not a publisher"
        );
    }

    /// All five R2 keys or none: four of them cannot upload four fifths of a
    /// page.
    #[test]
    fn the_publisher_needs_every_one_of_its_five_keys() {
        fn r2_config() -> InvoicingConfig {
            InvoicingConfig {
                r2_account_id: Some("acct".into()),
                r2_access_key: Some("access".into()),
                r2_secret_key: Some("secret".into()),
                r2_bucket: Some("billing".into()),
                public_base_url: Some("https://billing.example.test/i".into()),
                ..test_config()
            }
        }
        assert!(optional_publisher(&r2_config()).is_some());

        let drops: [fn(&mut InvoicingConfig); 5] = [
            |c| c.r2_account_id = None,
            |c| c.r2_access_key = None,
            |c| c.r2_secret_key = None,
            |c| c.r2_bucket = None,
            |c| c.public_base_url = None,
        ];
        for drop in drops {
            let mut cfg = r2_config();
            drop(&mut cfg);
            assert!(optional_publisher(&cfg).is_none());
        }
    }
}
