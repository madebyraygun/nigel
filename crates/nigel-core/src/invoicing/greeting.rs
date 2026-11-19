//! The operator's own opening paragraph on an invoice or document email, and
//! the email preview both send dialogs show before anything goes out.
//!
//! A greeting is plain text that rides in the email body only: it is never
//! drawn on the published page or in the PDF. Each kind has a saved default in
//! the `metadata` table, and each send may replace it or clear it.

use rusqlite::Connection;
use serde::Serialize;

use crate::error::{NigelError, Result};
use crate::fmt::printable;

/// The longest greeting accepted, counted in characters after trimming.
pub const EMAIL_GREETING_MAX_CHARS: usize = 2000;

/// The `metadata` key holding the default greeting for invoice emails.
pub const INVOICE_GREETING_KEY: &str = "invoice_email_greeting";

/// The `metadata` key holding the default greeting for document emails.
pub const DOCUMENT_GREETING_KEY: &str = "document_email_greeting";

/// What an email preview prints where a link that only exists after the send
/// would go: the invoice's published page, or a recipient's personal page.
pub const PREVIEW_LINK_PLACEHOLDER: &str = "[link created when sent]";

/// Why a greeting was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GreetingProblem {
    /// Longer than [`EMAIL_GREETING_MAX_CHARS`] after trimming.
    TooLong,
    /// Names a placeholder its kind does not offer; this is the first such
    /// name, as written between `${` and `}`.
    UnknownPlaceholder(String),
}

/// A refused greeting: the sentence to show, what was wrong, and the settings
/// field it came from when the refusal is a settings write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GreetingRefusal {
    pub message: String,
    pub problem: GreetingProblem,
    pub field: Option<&'static str>,
}

impl std::fmt::Display for GreetingRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

fn refused(message: String, problem: GreetingProblem) -> NigelError {
    NigelError::GreetingInvalid(Box::new(GreetingRefusal {
        message,
        problem,
        field: None,
    }))
}

/// `err` with `prefix` in front of its sentence and, for a greeting refusal,
/// `field` recorded on it. Any other error passes through unchanged.
pub fn refusal_in_context(
    err: NigelError,
    prefix: &str,
    field: Option<&'static str>,
) -> NigelError {
    match err {
        NigelError::GreetingInvalid(refusal) => {
            NigelError::GreetingInvalid(Box::new(GreetingRefusal {
                message: format!("{prefix}{}", refusal.message),
                field: field.or(refusal.field),
                ..*refusal
            }))
        }
        other => other,
    }
}

/// A greeting trimmed, with an empty one as none, or the refusal for one over
/// [`EMAIL_GREETING_MAX_CHARS`].
pub fn clean_greeting(text: &str) -> Result<Option<String>> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let chars = trimmed.chars().count();
    if chars > EMAIL_GREETING_MAX_CHARS {
        return Err(refused(
            format!(
                "The email greeting can be at most {EMAIL_GREETING_MAX_CHARS} characters; this one is {chars}."
            ),
            GreetingProblem::TooLong,
        ));
    }
    Ok(Some(trimmed.to_string()))
}

/// Which email a greeting opens, and so which placeholders it may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GreetingKind {
    Invoice,
    Document,
}

/// The placeholders any greeting may name.
const SHARED_PLACEHOLDERS: &[&str] = &["firstName", "fullName", "clientName", "companyName"];

/// The placeholders only an invoice greeting may name.
const INVOICE_PLACEHOLDERS: &[&str] = &["invoiceNumber", "amountDue", "dueDate"];

/// The placeholders only a document greeting may name.
const DOCUMENT_PLACEHOLDERS: &[&str] = &["documentTitle"];

impl GreetingKind {
    /// The `metadata` key holding this kind's saved default.
    pub fn key(self) -> &'static str {
        match self {
            GreetingKind::Invoice => INVOICE_GREETING_KEY,
            GreetingKind::Document => DOCUMENT_GREETING_KEY,
        }
    }

    /// The settings field this kind's saved default is written through.
    pub fn settings_field(self) -> &'static str {
        match self {
            GreetingKind::Invoice => "invoiceEmailGreeting",
            GreetingKind::Document => "documentEmailGreeting",
        }
    }

    /// How a sentence names this kind's greeting: `invoice` or `document`.
    pub fn noun(self) -> &'static str {
        match self {
            GreetingKind::Invoice => "invoice",
            GreetingKind::Document => "document",
        }
    }

    /// The saved default, empty when none is set. A failed read is an error,
    /// never an empty greeting.
    pub fn saved_default(self, conn: &Connection) -> Result<String> {
        Ok(crate::db::read_metadata(conn, self.key())?.unwrap_or_default())
    }

    /// Every placeholder a greeting of this kind may name, written `${name}`.
    pub fn placeholders(self) -> Vec<&'static str> {
        let own = match self {
            GreetingKind::Invoice => INVOICE_PLACEHOLDERS,
            GreetingKind::Document => DOCUMENT_PLACEHOLDERS,
        };
        SHARED_PLACEHOLDERS.iter().chain(own).copied().collect()
    }
}

