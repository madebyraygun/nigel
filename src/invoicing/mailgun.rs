use crate::error::{NigelError, Result};
use crate::invoicing::gateway::Mailer;

/// The sender identity of one message: what Mailgun puts in `From` and, when
/// set, in `Reply-To`.
///
/// A value type with no configuration in it, because `src/invoicing/` reads no
/// settings — `cli::invoice::build_clients` resolves it and passes it in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailEnvelope {
    pub from_address: String,
    pub from_name: Option<String>,
    pub reply_to: Option<String>,
}

/// RFC 5322 `atext`, the characters a display name may carry unquoted.
fn is_atext(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            '!' | '#'
                | '$'
                | '%'
                | '&'
                | '\''
                | '*'
                | '+'
                | '-'
                | '/'
                | '='
                | '?'
                | '^'
                | '_'
                | '`'
                | '{'
                | '|'
                | '}'
                | '~'
        )
        || !c.is_ascii()
}

/// `Name <addr>` per RFC 5322, or the bare address when there is no name.
///
/// A name is quoted whenever it carries anything outside `atext` plus spaces —
/// the comma, the colon, the brackets, `@`, the backslash and the quote — and
/// inside the quotes only `"` and `\` are escaped, which is all `quoted-string`
/// allows. A name is never concatenated raw into the header.
///
/// A UTF-8 name passes through unencoded: Mailgun's API is UTF-8 and accepts it
/// in the `from` form field, where RFC 2047 encoded-words would cost a base64
/// dependency for a case the upstream already handles.
pub fn format_address(display_name: Option<&str>, address: &str) -> String {
    let name = display_name.map(str::trim).unwrap_or("");
    if name.is_empty() {
        return address.to_string();
    }
    if name.chars().all(|c| is_atext(c) || c == ' ') {
        return format!("{name} <{address}>");
    }
    let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\" <{address}>")
}

/// The sentence for a from address that is not on the verified sending domain,
/// or `None` when it is.
///
/// A warning rather than a refusal: a Mailgun domain of `mg.example.com`
/// sending for `billing@example.com` is a common, deliverable setup, and Nigel
/// cannot see which sender identities the operator has verified. Naming no
/// value keeps the configured address out of every log and response.
pub fn from_address_domain_warning(from: &str, domain: &str) -> Option<String> {
    let on_domain = from
        .rsplit_once('@')
        .is_some_and(|(_, host)| host.eq_ignore_ascii_case(domain));
    if on_domain {
        return None;
    }
    Some(
        "from_email is not on mailgun_domain — Mailgun will refuse it unless the sender \
         is separately verified."
            .to_string(),
    )
}

/// Characters no header value may carry.
///
/// The ASCII controls, the C1 controls, and the three Unicode characters a
/// parser may treat as a line ending — `NEL`, `LINE SEPARATOR` and `PARAGRAPH
/// SEPARATOR`. Nigel sends UTF-8 straight through to Mailgun, so "is it a
/// newline" is not a question ASCII alone answers.
fn is_forbidden_header_char(c: char) -> bool {
    c.is_ascii_control()
        || ('\u{0080}'..='\u{009f}').contains(&c)
        || matches!(c, '\u{2028}' | '\u{2029}')
}

/// No header value may carry a control character.
///
/// Refused rather than stripped: stripping would silently send a message the
/// operator did not write, and a `from_name` of `"Bluepeak\r\nBcc: someone@else"`
/// would otherwise add a recipient nobody chose. The value comes from local
/// configuration, but `NIGEL_FROM_NAME` is an environment variable and
/// environments get assembled by CI systems.
pub fn validate_header_value(value: &str, what: &str) -> Result<()> {
    if value.chars().any(is_forbidden_header_char) {
        return Err(NigelError::Invalid(format!(
            "{what} may not contain control characters"
        )));
    }
    Ok(())
}

/// The refusal an address that already carries a display name gets.
///
/// `from_email` is composed into the `From` header by [`format_address`],
/// which assumes a bare `addr-spec`. An installation that put `Acme LLC
/// <billing@mg.example.com>` in `from_email` — the only way to get a display
/// name before `from_name` existed — would otherwise produce a nested
/// `name-addr` that Mailgun rejects, and it would reject it *after* the Stripe
/// link was created and the page published. Refusing up front costs nothing.
pub fn validate_bare_address(address: &str, what: &str) -> Result<()> {
    if address.contains('<') || address.contains('>') {
        return Err(NigelError::Invalid(format!(
            "{what} must be a bare address with no display name — put the name in from_name"
        )));
    }
    Ok(())
}

