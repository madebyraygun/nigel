use std::borrow::Cow;
use std::path::{Path, PathBuf};

use crate::error::{NigelError, Result};
use crate::invoicing::document::{
    address_lines, company_block, email_line, meta_rows, parse_logo, payment_lines,
    terms_block_text, MoneySummary,
};
use crate::models::{Client, Invoice, InvoiceLineItem};

/// The page Nigel renders when the data directory holds no template of its own.
pub const DEFAULT_TEMPLATE: &str = include_str!("templates/invoice.html");

/// Every `{{KEY}}` a template may use. Anything else shaped like a placeholder
/// is a typo and is refused at load time.
pub const PLACEHOLDERS: &[&str] = &[
    "NUMBER",
    "CLIENT",
    "CLIENT_EMAIL",
    "CLIENT_EMAIL_BLOCK",
    "CLIENT_ADDRESS",
    "CLIENT_ADDRESS_BLOCK",
    "COMPANY",
    "COMPANY_ADDRESS",
    "COMPANY_PHONE",
    "COMPANY_BLOCK",
    "LOGO",
    "ISSUE",
    "DUE_DATE",
    "DUE",
    "META_ROWS",
    "ROWS",
    "CURRENCY",
    "SUBTOTAL",
    "TAX",
    "TOTAL",
    "TOTALS",
    "NOTES",
    "TERMS",
    "TERMS_BLOCK",
    "PAYMENT_INSTRUCTIONS",
    "PAYMENT_BLOCK",
    "PAY_URL",
    "PAY",
    "CONTACT",
];

/// What an invoice is: which invoice, who owes, for what, how much. A template
/// without these renders a document that is wrong about money.
///
/// **This list does not grow.** Every key added to `PLACEHOLDERS` after a
/// release is optional, so a template exported from an older Nigel keeps
/// validating and keeps rendering exactly what it always did.
const REQUIRED: &[&str] = &["NUMBER", "CLIENT", "ROWS", "TOTAL"];

/// A required key another key stands in for.
///
/// `{{TOTALS}}` is the money block, and the total is one line of it, so a
/// template printing the block says what is owed just as `{{TOTAL}}` alone
/// does — which is the whole of what `REQUIRED` asks for.
const REQUIRED_ALTERNATIVES: &[(&str, &str)] = &[("TOTAL", "TOTALS")];

fn satisfies(found: &[&str], key: &str) -> bool {
    found.contains(&key)
        || REQUIRED_ALTERNATIVES
            .iter()
            .any(|(required, instead)| *required == key && found.contains(instead))
}

const MAX_TEMPLATE_BYTES: usize = 1024 * 1024;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Expands `{{KEY}}` placeholders in a single left-to-right pass, so substituted
/// values are never re-scanned for further placeholders. Unknown placeholders are
/// emitted verbatim.
fn expand(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(open) = rest.find("{{") {
        let after_open = &rest[open + 2..];
        match after_open.find("}}").and_then(|close| {
            vars.iter()
                .find(|(k, _)| *k == &after_open[..close])
                .map(|(_, v)| (close, *v))
        }) {
            Some((close, value)) => {
                out.push_str(&rest[..open]);
                out.push_str(value);
                rest = &after_open[close + 2..];
            }
            None => {
                out.push_str(&rest[..open + 2]);
                rest = after_open;
            }
        }
    }

    out.push_str(rest);
    out
}

/// The placeholder keys `source` uses, in order, once per occurrence. The scan
/// is deliberately narrow — `{{` + SCREAMING_SNAKE + `}}` and nothing else — so
/// a CSS brace or a `{{ not a key }}` aside is literal text rather than a
/// validation failure.
fn placeholder_tokens(source: &str) -> Vec<&str> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;

    while let Some(open) = source[at..].find("{{") {
        let start = at + open + 2;
        let mut end = start;
        while end < bytes.len()
            && (bytes[end].is_ascii_uppercase()
                || bytes[end] == b'_'
                || (end > start && bytes[end].is_ascii_digit()))
        {
            end += 1;
        }
        if end > start && source[end..].starts_with("}}") {
            out.push(&source[start..end]);
            at = end + 2;
        } else {
            at = start;
        }
    }
    out
}

fn braced(keys: &[&str]) -> String {
    keys.iter()
        .map(|k| format!("{{{{{k}}}}}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Checked when a template is loaded, never when one is rendered, so a typo
/// surfaces on `invoice template path` or `invoice preview` rather than in a
/// client's inbox. `path` is named in every failure.
fn validate_template(source: &str, path: &Path) -> Result<()> {
    if source.len() > MAX_TEMPLATE_BYTES {
        return Err(NigelError::Invalid(format!(
            "Invoice template {} is {} bytes; the limit is 1 MiB.",
            path.display(),
            source.len()
        )));
    }
    if source.trim().is_empty() {
        return Err(NigelError::Invalid(format!(
            "Invoice template {} is empty.",
            path.display()
        )));
    }

    let found = placeholder_tokens(source);

    let missing: Vec<&str> = REQUIRED
        .iter()
        .filter(|k| !satisfies(&found, k))
        .copied()
        .collect();
    if !missing.is_empty() {
        return Err(NigelError::Invalid(format!(
            "Invoice template {} is missing required placeholder(s): {}. Known placeholders: {}.",
            path.display(),
            braced(&missing),
            braced(PLACEHOLDERS)
        )));
    }

    let mut unknown: Vec<&str> = Vec::new();
    for key in found {
        if !PLACEHOLDERS.contains(&key) && !unknown.contains(&key) {
            unknown.push(key);
        }
    }
    if !unknown.is_empty() {
        return Err(NigelError::Invalid(format!(
            "Invoice template {} uses unknown placeholder(s): {}. Known placeholders: {}.",
            path.display(),
            braced(&unknown),
            braced(PLACEHOLDERS)
        )));
    }

    Ok(())
}

/// Where Nigel looks for an operator's own invoice page.
pub fn template_path(data_dir: &Path) -> PathBuf {
    data_dir.join("templates").join("invoice.html")
}

/// The operator's template when the file is there and valid, the embedded
/// default when it is not there at all. A file that exists but cannot be read
/// or does not validate is an error naming the path — never a silent fallback,
/// because the stock page would then reach a client nobody chose to send it to.
pub fn load_template(data_dir: &Path) -> Result<Cow<'static, str>> {
    let path = template_path(data_dir);
    let read_error = |e: std::io::Error| {
        NigelError::Invalid(format!(
            "Cannot read invoice template {}: {e}",
            path.display()
        ))
    };

    // Only a genuine "no such file" is an absent override. `Path::exists` would
    // answer false for a dangling symlink or an unreadable directory too, and
    // each of those would then render the stock page for someone who put a
    // template there on purpose.
    match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Cow::Borrowed(DEFAULT_TEMPLATE))
        }
        Err(e) => return Err(read_error(e)),
        Ok(_) => {}
    }

    // Sized before it is read, so a wrong file copied over the template cannot
    // be pulled into memory whole just to be rejected.
    let size = std::fs::metadata(&path).map_err(read_error)?.len();
    if size > MAX_TEMPLATE_BYTES as u64 {
        return Err(NigelError::Invalid(format!(
            "Invoice template {} is {size} bytes; the limit is 1 MiB.",
            path.display()
        )));
    }

    let source = std::fs::read_to_string(&path).map_err(read_error)?;
    validate_template(&source, &path)?;
    Ok(Cow::Owned(source))
}

