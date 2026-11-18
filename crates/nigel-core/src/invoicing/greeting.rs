//! The operator's own opening paragraph on an invoice or document email, and
//! the email preview both send dialogs show before anything goes out.
//!
//! A greeting is plain text that rides in the email body only: it is never
//! drawn on the published page or in the PDF. Each kind has a saved default in
//! the `metadata` table, and each send may replace it or clear it.

use rusqlite::Connection;
use serde::Serialize;

use crate::error::{NigelError, Result};

/// The longest greeting accepted, counted in characters after trimming.
pub const EMAIL_GREETING_MAX_CHARS: usize = 2000;

/// The `metadata` key holding the default greeting for invoice emails.
pub const INVOICE_GREETING_KEY: &str = "invoice_email_greeting";

/// The `metadata` key holding the default greeting for document emails.
pub const DOCUMENT_GREETING_KEY: &str = "document_email_greeting";

/// What an email preview prints where a link that only exists after the send
/// would go: the invoice's published page, or a recipient's personal page.
pub const PREVIEW_LINK_PLACEHOLDER: &str = "[link created when sent]";

/// A greeting trimmed, with an empty one as none, or the refusal for one over
/// [`EMAIL_GREETING_MAX_CHARS`].
pub fn clean_greeting(text: &str) -> Result<Option<String>> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let chars = trimmed.chars().count();
    if chars > EMAIL_GREETING_MAX_CHARS {
        return Err(NigelError::GreetingInvalid(format!(
            "The email greeting can be at most {EMAIL_GREETING_MAX_CHARS} characters; this one is {chars}."
        )));
    }
    Ok(Some(trimmed.to_string()))
}

/// The greeting one send uses: `None` asked for is the saved default, an empty
/// string is no greeting, and anything else is exactly that text, trimmed.
pub fn resolve_greeting(requested: Option<&str>, default: &str) -> Result<Option<String>> {
    clean_greeting(requested.unwrap_or(default))
}

/// The saved default greeting for invoice emails, empty when none is set.
pub fn invoice_default(conn: &Connection) -> String {
    crate::db::get_metadata(conn, INVOICE_GREETING_KEY).unwrap_or_default()
}

/// The saved default greeting for document emails, empty when none is set.
pub fn document_default(conn: &Connection) -> String {
    crate::db::get_metadata(conn, DOCUMENT_GREETING_KEY).unwrap_or_default()
}

/// Every email one send would deliver, built by the functions the send itself
/// uses, with each link that does not exist yet replaced by a placeholder.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EmailPreview {
    pub emails: Vec<PreviewEmail>,
    /// Every placeholder string the bodies carry, so a screen can mark them.
    pub placeholders: Vec<String>,
}

/// One message as it would go out.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewEmail {
    /// `client` for an invoice; `signer` or `collaborator` for a document.
    pub role: String,
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
/// body, with a note naming the placeholder when one appears.
pub fn format_email_preview(preview: &EmailPreview) -> String {
    let rule = "-".repeat(60);
    let mut out = String::new();
    for email in &preview.emails {
        out.push_str(&rule);
        out.push('\n');
        out.push_str(&format!("To: {}\n", email.to));
        if !email.cc.is_empty() {
            out.push_str(&format!("Cc: {}\n", email.cc.join(", ")));
        }
        out.push_str(&format!("Subject: {}\n", email.subject));
        if !email.attachments.is_empty() {
            out.push_str(&format!("Attachments: {}\n", email.attachments.join(", ")));
        }
        out.push('\n');
        out.push_str(email.body.trim_end());
        out.push('\n');
    }
    out.push_str(&rule);
    out.push('\n');
    for placeholder in &preview.placeholders {
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
    fn absent_is_the_default_empty_is_none_and_text_is_trimmed() {
        assert_eq!(
            resolve_greeting(None, "  Thanks, Acme.  ").unwrap(),
            Some("Thanks, Acme.".into())
        );
        assert_eq!(resolve_greeting(Some(""), "Thanks, Acme.").unwrap(), None);
        assert_eq!(
            resolve_greeting(Some("   "), "Thanks, Acme.").unwrap(),
            None
        );
        assert_eq!(
            resolve_greeting(Some("  Hi Globex\nSee attached. "), "Thanks").unwrap(),
            Some("Hi Globex\nSee attached.".into())
        );
        assert_eq!(resolve_greeting(None, "").unwrap(), None);
    }

    #[test]
    fn the_limit_counts_characters_after_trimming() {
        let at_limit = "é".repeat(EMAIL_GREETING_MAX_CHARS);
        assert!(clean_greeting(&format!("  {at_limit}  "))
            .unwrap()
            .is_some());
        let over = "a".repeat(EMAIL_GREETING_MAX_CHARS + 1);
        let err = clean_greeting(&over).unwrap_err();
        assert!(matches!(err, NigelError::GreetingInvalid(_)), "{err:?}");
        assert!(
            resolve_greeting(None, &over).is_err(),
            "a default is checked too"
        );
    }

    #[test]
    fn the_terminal_preview_prints_headers_attachments_body_and_the_placeholder_note() {
        let preview = EmailPreview {
            emails: vec![PreviewEmail {
                role: "client".into(),
                to: "Juniper Labs <ap@juniper.example>".into(),
                cc: vec!["ops@juniper.example".into()],
                subject: "Invoice #1001 from Initech".into(),
                body: format!("Thanks!\n\nPay now: {PREVIEW_LINK_PLACEHOLDER}\n"),
                attachments: vec!["invoice.pdf".into()],
            }],
            placeholders: vec![PREVIEW_LINK_PLACEHOLDER.into()],
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
