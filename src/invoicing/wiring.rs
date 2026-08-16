//! Where settings meet invoicing.
//!
//! `src/invoicing/` never reads settings — every value arrives as a parameter,
//! resolved by whichever surface is calling. These functions are the wiring
//! that assembles a send or a republish out of those values, and they live here
//! rather than in the CLI because the HTTP layer needs them too. A function
//! here that reaches for `crate::settings` has broken the rule this module
//! exists to keep.

use std::path::Path;

use rusqlite::Connection;

use crate::error::{NigelError, Result};
use crate::invoicing::clients::get_client;
use crate::invoicing::invoices::get_invoice_by_number;
use crate::invoicing::mailgun::{
    from_address_domain_warning, validate_bare_address, validate_header_value, EmailEnvelope,
    MailgunClient,
};
use crate::invoicing::r2::{public_base_url_warning, validate_public_base_url, R2Publisher};
use crate::invoicing::render_html::{load_template, Branding};
use crate::invoicing::republish::republish_invoice;
use crate::invoicing::stripe::StripeClient;
use crate::models::Invoice;
use crate::settings::InvoicingConfig;

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
pub fn company_name(conn: &Connection) -> String {
    crate::db::get_metadata(conn, "company_name").unwrap_or_default()
}

/// The whole letterhead, read from the one place it lives.
///
/// Owned, because `Branding` borrows and the values come out of the database.
/// Resolved here rather than at each `Branding` site: the fields are only ever
/// correct together, and six hand-built literals each doing their own
/// `get_metadata` calls is how a document ends up with an address and no phone.
pub struct CompanyProfile {
    pub name: String,
    pub address: String,
    pub phone: String,
    pub logo: String,
    pub payment_instructions: String,
}

pub fn company_profile(conn: &Connection) -> CompanyProfile {
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
    pub fn branding<'a>(&'a self, template: &'a str, contact_email: &'a str) -> Branding<'a> {
        Branding {
            template,
            company: &self.name,
            company_address: &self.address,
            company_phone: &self.phone,
            logo: &self.logo,
            // The self-contained page. `send` and a republish point it at the
            // hosted object through `with_logo_url`; `preview` and the API's
            // preview routes never do, which is what keeps a preview a file that
            // renders with no network and no configuration.
            logo_url: None,
            payment_instructions: &self.payment_instructions,
            contact_email,
        }
    }
}

pub(crate) fn build_gateway(cfg: &InvoicingConfig) -> Result<StripeClient> {
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
pub fn optional_gateway(cfg: &InvoicingConfig) -> Option<StripeClient> {
    Some(StripeClient {
        secret_key: cfg.stripe_secret_key.clone()?,
    })
}

/// The publisher, when every key it takes is set. All five or none: a publisher
/// missing its bucket is not a publisher that works for four fifths of a page.
pub fn optional_publisher(cfg: &InvoicingConfig) -> Option<R2Publisher> {
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
pub struct SendClients {
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
pub fn build_clients(cfg: InvoicingConfig, company: &str) -> Result<SendClients> {
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

/// The sentence a republish that could not even be attempted earns. One home,
/// so a terminal and a browser cannot word the same failure differently.
pub(crate) fn republish_warning(what: &str, e: NigelError) -> String {
    format!(
        "Warning: the payment is recorded, but the published page could not be republished \
         ({what}: {e})."
    )
}

/// The same with the invoice already loaded and the publisher supplied, which is
/// what lets the HTTP layer drive a republish against a fake, reach no network,
/// and spend one read rather than three on the hot pay path — the
/// `send_with`/`void_with` seam.
pub fn republish_with<P: crate::invoicing::gateway::AssetPublisher>(
    conn: &Connection,
    invoice: &Invoice,
    cfg: &InvoicingConfig,
    data_dir: &Path,
    publisher: Option<&P>,
) -> Vec<String> {
    // The ordinary case, and the one that must cost nothing: most payments land
    // on invoices that were never published.
    if invoice.published_at.is_none() {
        return Vec::new();
    }
    let client = match get_client(conn, invoice.client_id) {
        Ok(client) => client,
        Err(e) => return vec![republish_warning("reading the client", e)],
    };
    let template = match load_template(data_dir) {
        Ok(template) => template,
        Err(e) => return vec![republish_warning("loading the invoice template", e)],
    };

    // The preview fallback, not `require`: a republish must not depend on an
    // address being configured, since the page it is correcting is already up.
    let (contact_email, _) = contact_email_for_preview(cfg);
    let profile = company_profile(conn);
    let branding = profile.branding(&template, &contact_email);
    republish_invoice(conn, invoice, &client, &branding, publisher).warnings()
}

/// The CLI's own `republish_all` with the publisher injected, so the HTTP
/// layer's sync runs the same loop against a fake instead of keeping its own
/// copy of it — and the sentence an invoice nobody could look up earns has one
/// home.
pub fn republish_all_with<P: crate::invoicing::gateway::AssetPublisher>(
    conn: &Connection,
    numbers: &[i64],
    cfg: &InvoicingConfig,
    data_dir: &Path,
    publisher: Option<&P>,
) -> Vec<String> {
    numbers
        .iter()
        .flat_map(|number| match get_invoice_by_number(conn, *number) {
            Ok(invoice) => republish_with(conn, &invoice, cfg, data_dir, publisher),
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

/// What `{{CONTACT}}` prints when neither `contact_email` nor `from_email` is
/// configured. Preview is the one invoicing command that runs without any
/// configuration, so it renders a visible stand-in rather than refusing.
const PREVIEW_CONTACT_PLACEHOLDER: &str = "(contact_email not configured)";

pub fn contact_email_for_preview(cfg: &InvoicingConfig) -> (String, bool) {
    match contact_address(cfg) {
        Some(email) => (email, false),
        None => (PREVIEW_CONTACT_PLACEHOLDER.to_string(), true),
    }
}