/// Each `${name}` in `text`, in order, as `(start, end, name)` byte offsets
/// covering the whole token. A `${` with no closing `}` is plain text.
fn placeholder_tokens(text: &str) -> Vec<(usize, usize, &str)> {
    let mut tokens = Vec::new();
    let mut from = 0;
    while let Some(offset) = text[from..].find("${") {
        let start = from + offset;
        let Some(close) = text[start + 2..].find('}') else {
            break;
        };
        let end = start + 2 + close + 1;
        tokens.push((start, end, &text[start + 2..end - 1]));
        from = end;
    }
    tokens
}

/// Refuses a greeting that names a placeholder its kind does not offer, so a
/// typo is caught before it reaches a client as literal `${…}` text.
pub fn check_placeholders(text: &str, kind: GreetingKind) -> Result<()> {
    let known = kind.placeholders();
    let unknown: Vec<&str> = placeholder_tokens(text)
        .into_iter()
        .map(|(_, _, name)| name)
        .filter(|name| !known.contains(&name.trim()))
        .collect();
    if unknown.is_empty() {
        return Ok(());
    }
    let listed = |names: &[&str]| {
        names
            .iter()
            .map(|n| format!("${{{n}}}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    Err(refused(
        format!(
            "Unknown placeholder {}. Available: {}.",
            listed(&unknown),
            listed(&known)
        ),
        GreetingProblem::UnknownPlaceholder(unknown[0].trim().to_string()),
    ))
}

/// The values one email fills its greeting's placeholders with.
#[derive(Debug, Clone, Default)]
pub struct GreetingValues {
    pub full_name: String,
    pub client_name: String,
    pub company_name: String,
    pub invoice_number: String,
    pub amount_due: String,
    pub due_date: String,
    pub document_title: String,
}

impl GreetingValues {
    /// The first word of the recipient's name, or the client's name when the
    /// recipient has none.
    fn first_name(&self) -> String {
        self.full_name
            .split_whitespace()
            .next()
            .map(str::to_string)
            .unwrap_or_else(|| self.client_name.clone())
    }

    fn value(&self, name: &str) -> Option<String> {
        Some(match name {
            "firstName" => self.first_name(),
            "fullName" if self.full_name.trim().is_empty() => self.client_name.clone(),
            "fullName" => self.full_name.trim().to_string(),
            "clientName" => self.client_name.clone(),
            "companyName" => self.company_name.clone(),
            "invoiceNumber" => self.invoice_number.clone(),
            "amountDue" => self.amount_due.clone(),
            "dueDate" => self.due_date.clone(),
            "documentTitle" => self.document_title.clone(),
            _ => return None,
        })
    }
}

/// `greeting` with each placeholder replaced by its value. A known name with
/// no value — `${dueDate}` on an invoice with no due date — becomes empty; only
/// a name this module does not recognise stays as written, and
/// [`check_placeholders`] has refused those already.
pub fn fill_placeholders(greeting: &str, values: &GreetingValues) -> String {
    let mut out = String::with_capacity(greeting.len());
    let mut last = 0;
    for (start, end, name) in placeholder_tokens(greeting) {
        out.push_str(&greeting[last..start]);
        match values.value(name.trim()) {
            Some(value) => out.push_str(&value),
            None => out.push_str(&greeting[start..end]),
        }
        last = end;
    }
    out.push_str(&greeting[last..]);
    out
}

/// The greeting one send uses: `None` asked for is the saved default, an empty
/// string is no greeting, and anything else is exactly that text, trimmed.
/// Placeholders are checked against `kind` and left unfilled: each email fills
/// them with its own recipient's values.
pub fn resolve_greeting(
    requested: Option<&str>,
    default: &str,
    kind: GreetingKind,
) -> Result<Option<String>> {
    let greeting = clean_greeting(requested.unwrap_or(default))?;
    if let Some(text) = &greeting {
        check_placeholders(text, kind)?;
    }
    Ok(greeting)
}

/// The saved default greeting for invoice emails, empty when none is set.
pub fn invoice_default(conn: &Connection) -> Result<String> {
    GreetingKind::Invoice.saved_default(conn)
}

/// The saved default greeting for document emails, empty when none is set.
pub fn document_default(conn: &Connection) -> Result<String> {
    GreetingKind::Document.saved_default(conn)
}

/// Every email one send would deliver, built by the functions the send itself
/// uses, with each link that does not exist yet replaced by a placeholder.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EmailPreview {
    pub emails: Vec<PreviewEmail>,
    /// Every link placeholder the bodies carry, so a screen can mark them.
    pub link_placeholders: Vec<String>,
}

