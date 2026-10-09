use serde::{Deserialize, Serialize};

use crate::error::{NigelError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentStatus {
    Draft,
    Sent,
    ChangesRequested,
    Accepted,
    Declined,
    Executed,
    Withdrawn,
}

impl DocumentStatus {
    pub const ALL: [DocumentStatus; 7] = [
        DocumentStatus::Draft,
        DocumentStatus::Sent,
        DocumentStatus::ChangesRequested,
        DocumentStatus::Accepted,
        DocumentStatus::Declined,
        DocumentStatus::Executed,
        DocumentStatus::Withdrawn,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            DocumentStatus::Draft => "draft",
            DocumentStatus::Sent => "sent",
            DocumentStatus::ChangesRequested => "changes_requested",
            DocumentStatus::Accepted => "accepted",
            DocumentStatus::Declined => "declined",
            DocumentStatus::Executed => "executed",
            DocumentStatus::Withdrawn => "withdrawn",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == value)
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            DocumentStatus::Withdrawn | DocumentStatus::Executed | DocumentStatus::Declined
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecipientRole {
    Signer,
    Collaborator,
}

impl RecipientRole {
    pub fn as_str(self) -> &'static str {
        match self {
            RecipientRole::Signer => "signer",
            RecipientRole::Collaborator => "collaborator",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "signer" => Some(RecipientRole::Signer),
            "collaborator" => Some(RecipientRole::Collaborator),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureRole {
    Client,
    Countersign,
}

impl SignatureRole {
    pub fn as_str(self) -> &'static str {
        match self {
            SignatureRole::Client => "client",
            SignatureRole::Countersign => "countersign",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "client" => Some(SignatureRole::Client),
            "countersign" => Some(SignatureRole::Countersign),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    Online,
    Manual,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Online => "online",
            Method::Manual => "manual",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "online" => Some(Method::Online),
            "manual" => Some(Method::Manual),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentKind {
    pub id: i64,
    pub name: String,
    pub active: bool,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: i64,
    pub client_id: i64,
    pub kind_id: i64,
    pub kind: String,
    pub title: String,
    #[serde(skip_serializing)]
    pub token: String,
    pub declined_at: Option<String>,
    pub decline_note: Option<String>,
    pub withdrawn_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub status: DocumentStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentVersion {
    pub id: i64,
    pub document_id: i64,
    pub number: i64,
    #[serde(skip_serializing)]
    pub file_path: String,
    pub checksum: String,
    pub sent_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recipient {
    pub id: i64,
    pub version_id: i64,
    pub role: RecipientRole,
    pub name: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub token: String,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Signature {
    pub id: i64,
    pub version_id: i64,
    pub recipient_id: Option<i64>,
    pub role: SignatureRole,
    pub name: String,
    pub email: Option<String>,
    pub method: Method,
    pub signed_at: String,
    pub typed_name: Option<String>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRequest {
    pub id: i64,
    pub version_id: i64,
    pub recipient_id: Option<i64>,
    pub name: String,
    pub email: Option<String>,
    pub method: Method,
    pub requested_at: String,
    pub note: String,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRecord {
    #[serde(flatten)]
    pub version: DocumentVersion,
    pub recipients: Vec<Recipient>,
    pub signatures: Vec<Signature>,
    pub change_requests: Vec<ChangeRequest>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRecord {
    #[serde(flatten)]
    pub document: Document,
    pub client_name: String,
    pub versions: Vec<VersionRecord>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentListRow {
    pub id: i64,
    pub title: String,
    pub kind: String,
    pub client_id: i64,
    pub client_name: Option<String>,
    pub status: DocumentStatus,
    pub latest_version: i64,
    pub sent_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRecipient {
    pub role: RecipientRole,
    pub name: String,
    pub email: String,
}

impl NewRecipient {
    /// Trims both parts. A collaborator with no name is named by their address;
    /// a signer is left unnamed, which `validate_recipient` refuses.
    pub fn new(role: RecipientRole, name: &str, email: &str) -> Self {
        let email = email.trim().to_string();
        let name = match (name.trim(), role) {
            ("", RecipientRole::Collaborator) => email.clone(),
            (name, _) => name.to_string(),
        };
        NewRecipient { role, name, email }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseKind<'a> {
    Accept { typed_name: &'a str },
    RequestChanges { note: &'a str },
}

pub const CHECKSUM_PREFIX: &str = "sha256:";
pub const NOTE_MAX_CHARS: usize = 4000;

pub fn gen_document_token() -> String {
    let mut bytes = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    hex::encode(bytes)
}

pub fn checksum_of(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{CHECKSUM_PREFIX}{}", hex::encode(Sha256::digest(bytes)))
}

fn invalid_recipient(raw: &str) -> NigelError {
    NigelError::Invalid(format!(
        "Invalid recipient: {raw} (expected \"Name <address>\" or an address)"
    ))
}

fn is_address(email: &str) -> bool {
    match email.split_once('@') {
        Some((local, domain)) => {
            !local.is_empty()
                && domain.contains('.')
                && !domain.contains('@')
                && !email.contains(char::is_whitespace)
        }
        None => false,
    }
}

/// The rules every recipient meets before a version is frozen to it, whichever
/// path named them: a name (the signer types theirs to accept) and an address
/// with a local part and a dotted domain, both safe in a mail header.
pub fn validate_recipient(r: &NewRecipient) -> Result<()> {
    let (name, email) = (r.name.trim(), r.email.trim());
    crate::invoicing::mailgun::validate_header_value(name, "recipient name")?;
    crate::invoicing::mailgun::validate_header_value(email, "recipient address")?;
    if !is_address(email) {
        return Err(NigelError::Invalid(format!(
            "Invalid recipient address: {email} (expected an address like name@example.com)"
        )));
    }
    if name.is_empty() {
        return Err(match r.role {
            RecipientRole::Signer => NigelError::Conflict {
                code: "signer_name_required",
                message: format!("The signer {email} needs a name to type when accepting."),
            },
            RecipientRole::Collaborator => {
                NigelError::Invalid(format!("The collaborator {email} needs a name."))
            }
        });
    }
    Ok(())
}

pub fn parse_recipient(raw: &str, role: RecipientRole) -> Result<NewRecipient> {
    let raw = raw.trim();
    crate::invoicing::mailgun::validate_header_value(raw, "recipient")?;
    let (name, email) = match raw.rsplit_once('<') {
        Some((name, rest)) => {
            let email = rest
                .strip_suffix('>')
                .ok_or_else(|| invalid_recipient(raw))?;
            (name, email)
        }
        None => ("", raw),
    };
    let recipient = NewRecipient::new(role, name, email);
    validate_recipient(&recipient).map_err(|e| match e {
        NigelError::Conflict {
            code: "signer_name_required",
            ..
        } => NigelError::Invalid(format!(
            "The signer needs a name to type when accepting: write it as \"Name <{}>\".",
            recipient.email
        )),
        NigelError::Invalid(_) => invalid_recipient(raw),
        e => e,
    })?;
    Ok(recipient)
}

pub fn validate_note(note: &str) -> Result<String> {
    let count = note.chars().count();
    if note.trim().is_empty() || count > NOTE_MAX_CHARS {
        return Err(NigelError::Invalid(format!(
            "A change request note must be 1 to {NOTE_MAX_CHARS} characters, got {count}."
        )));
    }
    if note
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err(NigelError::Invalid(
            "A change request note may not carry control characters.".into(),
        ));
    }
    Ok(note.to_string())
}

pub fn validate_moment(value: &str, what: &str) -> Result<String> {
    if let Ok(instant) = chrono::DateTime::parse_from_rfc3339(value.trim()) {
        return Ok(instant
            .with_timezone(&chrono::Utc)
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string());
    }
    crate::invoicing::invoices::validate_date(value, what)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_128_bit_hex_and_distinct() {
        let a = gen_document_token();
        let b = gen_document_token();
        assert_eq!(a.len(), 32);
        assert!(a
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_ne!(a, b);
    }

    #[test]
    fn checksums_carry_their_algorithm() {
        assert_eq!(
            checksum_of(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn every_status_round_trips_through_its_wire_word() {
        for status in DocumentStatus::ALL {
            assert_eq!(DocumentStatus::parse(status.as_str()), Some(status));
            assert_eq!(serde_json::to_value(status).unwrap(), status.as_str());
        }
        assert_eq!(
            DocumentStatus::ChangesRequested.as_str(),
            "changes_requested"
        );
        assert!(DocumentStatus::Executed.is_terminal());
        assert!(!DocumentStatus::Accepted.is_terminal());
    }

    #[test]
    fn a_recipient_is_name_and_angle_bracketed_address() {
        let r = parse_recipient("Pat Example <pat@juniper.test>", RecipientRole::Signer).unwrap();
        assert_eq!(
            (r.name.as_str(), r.email.as_str()),
            ("Pat Example", "pat@juniper.test")
        );
        let bare = parse_recipient("sam@juniper.test", RecipientRole::Collaborator).unwrap();
        assert_eq!(bare.name, "sam@juniper.test");
        assert!(parse_recipient("pat@juniper.test", RecipientRole::Signer).is_err());
        assert!(parse_recipient(
            "Pat <pat@juniper.test>\r\nBcc: x@y.test",
            RecipientRole::Signer
        )
        .is_err());
        assert!(parse_recipient("Pat Example <>", RecipientRole::Signer).is_err());
    }

    #[test]
    fn a_recipient_address_has_a_local_part_and_a_dotted_domain() {
        for bad in [
            "",
            "pat",
            "@juniper.test",
            "pat@",
            "pat@juniper",
            "pat@@juniper.test",
            "pat @juniper.test",
        ] {
            assert!(
                parse_recipient(&format!("Pat Example <{bad}>"), RecipientRole::Signer).is_err(),
                "{bad}"
            );
        }
        let collaborator =
            NewRecipient::new(RecipientRole::Collaborator, "  ", " sam@juniper.test ");
        assert_eq!(collaborator.name, "sam@juniper.test");
        assert_eq!(collaborator.email, "sam@juniper.test");
        assert!(validate_recipient(&collaborator).is_ok());
        let unnamed = NewRecipient::new(RecipientRole::Signer, "", "pat@juniper.test");
        assert!(matches!(
            validate_recipient(&unnamed),
            Err(NigelError::Conflict {
                code: "signer_name_required",
                ..
            })
        ));
    }

    #[test]
    fn a_note_is_one_to_four_thousand_characters_of_text() {
        assert!(validate_note("").is_err());
        assert!(validate_note("   \n").is_err());
        assert!(validate_note(&"é".repeat(4000)).is_ok());
        assert!(validate_note(&"é".repeat(4001)).is_err());
        assert!(validate_note("line one\nline two\ttabbed").is_ok());
        assert!(validate_note("bell\u{7}").is_err());
    }

    #[test]
    fn a_moment_is_a_day_or_an_rfc3339_instant() {
        assert_eq!(
            validate_moment("2026-10-5", "accept").unwrap(),
            "2026-10-05"
        );
        assert_eq!(
            validate_moment("2026-10-05T10:04:11-07:00", "response").unwrap(),
            "2026-10-05T17:04:11Z"
        );
        assert!(validate_moment("yesterday", "accept").is_err());
    }
}