/// Which pay element the page carries.
pub enum PayButton<'a> {
    /// A live payment link, as a sent invoice renders it.
    Link(&'a str),
    /// A draft that gets its link when it is sent: an inert stand-in showing
    /// where the button goes, with nothing to click.
    Placeholder,
    /// No link, and none coming — a void invoice, or a page that never had one.
    Omitted,
}

/// What the page says about the sender, as opposed to what it says about the
/// invoice. `src/invoicing/` reads no settings and no database, so the CLI layer
/// resolves the whole letterhead and passes it in.
///
/// Every field is a borrowed `&str` and empty means unset, so a caller with
/// nothing to say says nothing rather than reaching for an `Option`.
/// `Default` is derived because the struct is built in a couple of dozen test
/// literals, and a field appended to each of them by hand is a field that ends
/// up meaning different things in different tests; production sites still name
/// every field.
#[derive(Default)]
pub struct Branding<'a> {
    pub template: &'a str,
    pub company: &'a str,
    pub company_address: &'a str,
    pub company_phone: &'a str,
    /// The stored `company_logo` data URI. Parsed by the render seam, once, for
    /// both documents.
    pub logo: &'a str,
    /// The operator's own payment instructions, multi-line.
    pub payment_instructions: &'a str,
    pub contact_email: &'a str,
}

/// The page that replaces a published invoice when it is voided.
///
/// It takes no template and no configuration: the invoice template is the
/// operator's to edit, and a voided page that could fail to render — a broken
/// override, a missing `from_email` — would leave a live Pay button up because
/// the notice replacing it did not compile. The only value on it is the number,
/// which is an `i64` and so cannot carry markup. No figures, no client name and
/// no pay button: whoever opens this address may be anyone the link was
/// forwarded to, and a cancelled invoice owes them one fact.
pub fn voided_page_html(number: i64) -> String {
    format!(
        "<!doctype html>\n\
         <html><head><meta charset=\"utf-8\"><title>Invoice {number} — voided</title>\n\
         <style>body{{font-family:system-ui,sans-serif;max-width:40rem;margin:2rem auto;padding:0 1rem}}</style>\n\
         </head><body>\n\
         <h1>This invoice has been voided</h1>\n\
         <p>Invoice #{number} was cancelled and is no longer payable. Please contact the sender if you were expecting to pay it.</p>\n\
         </body></html>\n"
    )
}

