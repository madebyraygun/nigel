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
use crate::invoicing::r2::R2PrivateStore;
use crate::invoicing::r2::{public_base_url_warning, validate_public_base_url, R2Publisher};
use crate::invoicing::render_html::{load_template, Branding};
use crate::invoicing::republish::republish_invoice;
use crate::invoicing::stripe::StripeClient;
use crate::models::Invoice;
use crate::settings::{derive_documents_base, DocumentsConfig, InvoicingConfig};

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
/// The fields are private and `company_profile` is the only constructor, so that
/// stays true of every profile that exists.
///
/// The address, the phone and the logo reach a document only through `branding`,
/// which is why they have no accessor: a caller wanting the letterhead wants all
/// of it, assembled, not three strings to arrange itself.
pub struct CompanyProfile {
    name: String,
    address: String,
    phone: String,
    logo: String,
    payment_instructions: String,
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
    /// The business name, as the subject line and the report headers want it.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The direct-deposit or cheque instructions an invoice prints, empty when
    /// the operator has set none.
    pub fn payment_instructions(&self) -> &str {
        &self.payment_instructions
    }

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

/// The Stripe client for the configured secret key, erroring by name when the
/// key is absent so a caller reports what is missing rather than what failed.
pub fn build_gateway(cfg: &InvoicingConfig) -> Result<StripeClient> {
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
///
/// The fields are private and `build_clients` is the only constructor, which is
/// what makes its checks unskippable: the public base URL must produce a working
/// link, and the from address must be a bare address safe to put in a header. A
/// hand-assembled set skips all of that and mails from whatever it was given.
pub struct SendClients {
    stripe: StripeClient,
    r2: R2Publisher,
    mail: MailgunClient,
    warnings: Vec<String>,
}

impl SendClients {
    /// The payment gateway.
    pub fn stripe(&self) -> &StripeClient {
        &self.stripe
    }

    /// The asset publisher the invoice page is uploaded through.
    pub fn r2(&self) -> &R2Publisher {
        &self.r2
    }

    /// The mail client, carrying the sender identity `build_clients` validated.
    pub fn mail(&self) -> &MailgunClient {
        &self.mail
    }

    /// Configuration Nigel will send with but wants the operator to look at.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
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

    let (mail, mail_warnings) = build_mailer(
        cfg.mailgun_api_key,
        cfg.mailgun_domain,
        cfg.from_email,
        cfg.from_name,
        cfg.reply_to_email,
        company,
    )?;
    // Config cautions travel as data on the one channel, so a terminal, the
    // TUI and a browser all say the same things about the same installation.
    // The `/i` one is a caution and not a refusal, because an edge rewrite can
    // map that prefix onto the domain root.
    let warnings = mail_warnings
        .into_iter()
        .chain(public_base_url_warning(&r2.public_base_url).map(str::to_string))
        .collect();
    Ok(SendClients {
        stripe,
        r2,
        mail,
        warnings,
    })
}

/// The mail client and its cautions, shared by both builders so the
/// header-injection refusals are identical.
///
/// `company` is the business name an unset `from_name` falls back to.
fn build_mailer(
    api_key: Option<String>,
    domain: Option<String>,
    from_email: Option<String>,
    from_name: Option<String>,
    reply_to: Option<String>,
    company: &str,
) -> Result<(MailgunClient, Vec<String>)> {
    let api_key = require(api_key, "mailgun_api_key")?;
    let domain = require(domain, "mailgun_domain")?;

    // The from address is composed into a header like every other value here,
    // so it is guarded like one — and it must be a bare address, because
    // `format_address` is what puts the display name on.
    let from_address = require(from_email, "from_email")?;
    validate_header_value(&from_address, "from_email")?;
    validate_bare_address(&from_address, "from_email")?;

    // An unset `from_name` falls back to the business name, and the refusal
    // has to name whichever of the two the bad value actually came from: an
    // operator who never set `from_name` cannot fix `from_name`.
    let (from_name, name_source) = match from_name {
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
    if let Some(reply_to) = &reply_to {
        validate_header_value(reply_to, "reply_to_email")?;
    }

    let warnings = from_address_domain_warning(&from_address, &domain)
        .into_iter()
        .collect();
    let mail = MailgunClient {
        api_key,
        domain,
        envelope: EmailEnvelope {
            from_address,
            from_name,
            reply_to,
        },
    };
    Ok((mail, warnings))
}

/// The clients a document send needs. No Stripe: a document has no payment.
///
/// The publisher is its own `R2Publisher` built with the documents base, never
/// the invoice publisher, so an object lands under `d/` and links under `/d`.
pub struct DocumentClients {
    publisher: R2Publisher,
    source: R2PrivateStore,
    mail: MailgunClient,
    response_url: Option<String>,
    warnings: Vec<String>,
}

impl DocumentClients {
    pub fn publisher(&self) -> &R2Publisher {
        &self.publisher
    }

    pub fn source(&self) -> &R2PrivateStore {
        &self.source
    }

    pub fn mail(&self) -> &MailgunClient {
        &self.mail
    }

    pub fn response_url(&self) -> Option<&str> {
        self.response_url.as_deref()
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

/// The clients a document send needs, or the first refusal the configuration
/// earns. The requirement order is `documents_status`'s.
pub fn build_document_clients(cfg: DocumentsConfig, company: &str) -> Result<DocumentClients> {
    let inv = cfg.invoicing;
    // Presence is checked in the status order before anything is validated, so
    // the first refusal names the first missing key.
    let api_key = require(inv.mailgun_api_key, "mailgun_api_key")?;
    let domain = require(inv.mailgun_domain, "mailgun_domain")?;
    let from_email = require(inv.from_email, "from_email")?;
    let account_id = require(inv.r2_account_id, "r2_account_id")?;
    let access_key = require(inv.r2_access_key, "r2_access_key")?;
    let secret_key = require(inv.r2_secret_key, "r2_secret_key")?;
    let bucket = require(inv.r2_bucket, "r2_bucket")?;
    let private_bucket = require(cfg.r2_private_bucket, "r2_private_bucket")?;
    let documents_base = require(
        derive_documents_base(
            cfg.documents_base_url.as_deref(),
            inv.public_base_url.as_deref(),
        ),
        "documents_base_url",
    )?;
    validate_public_base_url(&documents_base)?;

    let (mail, warnings) = build_mailer(
        Some(api_key),
        Some(domain),
        Some(from_email),
        inv.from_name,
        inv.reply_to_email,
        company,
    )?;
    Ok(DocumentClients {
        source: R2PrivateStore {
            account_id: account_id.clone(),
            access_key: access_key.clone(),
            secret_key: secret_key.clone(),
            bucket: private_bucket,
        },
        publisher: R2Publisher {
            account_id,
            access_key,
            secret_key,
            bucket,
            public_base_url: documents_base,
        },
        mail,
        response_url: cfg.document_response_url,
        warnings,
    })
}

/// The document publisher, when its keys and a documents base are all set.
pub fn optional_document_publisher(cfg: &DocumentsConfig) -> Option<R2Publisher> {
    let inv = &cfg.invoicing;
    Some(R2Publisher {
        account_id: inv.r2_account_id.clone()?,
        access_key: inv.r2_access_key.clone()?,
        secret_key: inv.r2_secret_key.clone()?,
        bucket: inv.r2_bucket.clone()?,
        public_base_url: derive_documents_base(
            cfg.documents_base_url.as_deref(),
            inv.public_base_url.as_deref(),
        )?,
    })
}

/// The private response store, when its four keys are set.
pub fn optional_response_source(cfg: &DocumentsConfig) -> Option<R2PrivateStore> {
    let inv = &cfg.invoicing;
    Some(R2PrivateStore {
        account_id: inv.r2_account_id.clone()?,
        access_key: inv.r2_access_key.clone()?,
        secret_key: inv.r2_secret_key.clone()?,
        bucket: cfg.r2_private_bucket.clone()?,
    })
}

/// The sentence a republish that could not even be attempted earns. One home,
/// so a terminal and a browser cannot word the same failure differently.
pub fn republish_warning(what: &str, e: NigelError) -> String {
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
pub fn contact_address(cfg: &InvoicingConfig) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn configured_for_documents() -> InvoicingConfig {
        InvoicingConfig {
            stripe_secret_key: None,
            mailgun_api_key: Some("key-test".into()),
            mailgun_domain: Some("mg.example.com".into()),
            from_email: Some("billing@example.com".into()),
            from_name: Some("Cedar Systems".into()),
            reply_to_email: None,
            contact_email: None,
            r2_account_id: Some("acct".into()),
            r2_access_key: Some("access".into()),
            r2_secret_key: Some("secret".into()),
            r2_bucket: Some("billing".into()),
            public_base_url: Some("https://billing.example.com/i".into()),
        }
    }

    #[test]
    fn the_document_builder_refuses_a_display_name_with_a_line_break() {
        let cfg = DocumentsConfig {
            invoicing: InvoicingConfig {
                from_name: Some("Cedar\r\nBcc: x@y.test".into()),
                ..configured_for_documents()
            },
            r2_private_bucket: Some("private".into()),
            ..Default::default()
        };
        assert!(build_document_clients(cfg, "").is_err());
    }

    #[test]
    fn the_document_publisher_is_built_on_the_documents_base_not_the_invoice_one() {
        let cfg = DocumentsConfig {
            invoicing: configured_for_documents(),
            r2_private_bucket: Some("private".into()),
            ..Default::default()
        };
        let clients = build_document_clients(cfg, "Cedar Systems").unwrap();
        assert_eq!(
            clients.publisher().public_base_url,
            "https://billing.example.com/d"
        );
        assert_eq!(clients.publisher().bucket, "billing");
        assert_eq!(clients.source().bucket, "private");
    }

    #[test]
    fn the_document_builder_names_the_first_missing_key_and_never_asks_for_stripe() {
        let cfg = DocumentsConfig {
            invoicing: configured_for_documents(),
            ..Default::default()
        };
        let err = build_document_clients(cfg, "").err().unwrap().to_string();
        assert!(err.contains("r2_private_bucket"), "got: {err}");
        assert!(!err.contains("stripe"), "got: {err}");
    }

    #[test]
    fn an_invoice_base_without_i_needs_an_explicit_documents_base() {
        let cfg = DocumentsConfig {
            invoicing: InvoicingConfig {
                public_base_url: Some("https://billing.example.com".into()),
                ..configured_for_documents()
            },
            r2_private_bucket: Some("private".into()),
            ..Default::default()
        };
        let err = build_document_clients(cfg, "").err().unwrap().to_string();
        assert!(err.contains("documents_base_url"), "got: {err}");
    }

    #[test]
    fn the_optional_builders_need_only_their_own_keys() {
        let mut cfg = DocumentsConfig {
            invoicing: configured_for_documents(),
            ..Default::default()
        };
        assert!(optional_response_source(&cfg).is_none());
        assert!(optional_document_publisher(&cfg).is_some());
        cfg.r2_private_bucket = Some("private".into());
        assert_eq!(optional_response_source(&cfg).unwrap().bucket, "private");
    }
}