/// Who one previewed message goes to.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PreviewRole {
    /// The client an invoice is billed to.
    Client,
    /// A document's signer.
    Signer,
    /// A document's collaborator.
    Collaborator,
}

impl PreviewRole {
    pub fn as_str(self) -> &'static str {
        match self {
            PreviewRole::Client => "client",
            PreviewRole::Signer => "signer",
            PreviewRole::Collaborator => "collaborator",
        }
    }
}

/// One message as it would go out.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewEmail {
    pub role: PreviewRole,
    /// The `To` header, as `Name <email>` or a bare address.
    pub to: String,
    /// Everyone copied. An invoice copies the client's other contacts; a
    /// document email has none.
    pub cc: Vec<String>,
    pub subject: String,
    /// The plain-text body.
    pub body: String,
    /// The attachment file names.
    pub attachments: Vec<String>,
}

/// The preview printed for a terminal: each message's headers, attachments and
/// body, with a note naming the placeholder when one appears. Every value goes
/// through [`printable`], since a contact's name or a greeting may carry
/// control characters the terminal would act on.
pub fn format_email_preview(preview: &EmailPreview) -> String {
    let rule = "-".repeat(60);
    let mut out = String::new();
    for email in &preview.emails {
        out.push_str(&rule);
        out.push('\n');
        out.push_str(&format!("To: {}\n", printable(&email.to)));
        if !email.cc.is_empty() {
            out.push_str(&format!("Cc: {}\n", printable(&email.cc.join(", "))));
        }
        out.push_str(&format!("Subject: {}\n", printable(&email.subject)));
        if !email.attachments.is_empty() {
            out.push_str(&format!(
                "Attachments: {}\n",
                printable(&email.attachments.join(", "))
            ));
        }
        out.push('\n');
        out.push_str(&printable(email.body.trim_end()));
        out.push('\n');
    }
    out.push_str(&rule);
    out.push('\n');
    for placeholder in &preview.link_placeholders {
        out.push_str(&format!(
            "{placeholder} stands in for a link the send creates.\n"
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_fill_and_a_missing_name_falls_back_to_the_client() {
        let values = GreetingValues {
            full_name: "Pat Example".into(),
            client_name: "Cedar Systems".into(),
            company_name: "Initech".into(),
            invoice_number: "1001".into(),
            amount_due: "USD 1,250.00".into(),
            due_date: "2026-12-01".into(),
            ..GreetingValues::default()
        };
        assert_eq!(
            fill_placeholders(
                "Hi ${firstName} (${fullName}): ${companyName} invoice ${ invoiceNumber } for ${clientName}, ${amountDue} due ${dueDate}.",
                &values
            ),
            "Hi Pat (Pat Example): Initech invoice 1001 for Cedar Systems, USD 1,250.00 due 2026-12-01."
        );
        let nameless = GreetingValues {
            client_name: "Cedar Systems".into(),
            ..GreetingValues::default()
        };
        assert_eq!(
            fill_placeholders("Hi ${firstName} / ${fullName}", &nameless),
            "Hi Cedar Systems / Cedar Systems"
        );
        assert_eq!(
            fill_placeholders("Costs ${ and $5 {x} ${unclosed", &values),
            "Costs ${ and $5 {x} ${unclosed"
        );
    }

    #[test]
    fn unknown_placeholders_are_refused_per_kind() {
        assert!(
            check_placeholders("Hi ${firstName}, ${invoiceNumber}", GreetingKind::Invoice).is_ok()
        );
        assert!(
            check_placeholders("Hi ${firstName}, ${documentTitle}", GreetingKind::Document).is_ok()
        );
        let err = check_placeholders("${invoiceNumber}", GreetingKind::Document).unwrap_err();
        assert!(
            matches!(
                &err,
                NigelError::GreetingInvalid(refusal)
                    if refusal.problem == GreetingProblem::UnknownPlaceholder("invoiceNumber".into())
            ),
            "{err:?}"
        );
        assert!(err.to_string().contains("${invoiceNumber}"), "{err}");
        assert!(err.to_string().contains("${documentTitle}"), "{err}");
        assert!(resolve_greeting(Some("Hi ${frstName}"), "", GreetingKind::Invoice).is_err());
        assert!(
            resolve_greeting(None, "Hi ${frstName}", GreetingKind::Invoice).is_err(),
            "a saved default is checked too"
        );
    }

    #[test]
    fn absent_is_the_default_empty_is_none_and_text_is_trimmed() {
        assert_eq!(
            resolve_greeting(None, "  Thanks, Acme.  ", GreetingKind::Invoice).unwrap(),
            Some("Thanks, Acme.".into())
        );
        assert_eq!(
            resolve_greeting(Some(""), "Thanks, Acme.", GreetingKind::Invoice).unwrap(),
            None
        );
        assert_eq!(
            resolve_greeting(Some("   "), "Thanks, Acme.", GreetingKind::Invoice).unwrap(),
            None
        );
        assert_eq!(
            resolve_greeting(
                Some("  Hi Globex\nSee attached. "),
                "Thanks",
                GreetingKind::Invoice
            )
            .unwrap(),
            Some("Hi Globex\nSee attached.".into())
        );
        assert_eq!(
            resolve_greeting(None, "", GreetingKind::Invoice).unwrap(),
            None
        );
    }

    #[test]
    fn a_saved_default_is_empty_when_unset_and_an_error_when_unreadable() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(
            invoice_default(&conn).is_err(),
            "a failed read is not an empty greeting"
        );
        conn.execute_batch("CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT)")
            .unwrap();
        assert_eq!(invoice_default(&conn).unwrap(), "");
        crate::db::set_metadata(&conn, DOCUMENT_GREETING_KEY, "Hi ${firstName}").unwrap();
        assert_eq!(document_default(&conn).unwrap(), "Hi ${firstName}");
    }

    #[test]
    fn the_terminal_preview_neutralises_control_characters() {
        let preview = EmailPreview {
            emails: vec![PreviewEmail {
                role: PreviewRole::Client,
                to: "Acme\x1b[2J <ap@acme.example>".into(),
                cc: vec!["Ops\u{202E} <ops@acme.example>".into()],
                subject: "Invoice\r #1001".into(),
                body: "Hi\x07 Acme\nLine two\n".into(),
                attachments: vec!["invoice\x1b.pdf".into()],
            }],
            link_placeholders: Vec::new(),
        };
        let text = format_email_preview(&preview);
        assert!(
            !text.chars().any(|c| c != '\n' && c.is_control()),
            "{text:?}"
        );
        assert!(!text.contains('\u{202E}'), "{text:?}");
        assert!(text.contains("Hi  Acme\nLine two"), "{text:?}");
    }

    #[test]
    fn the_limit_counts_characters_after_trimming() {
        let at_limit = "é".repeat(EMAIL_GREETING_MAX_CHARS);
        assert!(clean_greeting(&format!("  {at_limit}  "))
            .unwrap()
            .is_some());
        let over = "a".repeat(EMAIL_GREETING_MAX_CHARS + 1);
        let err = clean_greeting(&over).unwrap_err();
        assert!(
            matches!(
                &err,
                NigelError::GreetingInvalid(refusal) if refusal.problem == GreetingProblem::TooLong
            ),
            "{err:?}"
        );
        assert!(
            resolve_greeting(None, &over, GreetingKind::Invoice).is_err(),
            "a default is checked too"
        );
    }

    #[test]
    fn the_terminal_preview_prints_headers_attachments_body_and_the_placeholder_note() {
        let preview = EmailPreview {
            emails: vec![PreviewEmail {
                role: PreviewRole::Client,
                to: "Juniper Labs <ap@juniper.example>".into(),
                cc: vec!["ops@juniper.example".into()],
                subject: "Invoice #1001 from Initech".into(),
                body: format!("Thanks!\n\nPay now: {PREVIEW_LINK_PLACEHOLDER}\n"),
                attachments: vec!["invoice.pdf".into()],
            }],
            link_placeholders: vec![PREVIEW_LINK_PLACEHOLDER.into()],
        };
        let text = format_email_preview(&preview);
        for line in [
            "To: Juniper Labs <ap@juniper.example>",
            "Cc: ops@juniper.example",
            "Subject: Invoice #1001 from Initech",
            "Attachments: invoice.pdf",
            "Thanks!\n\nPay now: [link created when sent]",
            "[link created when sent] stands in for a link the send creates.",
        ] {
            assert!(text.contains(line), "missing {line:?} in:\n{text}");
        }
    }
}