pub fn message_fields(
    envelope: &EmailEnvelope,
    to: &str,
    subject: &str,
    html: &str,
) -> Vec<(String, String)> {
    let mut fields = vec![
        (
            "from".into(),
            format_address(envelope.from_name.as_deref(), &envelope.from_address),
        ),
        ("to".into(), to.to_string()),
        ("subject".into(), subject.to_string()),
        ("html".into(), html.to_string()),
    ];
    // Mailgun's form field for a custom header. An unset reply-to emits no
    // field at all rather than an empty one.
    if let Some(reply_to) = &envelope.reply_to {
        fields.push(("h:Reply-To".into(), reply_to.clone()));
    }
    fields
}

fn ensure_success(status: reqwest::StatusCode, body: &str) -> Result<()> {
    if status.is_success() {
        return Ok(());
    }
    Err(NigelError::Other(format!("mailgun {status}: {body}")))
}

pub struct MailgunClient {
    pub api_key: String,
    pub domain: String,
    pub envelope: EmailEnvelope,
}

impl Mailer for MailgunClient {
    fn send_invoice(&self, to: &str, subject: &str, html: &str, pdf: &[u8]) -> Result<()> {
        let url = format!("https://api.mailgun.net/v3/{}/messages", self.domain);

        let mut form = reqwest::blocking::multipart::Form::new();
        for (name, value) in message_fields(&self.envelope, to, subject, html) {
            form = form.text(name, value);
        }
        let part = reqwest::blocking::multipart::Part::bytes(pdf.to_vec())
            .file_name("invoice.pdf")
            .mime_str("application/pdf")
            .map_err(|e| NigelError::Other(format!("mailgun attachment: {e}")))?;
        form = form.part("attachment", part);

        let resp = crate::invoicing::http_client()
            .post(&url)
            .basic_auth("api", Some(&self.api_key))
            .multipart(form)
            .send()
            .map_err(|e| NigelError::Other(format!("mailgun request: {e}")))?;
        let status = resp.status();
        let body = resp.text().map_err(|e| NigelError::Other(e.to_string()))?;
        ensure_success(status, &body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(name: Option<&str>, reply_to: Option<&str>) -> EmailEnvelope {
        EmailEnvelope {
            from_address: "billing@mg.example.com".into(),
            from_name: name.map(str::to_string),
            reply_to: reply_to.map(str::to_string),
        }
    }

    #[test]
    fn message_fields_include_from_to_subject_html() {
        let f = message_fields(
            &envelope(None, None),
            "a@b.test",
            "Invoice #1248",
            "<p>hi</p>",
        );
        assert!(f.contains(&("from".into(), "billing@mg.example.com".into())));
        assert!(f.contains(&("to".into(), "a@b.test".into())));
        assert!(f.contains(&("subject".into(), "Invoice #1248".into())));
        assert!(f.contains(&("html".into(), "<p>hi</p>".into())));
    }

    #[test]
    fn a_plain_display_name_is_not_quoted() {
        assert_eq!(
            format_address(Some("Bluepeak"), "b@e.test"),
            "Bluepeak <b@e.test>"
        );
    }

    #[test]
    fn no_display_name_is_a_bare_address() {
        assert_eq!(format_address(None, "b@e.test"), "b@e.test");
        assert_eq!(format_address(Some("   "), "b@e.test"), "b@e.test");
    }

    #[test]
    fn a_comma_or_a_quote_is_encoded_not_concatenated() {
        assert_eq!(
            format_address(Some("Carter, Sam"), "b@e.test"),
            "\"Carter, Sam\" <b@e.test>"
        );
        assert_eq!(
            format_address(Some("Sam \"Nigel\" Carter"), "b@e.test"),
            "\"Sam \\\"Nigel\\\" Carter\" <b@e.test>"
        );
        assert_eq!(
            format_address(Some("Bluepeak \\ Co"), "b@e.test"),
            "\"Bluepeak \\\\ Co\" <b@e.test>"
        );
        for special in [
            "a;b", "a:b", "a<b", "a>b", "a@b", "a(b", "a)b", "a[b", "a]b",
        ] {
            let out = format_address(Some(special), "b@e.test");
            assert!(out.starts_with('"'), "{special} must be quoted, got {out}");
        }
    }

    /// Deliberate: Mailgun's API is UTF-8 and RFC 2047 would cost a base64
    /// dependency for a case the upstream already handles.
    #[test]
    fn a_utf8_display_name_passes_through_unencoded() {
        assert_eq!(
            format_address(Some("Bluepeak — Books"), "b@e.test"),
            "Bluepeak — Books <b@e.test>"
        );
    }

    #[test]
    fn a_header_value_may_not_carry_a_newline() {
        let err = validate_header_value("Bluepeak\r\nBcc: someone@else.test", "from_name")
            .unwrap_err()
            .to_string();
        assert!(err.contains("from_name"), "got: {err}");
        assert!(
            !err.contains("someone@else.test"),
            "the refusal must not echo the value"
        );
        assert!(validate_header_value("Bluepeak", "from_name").is_ok());
    }

    /// The refusal must describe what it actually refuses: every control
    /// character, not only `\r` and `\n`.
    #[test]
    fn the_refusal_names_control_characters_rather_than_line_breaks_alone() {
        let err = validate_header_value("Ray\u{0}gun", "from_name")
            .unwrap_err()
            .to_string();
        assert!(err.contains("control characters"), "got: {err}");
    }

    #[test]
    fn a_unicode_line_separator_is_refused_too() {
        // A header is UTF-8 all the way to Mailgun, so `is_ascii_control` is
        // not the whole set of things a parser may read as a new line.
        for sneaky in [
            "Bluepeak\u{85}Bcc: x@y.test",
            "Bluepeak\u{2028}x",
            "Bluepeak\u{2029}x",
        ] {
            assert!(
                validate_header_value(sneaky, "from_name").is_err(),
                "{sneaky:?} slipped through"
            );
        }
        // A legitimate non-ASCII name still passes.
        assert!(validate_header_value("Bluepeak — Books", "from_name").is_ok());
    }

    #[test]
    fn a_from_address_that_already_carries_a_display_name_is_refused() {
        // The old way to get a display name. Composed through `format_address`
        // it would produce a nested `name-addr` Mailgun rejects — after the
        // Stripe link and the upload.
        let err = validate_bare_address("Acme LLC <billing@mg.example.com>", "from_email")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("from_email") && err.contains("from_name"),
            "got: {err}"
        );
        assert!(!err.contains("billing@mg.example.com"), "got: {err}");
        assert!(validate_bare_address("billing@mg.example.com", "from_email").is_ok());
    }

    #[test]
    fn a_from_address_off_the_mailgun_domain_warns_rather_than_refusing() {
        assert_eq!(
            from_address_domain_warning("billing@mg.example.com", "mg.example.com"),
            None
        );
        assert_eq!(
            from_address_domain_warning("BILLING@MG.EXAMPLE.COM", "mg.example.com"),
            None
        );
        for off_domain in [
            "billing@example.com",
            "billing@sub.mg.example.com",
            "billing",
        ] {
            let warning = from_address_domain_warning(off_domain, "mg.example.com")
                .unwrap_or_else(|| panic!("{off_domain} should warn"));
            assert!(
                warning.contains("from_email") && warning.contains("mailgun_domain"),
                "got: {warning}"
            );
            assert!(
                !warning.contains(off_domain),
                "the warning must name keys, not values: {warning}"
            );
        }
    }

    #[test]
    fn a_reply_to_becomes_the_mailgun_header_field_and_is_absent_otherwise() {
        let with = message_fields(
            &envelope(Some("Bluepeak"), Some("sam@example.com")),
            "a@b.test",
            "Invoice #1248",
            "<p>hi</p>",
        );
        assert!(with.contains(&("from".into(), "Bluepeak <billing@mg.example.com>".into())));
        assert!(with.contains(&("h:Reply-To".into(), "sam@example.com".into())));

        let without = message_fields(&envelope(None, None), "a@b.test", "s", "<p>hi</p>");
        assert!(
            without.iter().all(|(k, _)| k != "h:Reply-To"),
            "an unset reply-to emits no field at all"
        );
        assert!(without.contains(&("from".into(), "billing@mg.example.com".into())));
    }

    #[test]
    fn ensure_success_rejects_non_2xx_and_keeps_mailgun_message() {
        let err = ensure_success(
            reqwest::StatusCode::UNAUTHORIZED,
            r#"{"message":"Invalid private key"}"#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("401"), "status missing from {msg:?}");
        assert!(
            msg.contains("Invalid private key"),
            "mailgun message missing from {msg:?}"
        );
    }

    #[test]
    fn ensure_success_accepts_2xx() {
        assert!(ensure_success(reqwest::StatusCode::OK, "{}").is_ok());
    }
}