pub fn render_invoice_html(
    branding: &Branding<'_>,
    invoice: &Invoice,
    client: &Client,
    items: &[InvoiceLineItem],
    money: &MoneySummary,
    pay: PayButton<'_>,
) -> String {
    let rows: String = items
        .iter()
        .map(|i| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{:.2}</td><td>{:.2}</td></tr>",
                esc(&i.description),
                i.quantity,
                i.unit_amount,
                i.line_total
            )
        })
        .collect();

    // The placeholder styles itself inline instead of adding a rule to
    // `templates/invoice.html`, so it renders correctly against a custom
    // template that knows nothing about a `.pay-placeholder` class. The grey
    // carries `.pay`'s white text at 4.5:1, the WCAG AA floor.
    let pay_url = match &pay {
        PayButton::Link(url) => *url,
        PayButton::Placeholder | PayButton::Omitted => "",
    };
    let pay = match pay {
        PayButton::Link(url) => format!("<a class=\"pay\" href=\"{}\">Pay online</a>", esc(url)),
        PayButton::Placeholder => "<span class=\"pay pay-placeholder\" style=\"background:#767676;cursor:default\">Pay online — link created when the invoice is sent</span>".to_string(),
        PayButton::Omitted => String::new(),
    };
    let due_date = invoice.due_date.as_deref().map(esc).unwrap_or_default();
    let due = match due_date.as_str() {
        "" => String::new(),
        date => format!("<br>Due: {date}"),
    };

    // A block rather than a bare value, so "empty when unset" needs no
    // conditional in a template language that has none.
    let block = |heading: &str, text: Option<&String>| match text {
        Some(t) if !t.trim().is_empty() => format!("<h3>{heading}</h3><p>{}</p>", esc(t)),
        _ => String::new(),
    };

    // The From block, whole. It carries its own wrapper and its own label so
    // that an installation with no letterhead renders nothing at all — a
    // "From" heading over an empty rule is the thing this avoids, and a `:empty`
    // CSS rule would not survive an operator's own template.
    let company = company_block(
        branding.company,
        branding.company_address,
        branding.company_phone,
    );
    let company_block = if company.is_empty() {
        String::new()
    } else {
        let mut body = String::new();
        if !company.name.is_empty() {
            body.push_str(&format!("<strong>{}</strong>", esc(company.name)));
        }
        for line in &company.address {
            body.push_str(&format!("<br>{}", esc(line)));
        }
        if let Some(phone) = company.phone {
            body.push_str(&format!("<br>ph. {}", esc(phone)));
        }
        // A leading `<br>` when there is no name at all: the block starts on
        // its first line either way.
        let body = body.strip_prefix("<br>").unwrap_or(&body).to_string();
        format!(
            "<div class=\"party from\"><span class=\"party-label\">From</span>\
             <div class=\"party-body\">{body}</div></div>"
        )
    };

    // The image the operator configured, or nothing. A stored value that cannot
    // be used renders no `<img>` at all rather than a broken one, and never an
    // error: a logo is decoration on a document about money.
    let logo = match parse_logo(branding.logo) {
        Ok(Some(logo)) => format!(
            "<img class=\"logo\" src=\"data:{};base64,{}\" alt=\"{}\">",
            logo.mime,
            esc(&logo.base64),
            esc(branding.company.trim())
        ),
        Ok(None) | Err(_) => String::new(),
    };

    let meta_rows: String = meta_rows(invoice)
        .iter()
        .map(|row| {
            let class = if row.emphasis {
                " class=\"strong\""
            } else {
                ""
            };
            format!(
                "<tr><th>{}</th><td{class}>{}</td></tr>",
                row.label,
                esc(&row.value)
            )
        })
        .collect();

    let terms_block = match terms_block_text(invoice) {
        Some(terms) => format!("<h3>Terms</h3><p>{}</p>", esc(terms)),
        None => String::new(),
    };

    // One paragraph, one line per typed line. Both documents print exactly
    // these lines, and an installation that takes no bank transfers prints no
    // heading either.
    let payment_block = match payment_lines(branding.payment_instructions) {
        lines if lines.is_empty() => String::new(),
        lines => format!(
            "<h3>Payment</h3><p>{}</p>",
            lines
                .iter()
                .map(|line| esc(line))
                .collect::<Vec<_>>()
                .join("<br>")
        ),
    };
    // One `<br>`-prefixed line per typed line, so a two-line address stays two
    // lines and an absent one contributes nothing at all — not even a break.
    let address_block: String = client
        .billing_address
        .as_deref()
        .map(|address| {
            address_lines(address)
                .iter()
                .map(|line| format!("<br>{}", esc(line)))
                .collect()
        })
        .unwrap_or_default();
    let email_block = match email_line(client.email.as_deref()) {
        Some(email) => format!("<br>{}", esc(email)),
        None => String::new(),
    };

    // The money block, from the one function that decides which lines exist.
    // The PDF asks the same question, which is what keeps the two documents
    // from disagreeing about the same invoice.
    let currency = esc(&invoice.currency);
    let totals: String = money
        .lines()
        .iter()
        .map(|line| {
            let class = if line.emphasis {
                " class=\"total\""
            } else {
                ""
            };
            format!(
                "<tr{class}><td colspan=\"3\">{}</td><td>{currency} {:.2}</td></tr>",
                line.label, line.amount
            )
        })
        .collect();

    expand(
        branding.template,
        &[
            ("NUMBER", &invoice.number.to_string()),
            ("CLIENT", &esc(&client.name)),
            (
                "CLIENT_EMAIL",
                &esc(client.email.as_deref().unwrap_or_default()),
            ),
            ("CLIENT_EMAIL_BLOCK", &email_block),
            (
                "CLIENT_ADDRESS",
                &esc(client.billing_address.as_deref().unwrap_or_default()),
            ),
            ("CLIENT_ADDRESS_BLOCK", &address_block),
            ("COMPANY", &esc(branding.company)),
            ("COMPANY_ADDRESS", &esc(branding.company_address)),
            ("COMPANY_PHONE", &esc(branding.company_phone)),
            ("COMPANY_BLOCK", &company_block),
            ("LOGO", &logo),
            ("ISSUE", &esc(&invoice.issue_date)),
            ("DUE_DATE", &due_date),
            ("DUE", &due),
            ("META_ROWS", &meta_rows),
            ("ROWS", &rows),
            ("CURRENCY", &currency),
            ("SUBTOTAL", &format!("{:.2}", invoice.subtotal)),
            ("TAX", &format!("{:.2}", invoice.tax)),
            ("TOTAL", &format!("{:.2}", invoice.total)),
            ("TOTALS", &totals),
            ("NOTES", &block("Notes", invoice.notes.as_ref())),
            ("TERMS", &block("Terms", invoice.terms.as_ref())),
            ("TERMS_BLOCK", &terms_block),
            ("PAYMENT_INSTRUCTIONS", &esc(branding.payment_instructions)),
            ("PAYMENT_BLOCK", &payment_block),
            ("PAY_URL", &esc(pay_url)),
            ("PAY", &pay),
            ("CONTACT", &esc(branding.contact_email)),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Client, Invoice, InvoiceLineItem};

    /// The money block for an invoice nobody has paid against, which is what
    /// every test that is about something else wants.
    fn money(invoice: &Invoice) -> MoneySummary {
        MoneySummary::of(invoice, 0.0)
    }

    fn sample() -> (Invoice, Client, Vec<InvoiceLineItem>) {
        let inv = Invoice {
            id: 1,
            number: 1248,
            client_id: 1,
            issue_date: "2026-08-04".into(),
            due_date: Some("2026-09-03".into()),
            status: "sent".into(),
            currency: "USD".into(),
            subtotal: 250.0,
            tax: 0.0,
            total: 250.0,
            notes: None,
            terms: None,
            token: "abc123".into(),
            stripe_payment_link_id: None,
            stripe_payment_link_url: None,
            published_at: Some("2026-08-04".into()),
            voided_at: None,
        };
        let client = Client {
            id: 1,
            name: "Acme <Co>".into(),
            email: Some("a@b.test".into()),
            billing_address: None,
            notes: None,
            archived_at: None,
        };
        let items = vec![InvoiceLineItem {
            id: None,
            invoice_id: Some(1),
            description: "Design".into(),
            quantity: 2.0,
            unit_amount: 100.0,
            line_total: 200.0,
            position: 0,
        }];
        (inv, client, items)
    }

    #[test]
    fn renders_number_total_items_and_pay_button() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand("billing@example.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Link("https://pay.stripe.test/x"),
        );
        assert!(html.contains("1248"));
        assert!(html.contains("Design"));
        assert!(html.contains("250.00"));
        assert!(html.contains("https://pay.stripe.test/x"));
        assert!(html.contains("Acme &lt;Co&gt;")); // escaped
    }

    /// `{{CONTACT}}` shipped and keeps its exact meaning. Only the stock page
    /// stopped using it — payment instructions are the operator's own text now.
    #[test]
    fn the_contact_placeholder_still_expands_to_the_contact_address() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand_with(
                "[{{CONTACT}}]{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}",
                "ap@acme.test",
            ),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(html.starts_with("[ap@acme.test]"), "got: {html}");
    }

    #[test]
    fn contact_email_is_escaped() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand_with(
                "{{CONTACT}}{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}",
                "<script>alert(1)</script>",
            ),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn client_name_containing_a_placeholder_stays_literal_text() {
        let (inv, mut client, items) = sample();
        client.name = "Acme {{ROWS}} {{PAY}} Co".into();
        let html = render_invoice_html(
            &brand("billing@example.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Link("https://pay.stripe.test/x"),
        );
        assert!(html.contains("Acme {{ROWS}} {{PAY}} Co"));
        assert_eq!(html.matches("Design").count(), 1);
        assert_eq!(html.matches("Pay online").count(), 1);
    }

    #[test]
    fn pay_url_cannot_break_out_of_the_href_attribute() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand("billing@example.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Link("https://pay.stripe.test/x\"onmouseover=\"alert(1)"),
        );
        assert!(html.contains("&quot;onmouseover"));
        assert!(!html.contains("\"onmouseover"));
    }

    #[test]
    fn placeholder_renders_an_inert_span_not_a_link() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand("b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Placeholder,
        );
        assert!(html.contains("<span class=\"pay pay-placeholder\""));
        assert!(
            html.contains("link created when the invoice is sent"),
            "got: {html}"
        );
        assert!(!html.contains("<a class=\"pay\""));
        assert!(
            !html.contains("href"),
            "a placeholder must not be clickable"
        );
    }

    #[test]
    fn only_the_pay_element_differs_between_link_and_placeholder() {
        let (inv, client, items) = sample();
        let linked = render_invoice_html(
            &brand("b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Link("https://pay.test/x"),
        );
        let pending = render_invoice_html(
            &brand("b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Placeholder,
        );
        // `{{PAY}}` sits alone on its own line in the template, which is what
        // lets a line filter isolate it. If that moves, this test is what notices.
        let strip = |s: &str| {
            s.lines()
                .filter(|l| !l.contains("class=\"pay"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert_eq!(strip(&linked), strip(&pending));
    }

    const MINIMAL: &str = "<p>{{NUMBER}} {{CLIENT}} {{ROWS}} {{TOTAL}}</p>";

    fn brand(contact_email: &str) -> Branding<'_> {
        Branding {
            template: DEFAULT_TEMPLATE,
            company: "",
            contact_email,
            ..Branding::default()
        }
    }

    fn brand_with<'a>(template: &'a str, contact_email: &'a str) -> Branding<'a> {
        Branding {
            template,
            company: "",
            contact_email,
            ..Branding::default()
        }
    }

    fn write_override(dir: &std::path::Path, source: &str) {
        let path = template_path(dir);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
    }

    #[test]
    fn template_path_is_templates_invoice_html() {
        assert_eq!(
            template_path(Path::new("/books")),
            Path::new("/books/templates/invoice.html")
        );
    }

    #[test]
    fn no_override_falls_back_to_the_embedded_default() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = load_template(dir.path()).unwrap();
        assert!(matches!(loaded, std::borrow::Cow::Borrowed(_)));
        assert_eq!(loaded, DEFAULT_TEMPLATE);
    }

    #[test]
    fn an_override_file_wins_over_the_default() {
        let dir = tempfile::tempdir().unwrap();
        write_override(dir.path(), MINIMAL);
        assert_eq!(load_template(dir.path()).unwrap(), MINIMAL);
    }

    #[test]
    fn an_unreadable_override_errors_naming_the_path() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(template_path(dir.path())).unwrap();
        let err = load_template(dir.path()).unwrap_err().to_string();
        assert!(
            err.contains(&template_path(dir.path()).display().to_string()),
            "got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlink_errors_rather_than_falling_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = template_path(dir.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(dir.path().join("elsewhere.html"), &path).unwrap();

        let loaded = load_template(dir.path());
        assert!(
            loaded.is_err(),
            "a template symlinked to nothing is a broken override, not an absent one"
        );
        assert!(loaded
            .unwrap_err()
            .to_string()
            .contains(&path.display().to_string()));
    }

    #[test]
    fn an_invalid_override_errors_rather_than_falling_back() {
        let dir = tempfile::tempdir().unwrap();
        write_override(dir.path(), "<p>hello</p>");
        let loaded = load_template(dir.path());
        assert!(loaded.is_err(), "a broken override must never render");
        assert!(
            loaded.unwrap_err().to_string().contains("{{NUMBER}}"),
            "the failure must name what is missing"
        );
    }

    #[test]
    fn the_default_template_validates() {
        assert!(validate_template(DEFAULT_TEMPLATE, Path::new("/tmp/t.html")).is_ok());
    }

    #[test]
    fn an_empty_or_whitespace_template_is_rejected() {
        for source in ["", "\n \t\n"] {
            let err = validate_template(source, Path::new("/tmp/t.html"))
                .unwrap_err()
                .to_string();
            assert!(err.contains("is empty"), "got: {err}");
            assert!(err.contains("/tmp/t.html"), "got: {err}");
        }
    }

    #[test]
    fn an_oversized_template_is_rejected() {
        let source = "x".repeat(MAX_TEMPLATE_BYTES + 1);
        let err = validate_template(&source, Path::new("/tmp/t.html"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("1 MiB"), "got: {err}");
        assert!(err.contains("/tmp/t.html"), "got: {err}");
    }

    /// Either form of the money block satisfies the requirement; neither is a
    /// refusal, and a page with no figure at all still is.
    #[test]
    fn the_totals_block_stands_in_for_the_bare_total() {
        let path = Path::new("/tmp/t.html");
        assert!(
            validate_template("<p>{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTALS}}</p>", path).is_ok(),
            "the money block says what is owed"
        );
        assert!(validate_template("<p>{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}</p>", path).is_ok());

        let err = validate_template("<p>{{NUMBER}}{{CLIENT}}{{ROWS}}</p>", path)
            .unwrap_err()
            .to_string();
        assert!(err.contains("{{TOTAL}}"), "got: {err}");
    }

    #[test]
    fn a_template_missing_a_required_placeholder_is_rejected() {
        let err = validate_template(
            "<p>{{NUMBER}} {{CLIENT}} {{ROWS}}</p>",
            Path::new("/tmp/t.html"),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("missing required"), "got: {err}");
        assert!(err.contains("{{TOTAL}}"), "got: {err}");
    }

    #[test]
    fn a_template_with_an_unknown_placeholder_is_rejected() {
        let err = validate_template(
            "<p>{{NUMBER}} {{CLIENT}} {{ROWS}} {{TOTAL}} {{TOTL}}</p>",
            Path::new("/tmp/t.html"),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("{{TOTL}}"), "got: {err}");
        assert!(
            err.contains("{{NUMBER}}"),
            "the known list is missing: {err}"
        );
    }

    #[test]
    fn non_placeholder_braces_are_left_alone() {
        let source = "{{ not a key }} {{lower}} {{ }} {{ {{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}";
        assert!(validate_template(source, Path::new("/tmp/t.html")).is_ok());
    }

    #[test]
    fn placeholder_tokens_finds_each_key_once_per_occurrence() {
        assert_eq!(
            placeholder_tokens("{{NUMBER}} x {{NUMBER}} {{ROWS}} {{lower}} {{"),
            vec!["NUMBER", "NUMBER", "ROWS"]
        );
    }

    #[test]
    fn omits_pay_button_when_no_url() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand("billing@example.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(!html.contains("Pay online"));
    }

    #[test]
    fn a_custom_template_renders_instead_of_the_default() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand_with("<h1>{{NUMBER}}</h1>{{CLIENT}}{{ROWS}}{{TOTAL}}", "b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(html.starts_with("<h1>1248</h1>"), "got: {html}");
        assert!(!html.contains("Direct deposit"));
    }

    #[test]
    fn a_client_name_that_looks_like_a_placeholder_stays_literal_in_a_custom_template() {
        let (inv, mut client, items) = sample();
        client.name = "Acme {{ROWS}} {{PAY}} Co".into();
        let html = render_invoice_html(
            &brand_with(MINIMAL, "b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Link("https://pay.test/x"),
        );
        assert!(html.contains("Acme {{ROWS}} {{PAY}} Co"), "got: {html}");
        assert_eq!(html.matches("<tr>").count(), items.len());
    }

    #[test]
    fn a_custom_template_can_place_a_value_in_a_quoted_attribute() {
        let (inv, mut client, items) = sample();
        client.name = r#"a" onmouseover="x"#.into();
        let html = render_invoice_html(
            &brand_with(
                r#"<span title="{{CLIENT}}">{{NUMBER}}{{ROWS}}{{TOTAL}}</span>"#,
                "b@e.test",
            ),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(html.contains("&quot;"), "got: {html}");
        assert!(!html.contains(r#"" onmouseover=""#), "got: {html}");
    }

    #[test]
    fn company_renders_and_is_escaped() {
        let (inv, client, items) = sample();
        let branding = Branding {
            template: "<h1>{{COMPANY}}</h1>{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}",
            company: "A & B <Co>",
            contact_email: "b@e.test",
            ..Branding::default()
        };
        let html = render_invoice_html(
            &branding,
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(html.contains("A &amp; B &lt;Co&gt;"), "got: {html}");
    }

    #[test]
    fn company_renders_empty_when_unset() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand_with(
                "<h1>{{COMPANY}}</h1>{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}",
                "b@e.test",
            ),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(html.starts_with("<h1></h1>"), "got: {html}");
    }

    /// Every fragment placed on one line, so a test can read exactly what a
    /// block expanded to and exactly what it did not.
    const FRAGMENTS: &str =
        "[{{COMPANY_BLOCK}}][{{CLIENT_ADDRESS_BLOCK}}][{{CLIENT_EMAIL_BLOCK}}][{{TOTALS}}]\
         {{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}";

    #[test]
    fn the_company_block_carries_the_name_and_disappears_without_one() {
        let (inv, client, items) = sample();
        let branding = Branding {
            template: FRAGMENTS,
            company: "A & B <Co>",
            contact_email: "b@e.test",
            ..Branding::default()
        };
        let html = render_invoice_html(
            &branding,
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(
            html.starts_with(
                "[<div class=\"party from\"><span class=\"party-label\">From</span>\
                 <div class=\"party-body\"><strong>A &amp; B &lt;Co&gt;</strong></div></div>]"
            ),
            "got: {html}"
        );

        let html = render_invoice_html(
            &brand_with(FRAGMENTS, "b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(html.starts_with("[]"), "got: {html}");
    }

    #[test]
    fn the_company_block_is_the_whole_from_block() {
        let (inv, client, items) = sample();
        let branding = Branding {
            template: FRAGMENTS,
            company: "Bluepeak LLC",
            company_address: "P.O. Box 1234\nSpringfield, CA 90001",
            company_phone: "619.555.0123",
            contact_email: "b@e.test",
            ..Branding::default()
        };
        let html = render_invoice_html(
            &branding,
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(
            html.contains("<strong>Bluepeak LLC</strong>"),
            "got: {html}"
        );
        assert!(html.contains("<br>P.O. Box 1234"), "got: {html}");
        assert!(html.contains("<br>Springfield, CA 90001"), "got: {html}");
        assert!(html.contains("<br>ph. 619.555.0123"), "got: {html}");
    }

    #[test]
    fn the_from_block_omits_the_lines_it_does_not_have() {
        let (inv, client, items) = sample();
        let branding = Branding {
            template: FRAGMENTS,
            company: "Bluepeak LLC",
            contact_email: "b@e.test",
            ..Branding::default()
        };
        let html = render_invoice_html(
            &branding,
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(
            html.starts_with(
                "[<div class=\"party from\"><span class=\"party-label\">From</span>\
                 <div class=\"party-body\"><strong>Bluepeak LLC</strong></div></div>]"
            ),
            "no empty address rows and no bare ph.: {html}"
        );
    }

    /// A phone but no name is a letterhead too, and its first line must not be
    /// a stray break.
    #[test]
    fn a_from_block_with_no_name_still_starts_on_its_first_line() {
        let (inv, client, items) = sample();
        let branding = Branding {
            template: FRAGMENTS,
            company_phone: "619.555.0123",
            contact_email: "b@e.test",
            ..Branding::default()
        };
        let html = render_invoice_html(
            &branding,
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(
            html.contains("<div class=\"party-body\">ph. 619.555.0123</div>"),
            "got: {html}"
        );
    }

    #[test]
    fn a_company_address_containing_markup_is_text() {
        let (inv, client, items) = sample();
        let branding = Branding {
            template: FRAGMENTS,
            company: "Bluepeak",
            company_address: "<script>alert(1)</script>",
            contact_email: "b@e.test",
            ..Branding::default()
        };
        let html = render_invoice_html(
            &branding,
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(html.contains("&lt;script&gt;"), "got: {html}");
        assert!(!html.contains("<script>"), "got: {html}");
    }

    /// A 2x1 PNG as a data URI. The renderer neither decodes nor draws it; it
    /// has to be a real one only because `parse_logo` checks.
    fn png_data_uri() -> String {
        use base64::Engine as _;
        let png: &[u8] = &[
            0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H',
            b'D', b'R', 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        )
    }

    fn logo_html(logo: &str) -> String {
        let (inv, client, items) = sample();
        let branding = Branding {
            template: "[{{LOGO}}]{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}",
            company: "Bluepeak <LLC>",
            logo,
            contact_email: "b@e.test",
            ..Branding::default()
        };
        render_invoice_html(
            &branding,
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        )
    }

    #[test]
    fn the_logo_is_an_img_with_the_company_name_as_its_alt_text() {
        let uri = png_data_uri();
        let html = logo_html(&uri);
        assert!(
            html.contains(&format!("<img class=\"logo\" src=\"{uri}\"")),
            "the src is the stored data URI: {html}"
        );
        assert!(
            html.contains("alt=\"Bluepeak &lt;LLC&gt;\""),
            "a Gmail reader sees the name: {html}"
        );
    }

    #[test]
    fn no_logo_renders_no_img() {
        assert!(logo_html("").starts_with("[]"), "got: {}", logo_html(""));
    }

    /// A stored value that cannot be used is no `<img>` at all — never a broken
    /// one, and never a failed render.
    #[test]
    fn an_unusable_logo_renders_no_img_rather_than_failing() {
        for bad in [
            "data:image/png;base64,!!!",
            "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=",
            "https://example.test/logo.png",
            "data:image/png;base64,bm90IGEgcG5n",
        ] {
            let html = logo_html(bad);
            assert!(html.starts_with("[]"), "{bad} rendered something: {html}");
        }
    }

    #[test]
    fn the_meta_rows_are_table_rows_in_the_shared_order() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand_with(
                "[{{META_ROWS}}]{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}",
                "b@e.test",
            ),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        let expected: String = crate::invoicing::document::meta_rows(&inv)
            .iter()
            .map(|row| {
                let class = if row.emphasis {
                    " class=\"strong\""
                } else {
                    ""
                };
                format!(
                    "<tr><th>{}</th><td{class}>{}</td></tr>",
                    row.label, row.value
                )
            })
            .collect();
        assert!(html.starts_with(&format!("[{expected}]")), "got: {html}");
        assert!(
            html.contains("<th>Invoice ID</th><td class=\"strong\">1248</td>"),
            "the number is what a client quotes back: {html}"
        );
    }

    #[test]
    fn a_due_date_with_terms_reads_as_one_value() {
        let (mut inv, client, items) = sample();
        inv.terms = Some("Net 30".into());
        let html = render_invoice_html(
            &brand_with(
                "[{{META_ROWS}}][{{TERMS_BLOCK}}]{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}",
                "b@e.test",
            ),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(
            html.contains("<th>Due Date</th><td>2026-09-03 (Net 30)</td>"),
            "one cell, not two rows: {html}"
        );
        assert!(
            html.contains("][]"),
            "the terms rode beside the date: {html}"
        );
    }

    #[test]
    fn multi_line_terms_are_a_block_beside_a_bare_due_date() {
        let (mut inv, client, items) = sample();
        inv.terms = Some("Net 30\nLate fees after 60 days.".into());
        let html = render_invoice_html(
            &brand_with(
                "[{{META_ROWS}}][{{TERMS_BLOCK}}]{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}",
                "b@e.test",
            ),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(
            html.contains("<th>Due Date</th><td>2026-09-03</td>"),
            "no parenthetical paragraph: {html}"
        );
        assert!(html.contains("<h3>Terms</h3>"), "got: {html}");
        assert!(html.contains("Late fees after 60 days."), "got: {html}");
    }

    fn payment_html(instructions: &str) -> String {
        let (inv, client, items) = sample();
        let branding = Branding {
            template: "[{{PAYMENT_BLOCK}}][{{PAYMENT_INSTRUCTIONS}}]\
                       {{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}",
            payment_instructions: instructions,
            contact_email: "b@e.test",
            ..Branding::default()
        };
        render_invoice_html(
            &branding,
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        )
    }

    #[test]
    fn the_payment_block_is_the_configured_text_one_line_per_line() {
        let html = payment_html("Wells Fargo\nRouting 121000248\nAccount 1234567890");
        assert!(
            html.starts_with(
                "[<h3>Payment</h3><p>Wells Fargo<br>Routing 121000248<br>Account 1234567890</p>]"
            ),
            "got: {html}"
        );
    }

    #[test]
    fn no_payment_instructions_render_no_payment_block() {
        assert!(payment_html("").starts_with("[][]"), "{}", payment_html(""));
        // `{{PAYMENT_INSTRUCTIONS}}` is the raw escaped value an author placed,
        // so whitespace stays whitespace there; the *block* is what disappears.
        assert!(
            payment_html("  \n ").starts_with("[]"),
            "{}",
            payment_html("  \n ")
        );
    }

    #[test]
    fn payment_instructions_containing_markup_are_text() {
        let html = payment_html("<script>alert(1)</script>");
        assert!(html.contains("&lt;script&gt;"), "got: {html}");
        assert!(!html.contains("<script>"), "got: {html}");
    }

    #[test]
    fn the_client_address_block_is_br_joined_and_escaped() {
        let (inv, mut client, items) = sample();
        client.billing_address = Some("123 <Main> St\nSpringfield".into());
        let html = render_invoice_html(
            &brand_with(FRAGMENTS, "b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(
            html.contains("[<br>123 &lt;Main&gt; St<br>Springfield]"),
            "got: {html}"
        );
    }

    #[test]
    fn the_client_email_block_carries_the_address() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand_with(FRAGMENTS, "b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(html.contains("[<br>a@b.test]"), "got: {html}");
    }

    #[test]
    fn an_absent_address_or_email_renders_nothing_at_all() {
        let (inv, mut client, items) = sample();
        client.email = None;
        client.billing_address = Some("   \n  ".into());

        let html = render_invoice_html(
            &brand_with(FRAGMENTS, "b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        // No stray `<br>`, no empty paragraph, no label with nothing after it.
        assert!(html.starts_with("[][][]["), "got: {html}");
    }

    #[test]
    fn the_totals_fragment_is_table_rows_in_the_shared_order() {
        let (inv, client, items) = sample();
        let summary = MoneySummary::of(&inv, 100.0);
        let html = render_invoice_html(
            &brand_with(FRAGMENTS, "b@e.test"),
            &inv,
            &client,
            &items,
            &summary,
            PayButton::Omitted,
        );

        let totals = html
            .split("][")
            .nth(3)
            .expect("the totals fragment")
            .to_string();
        // Same labels, same order as the one function that decides them.
        let labels: Vec<&str> = summary.lines().iter().map(|l| l.label).collect();
        assert_eq!(labels, vec!["Total", "Paid", "Balance due"]);
        let mut at = 0;
        for label in &labels {
            let found = totals[at..]
                .find(label)
                .unwrap_or_else(|| panic!("{label} missing or out of order in: {totals}"));
            at += found + label.len();
        }
        for emphasised in [
            "<tr class=\"total\"><td colspan=\"3\">Total</td><td>USD 250.00</td></tr>",
            "<tr class=\"total\"><td colspan=\"3\">Balance due</td><td>USD 150.00</td></tr>",
        ] {
            assert!(
                totals.contains(emphasised),
                "the emphasised rows carry the existing class: {totals}"
            );
        }
        assert!(
            totals.contains("<tr><td colspan=\"3\">Paid</td><td>USD 100.00</td></tr>"),
            "Paid is not the line the eye lands on: {totals}"
        );
    }

    #[test]
    fn an_overpaid_invoice_shows_a_credit_and_never_a_negative_balance() {
        let (inv, client, items) = sample();
        let summary = MoneySummary::of(&inv, 300.0);
        let html = render_invoice_html(
            &brand_with(FRAGMENTS, "b@e.test"),
            &inv,
            &client,
            &items,
            &summary,
            PayButton::Omitted,
        );
        assert!(html.contains("Balance due</td><td>USD 0.00"), "got: {html}");
        assert!(html.contains("Credit</td><td>USD 50.00"), "got: {html}");
        assert!(!html.contains("USD -"), "no negative figure: {html}");
    }

    #[test]
    fn a_very_long_address_block_is_clamped_the_way_the_pdf_clamps_it() {
        let (inv, mut client, items) = sample();
        client.billing_address = Some(
            (1..=12)
                .map(|n| format!("Line {n}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let html = render_invoice_html(
            &brand_with(FRAGMENTS, "b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );

        assert!(html.contains("<br>Line 1<br>"), "got: {html}");
        assert!(!html.contains("Line 12"), "clamped: {html}");
        let block = html.split("][").nth(1).expect("the address fragment");
        assert_eq!(
            block.matches("<br>").count(),
            crate::invoicing::document::MAX_ADDRESS_LINES,
            "one break per drawn line, and no more: {block}"
        );
        assert!(
            html.contains(crate::invoicing::document::ADDRESS_TRUNCATED),
            "the cut is shown: {html}"
        );
    }

    #[test]
    fn the_bare_text_keys_did_not_change_meaning() {
        let (mut inv, mut client, items) = sample();
        inv.subtotal = 240.0;
        inv.tax = 10.0;
        client.billing_address = Some("123 Main St".into());

        let branding = Branding {
            template: "[{{SUBTOTAL}}][{{TAX}}][{{COMPANY}}][{{CLIENT_ADDRESS}}][{{CLIENT_EMAIL}}]\
                       {{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}",
            company: "Bluepeak LLC",
            contact_email: "b@e.test",
            ..Branding::default()
        };
        let html = render_invoice_html(
            &branding,
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(
            html.starts_with("[240.00][10.00][Bluepeak LLC][123 Main St][a@b.test]"),
            "got: {html}"
        );
    }

    #[test]
    fn the_stock_page_shows_the_company_the_address_and_the_email() {
        let (inv, mut client, items) = sample();
        client.billing_address = Some("123 Main St\nSpringfield, IL 62704".into());
        let branding = Branding {
            template: DEFAULT_TEMPLATE,
            company: "Bluepeak LLC",
            contact_email: "b@e.test",
            ..Branding::default()
        };
        let html = render_invoice_html(
            &branding,
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );

        let at = |needle: &str| {
            html.find(needle)
                .unwrap_or_else(|| panic!("missing {needle}: {html}"))
        };
        assert!(at("Bluepeak LLC") < at("Invoice #1248"));
        assert!(at("Invoice For") < at("123 Main St"));
        assert!(at("123 Main St") < at("Springfield, IL 62704"));
        assert!(at("Springfield, IL 62704") < at("a@b.test"));
        assert!(at("a@b.test") < at("Description"));
    }

    /// The seven house blocks, in document order.
    #[test]
    fn the_stock_page_carries_all_seven_house_blocks() {
        let (mut inv, mut client, items) = sample();
        inv.notes = Some("Thanks for your business.".into());
        client.billing_address = Some("123 Main St".into());
        let uri = png_data_uri();
        let branding = Branding {
            template: DEFAULT_TEMPLATE,
            company: "Bluepeak LLC",
            company_address: "P.O. Box 1234",
            company_phone: "619.555.0123",
            logo: &uri,
            payment_instructions: "Wells Fargo, routing 121000248",
            contact_email: "b@e.test",
        };
        let html = render_invoice_html(
            &branding,
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Link("https://pay.stripe.test/x"),
        );

        let at = |needle: &str| {
            html.find(needle)
                .unwrap_or_else(|| panic!("missing {needle}: {html}"))
        };
        assert!(at("class=\"logo\"") < at(">From<"));
        assert!(at(">From<") < at("Invoice ID"));
        assert!(at("Invoice ID") < at("Invoice For"));
        assert!(at("Invoice For") < at("<th>Description</th>"));
        assert!(at("<th>Description</th>") < at("<td colspan=\"3\">Total</td>"));
        assert!(at("<td colspan=\"3\">Total</td>") < at("<hr class=\"foot-rule\">"));
        assert!(at("<hr class=\"foot-rule\">") < at("<h3>Notes</h3>"));
        assert!(at("<h3>Notes</h3>") < at("<h3>Payment</h3>"));
    }

    #[test]
    fn the_stock_page_item_table_says_quantity_and_unit_price() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand("b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(html.contains("<th>Quantity</th>"), "got: {html}");
        assert!(html.contains("<th>Unit Price</th>"), "got: {html}");
        assert!(!html.contains("<th>Qty</th>"), "got: {html}");
        assert!(!html.contains("<th>Unit</th>"), "got: {html}");
        assert!(!html.contains("<th>Rate</th>"), "got: {html}");
    }

    /// The wording that used to be compiled into the page for everyone,
    /// whether or not they have ever taken a bank transfer.
    #[test]
    fn the_stock_page_no_longer_hardcodes_a_bank_transfer_paragraph() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand("ap@acme.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        for absence in [
            "Direct deposit",
            "bank transfer",
            "account details",
            "ap@acme.test",
        ] {
            assert!(!html.contains(absence), "{absence} survived in: {html}");
        }
    }

    #[test]
    fn the_stock_page_prints_the_configured_payment_instructions() {
        let (inv, client, items) = sample();
        let branding = Branding {
            template: DEFAULT_TEMPLATE,
            payment_instructions: "Wells Fargo\nRouting 121000248",
            contact_email: "b@e.test",
            ..Branding::default()
        };
        let html = render_invoice_html(
            &branding,
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(html.contains("<h3>Payment</h3>"), "got: {html}");
        assert!(
            html.contains("Wells Fargo<br>Routing 121000248"),
            "got: {html}"
        );
    }

    #[test]
    fn the_stock_page_of_a_sparse_invoice_prints_no_empty_labels() {
        let (mut inv, mut client, items) = sample();
        inv.due_date = None;
        inv.notes = None;
        inv.terms = None;
        inv.tax = 0.0;
        client.email = None;
        client.billing_address = None;

        let html = render_invoice_html(
            &brand("b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        for absence in [
            "<p></p>",
            "<h3></h3>",
            "Due Date",
            "<br><br>",
            "Notes",
            "Terms",
            "Payment",
            "From",
            "ph.",
            "class=\"logo\"",
            "Subtotal",
            "Tax",
            "Paid",
        ] {
            assert!(!html.contains(absence), "{absence} survived in: {html}");
        }
    }

    /// The letterhead holds two fragments, so neither can own the wrapper the
    /// way `{{COMPANY_BLOCK}}` owns its own `.party` div. An installation with
    /// no logo and no company would otherwise reserve the band's whole margin
    /// above the title for nothing, where the PDF draws nothing at all.
    #[test]
    fn an_empty_letterhead_takes_up_no_room_on_the_stock_page() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand("b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(
            html.contains("<header class=\"letterhead\"></header>"),
            "nothing to draw: {html}"
        );
        assert!(
            html.contains(".letterhead:empty{display:none}"),
            "and so nothing is drawn: {html}"
        );
    }

    #[test]
    fn the_stock_page_and_the_money_lines_agree() {
        let (mut inv, client, items) = sample();
        inv.subtotal = 240.0;
        inv.tax = 10.0;
        let summary = MoneySummary::of(&inv, 50.0);

        let html = render_invoice_html(
            &brand("b@e.test"),
            &inv,
            &client,
            &items,
            &summary,
            PayButton::Omitted,
        );
        let mut at = 0;
        for line in summary.lines() {
            let found = html[at..]
                .find(line.label)
                .unwrap_or_else(|| panic!("{} missing or out of order: {html}", line.label));
            at += found + line.label.len();
        }
        assert_eq!(html.matches("Balance due").count(), 1, "got: {html}");
    }

    #[test]
    fn every_placeholder_in_the_vocabulary_expands() {
        let (inv, client, items) = sample();
        let template = PLACEHOLDERS
            .iter()
            .map(|k| format!("{{{{{k}}}}}"))
            .collect::<Vec<_>>()
            .join("\n");
        let html = render_invoice_html(
            &brand_with(&template, "b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Link("https://pay.test/x"),
        );
        assert!(!html.contains("{{"), "unexpanded placeholder in: {html}");
    }

    #[test]
    fn optional_values_render_empty_rather_than_an_empty_tag() {
        let (mut inv, mut client, items) = sample();
        inv.due_date = None;
        inv.notes = None;
        inv.terms = None;
        client.email = None;
        client.billing_address = None;

        let html = render_invoice_html(
            &brand("b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(!html.contains("Due:"), "got: {html}");
        assert!(!html.contains("<p></p>"), "got: {html}");
        assert!(!html.contains("Notes"), "got: {html}");
        assert!(!html.contains("Terms"), "got: {html}");
    }

    #[test]
    fn notes_and_terms_are_escaped() {
        let (mut inv, client, items) = sample();
        inv.notes = Some("<b>thanks</b>".into());
        inv.terms = Some("<i>net 30</i>".into());

        let html = render_invoice_html(
            &brand("b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(html.contains("&lt;b&gt;thanks&lt;/b&gt;"), "got: {html}");
        assert!(html.contains("&lt;i&gt;net 30&lt;/i&gt;"), "got: {html}");
        assert!(!html.contains("<b>thanks"), "got: {html}");
    }

    #[test]
    fn due_date_is_the_bare_date_and_due_is_the_fragment() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand_with(
                "[{{DUE_DATE}}][{{DUE}}]{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}",
                "b@e.test",
            ),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(
            html.starts_with("[2026-09-03][<br>Due: 2026-09-03]"),
            "got: {html}"
        );
    }

    #[test]
    fn the_pay_url_is_the_link_only_when_there_is_one() {
        let (inv, client, items) = sample();
        let template = "[{{PAY_URL}}]{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}";
        let linked = render_invoice_html(
            &brand_with(template, "b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Link("https://pay.test/x"),
        );
        assert!(linked.starts_with("[https://pay.test/x]"), "got: {linked}");

        for pay in [PayButton::Placeholder, PayButton::Omitted] {
            let html = render_invoice_html(
                &brand_with(template, "b@e.test"),
                &inv,
                &client,
                &items,
                &money(&inv),
                pay,
            );
            assert!(html.starts_with("[]"), "got: {html}");
        }
    }

    #[test]
    fn the_default_template_renders_notes_and_terms() {
        let (mut inv, client, items) = sample();
        inv.notes = Some("Thanks for the work".into());
        inv.terms = Some("Net 30".into());

        let html = render_invoice_html(
            &brand("b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(html.contains("Thanks for the work"), "got: {html}");
        assert!(html.contains("Net 30"), "got: {html}");
    }

    #[test]
    fn operator_markup_in_a_template_is_not_sanitized() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand_with(
                "<script>alert(1)</script>{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}",
                "b@e.test",
            ),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(html.contains("<script>alert(1)</script>"));
    }

    /// The stock page as it shipped before the money block existed, pasted
    /// verbatim. An operator who ran `nigel invoice template` against that
    /// release has this file in their data directory.
    const LEGACY_TEMPLATE: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><title>Invoice {{NUMBER}}</title>
<style>body{font-family:system-ui,sans-serif;max-width:40rem;margin:2rem auto;padding:0 1rem}
table{width:100%;border-collapse:collapse}td,th{text-align:left;padding:.4rem;border-bottom:1px solid #ddd}
.total{font-weight:700}.pay{display:inline-block;margin:1rem 0;padding:.6rem 1rem;background:#111;color:#fff;text-decoration:none;border-radius:.4rem}</style>
</head><body>
<h1>Invoice #{{NUMBER}}</h1>
<p>Billed to: {{CLIENT}}<br>Issued: {{ISSUE}}{{DUE}}</p>
<table><thead><tr><th>Description</th><th>Qty</th><th>Unit</th><th>Amount</th></tr></thead>
<tbody>{{ROWS}}</tbody></table>
<p class="total">Total: {{CURRENCY}} {{TOTAL}}</p>
{{PAY}}
{{NOTES}}
{{TERMS}}
<h3>Direct deposit</h3>
<p>To pay by bank transfer, reference invoice <strong>#{{NUMBER}}</strong>. Contact {{CONTACT}} for account details.</p>
</body></html>
"#;

    #[test]
    fn a_template_exported_before_the_field_parity_change_still_loads_and_renders() {
        let (inv, client, items) = sample();
        let dir = tempfile::tempdir().unwrap();
        write_override(dir.path(), LEGACY_TEMPLATE);

        let loaded = load_template(dir.path()).expect("an older export must keep working");
        let html = render_invoice_html(
            &brand_with(&loaded, "b@e.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );
        assert!(html.contains("Billed to: Acme"), "got: {html}");
        assert!(html.contains("Total: USD 250.00"), "got: {html}");
        assert!(!html.contains("{{"), "no unexpanded placeholder: {html}");
    }

    /// The house layout added seven placeholders and rewrote the stock page. A
    /// template exported before any of that carries none of the new keys, and
    /// every key it does carry has to render what it always rendered — so the
    /// upgrade is invisible to an operator who owns their own template.
    #[test]
    fn the_pre_204_template_gains_nothing_and_loses_nothing_from_the_house_layout() {
        let (mut inv, client, items) = sample();
        inv.due_date = Some("2026-09-05".into());
        inv.terms = Some("Net 30".into());

        let html = render_invoice_html(
            &brand_with(LEGACY_TEMPLATE, "ap@acme.test"),
            &inv,
            &client,
            &items,
            &money(&inv),
            PayButton::Omitted,
        );

        assert!(html.contains("Billed to: Acme"), "got: {html}");
        assert!(
            html.contains("Due: 2026-09-05"),
            "the old {{{{DUE}}}} shape, unchanged: {html}"
        );
        assert!(
            !html.contains("(Net 30)"),
            "{{{{DUE}}}} did not gain the parenthetical: {html}"
        );
        assert!(
            html.contains("<h3>Terms</h3>"),
            "the old {{{{TERMS}}}} block, unchanged: {html}"
        );
        assert!(html.contains("ap@acme.test"), "{{{{CONTACT}}}}: {html}");
        assert!(!html.contains("{{"), "no unexpanded placeholder: {html}");
    }

    /// The list a template must satisfy never grows: every placeholder the
    /// house layout added is optional, which is what makes the test above hold
    /// for every template exported from every earlier release.
    #[test]
    fn required_is_still_exactly_four_keys() {
        assert_eq!(REQUIRED, &["NUMBER", "CLIENT", "ROWS", "TOTAL"]);
        assert_eq!(REQUIRED_ALTERNATIVES, &[("TOTAL", "TOTALS")]);
    }

    #[test]
    fn the_voided_page_names_the_invoice_and_offers_nothing_to_pay() {
        let html = voided_page_html(1248);
        assert!(html.contains("voided"), "got: {html}");
        assert!(html.contains("#1248"), "got: {html}");
        assert!(!html.contains("class=\"pay\""), "got: {html}");
        assert!(!html.contains("href"), "no link to follow: {html}");
    }

    /// It renders from nothing but the number, so no configuration and no
    /// operator template can stop a void from replacing the live page.
    #[test]
    fn the_voided_page_expands_no_placeholders() {
        let html = voided_page_html(1248);
        assert!(!html.contains("{{"), "got: {html}");
    }
}
