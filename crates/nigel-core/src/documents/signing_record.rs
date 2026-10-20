//! The signing record: a one-page PDF beside an executed version that names the
//! signed file's checksum and every signature on it. The signed PDF itself is
//! never touched; the record is a separate file.
//!
//! It comes in two variants. The **full** record carries the signer's IP
//! address and browser and goes only to the operator (the copy kept beside the
//! version's PDF in the data directory, and the API download) and to the
//! signer (their confirmation email). The **redacted** record leaves those two
//! out and says so; it is what collaborators are emailed and what is published
//! under `d/`, where anyone holding a page link can fetch it.
use std::path::Path;

use rusqlite::Connection;

use super::model::{DocumentStatus, Method, SignatureRole};
use super::render::attachment_name;
use super::store::{get_document, latest_sent_version, signatures, write_version_file};
use crate::error::{NigelError, Result};
use crate::invoicing::clients::get_client;

/// One signature as the record prints it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedSignature {
    pub role: SignatureRole,
    pub name: String,
    pub typed_name: Option<String>,
    pub method: Method,
    pub signed_at: String,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}

/// Everything the record prints, already reduced to plain text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningRecord {
    pub company: String,
    pub title: String,
    pub kind: String,
    pub client: String,
    pub version: i64,
    pub checksum: String,
    /// The client's acceptance first, then the countersignature.
    pub signatures: Vec<RecordedSignature>,
    /// This is the redacted variant: IP addresses and browsers are left out,
    /// and the record says the sender keeps them.
    pub network_details_withheld: bool,
}

impl SigningRecord {
    /// The variant for collaborators and the public bucket: the same record
    /// with every IP address and browser removed.
    pub fn redacted(&self) -> SigningRecord {
        SigningRecord {
            signatures: self
                .signatures
                .iter()
                .map(|s| RecordedSignature {
                    ip: None,
                    user_agent: None,
                    ..s.clone()
                })
                .collect(),
            network_details_withheld: true,
            ..self.clone()
        }
    }
}

/// The warning a countersignature carries in a build without the `pdf`
/// feature.
pub const RECORD_UNAVAILABLE: &str = "Warning: this build has no PDF support, so no signing record was generated. Use a build with the `pdf` feature (the release builds have it), then run `nigel document signing-record` for this document.";

/// The refusal a request for a record answers in a build without the `pdf`
/// feature.
pub const PDF_UNAVAILABLE: &str = "This build has no PDF support, so it cannot generate a signing record. Use a build with the `pdf` feature (the release builds have it).";

/// `text` with every control character removed, along with the Unicode line
/// separators and bidirectional overrides, so a value a recipient typed prints
/// as the one line of plain text it claims to be.
pub fn plain_text(text: &str) -> String {
    text.chars()
        .filter(|c| {
            !c.is_control()
                && !matches!(
                    c,
                    '\u{2028}' | '\u{2029}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
                )
        })
        .collect()
}

/// The download name: the version PDF's name with `-signing-record` before
/// the extension, e.g. `Website-rebuild-v3-signing-record.pdf`.
pub fn signing_record_name(title: &str, version: i64) -> String {
    let pdf = attachment_name(title, version);
    let stem = pdf.strip_suffix(".pdf").unwrap_or(&pdf);
    format!("{stem}-signing-record.pdf")
}

/// Where the local copy lives, relative to the data directory: beside the
/// version's own PDF.
pub fn signing_record_rel(document_id: i64, version: i64) -> String {
    format!("documents/{document_id}/v{version}-signing-record.pdf")
}

/// The record for an executed document's signed version. Refused for a
/// document that is not executed.
pub fn signing_record(conn: &Connection, id: i64, company: &str) -> Result<SigningRecord> {
    let document = get_document(conn, id)?;
    if document.status != DocumentStatus::Executed {
        return Err(NigelError::Conflict {
            code: "document_not_executed",
            message: format!(
                "Document #{id} is {}; a signing record exists only once it is executed.",
                document.status.as_str().replace('_', " ")
            ),
        });
    }
    let version = latest_sent_version(conn, id)?.ok_or_else(|| {
        NigelError::Other(format!(
            "Document #{id} is executed but has no sent version."
        ))
    })?;
    let client = get_client(conn, document.client_id)?;
    let mut stamps = signatures(conn, version.id)?;
    stamps.sort_by_key(|s| match s.role {
        SignatureRole::Client => 0,
        SignatureRole::Countersign => 1,
    });
    let online = |method: Method, value: Option<String>| match method {
        Method::Online => value.map(|v| plain_text(&v)),
        Method::Manual => None,
    };
    Ok(SigningRecord {
        company: plain_text(company),
        title: plain_text(&document.title),
        kind: plain_text(&document.kind),
        client: plain_text(&client.name),
        version: version.number,
        checksum: version.checksum,
        signatures: stamps
            .into_iter()
            .map(|s| RecordedSignature {
                role: s.role,
                name: plain_text(&s.name),
                typed_name: s.typed_name.map(|t| plain_text(&t)),
                method: s.method,
                signed_at: plain_text(&s.signed_at),
                ip: online(s.method, s.ip),
                user_agent: online(s.method, s.user_agent),
            })
            .collect(),
        network_details_withheld: false,
    })
}

/// The record's PDF bytes, or `None` in a build without the `pdf` feature.
pub fn render_signing_record(record: &SigningRecord) -> Option<Result<Vec<u8>>> {
    #[cfg(feature = "pdf")]
    {
        Some(crate::pdf::render_signing_record(record))
    }
    #[cfg(not(feature = "pdf"))]
    {
        let _ = record;
        None
    }
}

/// A generated record in both variants. `bytes` is the full record, which is
/// the one written to the data directory; `redacted` is the one published and
/// sent to collaborators.
#[derive(Debug, Clone)]
pub struct GeneratedRecord {
    pub version: i64,
    pub filename: String,
    pub bytes: Vec<u8>,
    pub redacted: Vec<u8>,
}

/// The full record as kept in the data directory.
#[derive(Debug, Clone)]
pub struct KeptRecord {
    pub version: i64,
    pub filename: String,
    pub bytes: Vec<u8>,
}

/// Build, render and write the record for an executed document, replacing any
/// earlier copy. `Ok(None)` in a build without the `pdf` feature.
pub fn generate_signing_record(
    conn: &Connection,
    data_dir: &Path,
    id: i64,
    company: &str,
) -> Result<Option<GeneratedRecord>> {
    let record = signing_record(conn, id, company)?;
    let Some(bytes) = render_signing_record(&record) else {
        return Ok(None);
    };
    let bytes = bytes?;
    let redacted = render_signing_record(&record.redacted())
        .expect("rendered once, so the pdf feature is on")?;
    write_version_file(data_dir, &signing_record_rel(id, record.version), &bytes)?;
    let title = get_document(conn, id)?.title;
    Ok(Some(GeneratedRecord {
        version: record.version,
        filename: signing_record_name(&title, record.version),
        bytes,
        redacted,
    }))
}

/// The local copy of an executed document's full record, or `None` when none
/// has been written.
pub fn read_signing_record(
    conn: &Connection,
    data_dir: &Path,
    id: i64,
) -> Result<Option<KeptRecord>> {
    let document = get_document(conn, id)?;
    let Some(version) = latest_sent_version(conn, id)? else {
        return Ok(None);
    };
    let path = data_dir.join(signing_record_rel(id, version.number));
    match std::fs::read(&path) {
        Ok(bytes) => Ok(Some(KeptRecord {
            version: version.number,
            filename: signing_record_name(&document.title, version.number),
            bytes,
        })),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::record::{record_countersign, record_manual_accept};
    use crate::documents::testing::{sent_document_with_fakes, test_conn};

    #[test]
    fn plain_text_removes_control_characters_and_line_separators() {
        assert_eq!(
            plain_text("Pat\r\nExample\u{0}\u{1b}[2J\u{85}\u{2028}\u{202e}"),
            "PatExample[2J"
        );
        assert_eq!(plain_text("Zoë Ñúñez"), "Zoë Ñúñez");
    }

    #[test]
    fn the_record_name_follows_the_version_pdf() {
        assert_eq!(
            signing_record_name("Website rebuild", 3),
            "Website-rebuild-v3-signing-record.pdf"
        );
        assert_eq!(
            signing_record_name("—", 1),
            "document-v1-signing-record.pdf"
        );
    }

    #[test]
    fn a_record_is_refused_until_the_document_is_executed() {
        let (dir, conn) = test_conn();
        let (id, _, _) = sent_document_with_fakes(&conn, dir.path());
        record_manual_accept(&conn, id, "Pat Example", "2026-10-06").unwrap();
        let err = signing_record(&conn, id, "Initech").unwrap_err();
        assert!(
            matches!(
                err,
                NigelError::Conflict {
                    code: "document_not_executed",
                    ..
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn the_record_lists_the_acceptance_then_the_countersignature() {
        let (dir, conn) = test_conn();
        let (id, _, _) = sent_document_with_fakes(&conn, dir.path());
        record_manual_accept(&conn, id, "Pat Example", "2026-10-06").unwrap();
        record_countersign(&conn, id, "Sam Example", "2026-10-07").unwrap();
        let record = signing_record(&conn, id, "Initech").unwrap();
        assert_eq!(
            (
                record.title.as_str(),
                record.client.as_str(),
                record.version
            ),
            ("Website rebuild", "Cedar Systems", 1)
        );
        assert!(record.checksum.starts_with("sha256:"));
        let roles: Vec<_> = record.signatures.iter().map(|s| s.role).collect();
        assert_eq!(roles, [SignatureRole::Client, SignatureRole::Countersign]);
        assert!(record.signatures.iter().all(|s| s.ip.is_none()));
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn the_pdf_prints_every_field_with_recipient_text_as_plain_text() {
        use crate::documents::testing::accept_online;
        let (dir, conn) = test_conn();
        let (id, _, _) = sent_document_with_fakes(&conn, dir.path());
        accept_online(
            &conn,
            id,
            "Pat\u{1b}[31m Example\r\n",
            "Mozilla/5.0 (X11)\u{7}\u{2028}Firefox/131.0",
        );
        record_countersign(&conn, id, "Sam Example", "2026-10-07").unwrap();
        let record = signing_record(&conn, id, "Initech").unwrap();
        let client = &record.signatures[0];
        assert_eq!(client.typed_name.as_deref(), Some("Pat[31m Example"));
        assert_eq!(
            client.user_agent.as_deref(),
            Some("Mozilla/5.0 (X11)Firefox/131.0")
        );

        let bytes = render_signing_record(&record).unwrap().unwrap();
        let text = crate::pdf::extract_text(&bytes);
        for expected in [
            "Signing record",
            "Initech",
            "Website rebuild",
            "Proposal",
            "Cedar Systems",
            record.checksum.as_str(),
            "Client acceptance",
            "Pat Example",
            "Pat[31m Example",
            "Online",
            "2026-10-06 17:04:11 UTC",
            "203.0.113.7",
            "Mozilla/5.0 (X11)Firefox/131.0",
            "Countersignature",
            "Sam Example",
            "Manual",
            "2026-10-07",
        ] {
            assert!(text.contains(expected), "{expected:?} missing from {text}");
        }
        assert!(text.find("Client acceptance") < text.find("Countersignature"));
        assert!(!text.contains('\u{1b}') && !text.contains('\u{7}'));
        assert_eq!(
            crate::pdf::page_streams(&bytes).len(),
            1,
            "the record is one page"
        );
        assert!(!text.contains("This copy leaves out"));

        let redacted = record.redacted();
        assert!(redacted.network_details_withheld);
        assert!(redacted
            .signatures
            .iter()
            .all(|s| s.ip.is_none() && s.user_agent.is_none()));
        let bytes = render_signing_record(&redacted).unwrap().unwrap();
        let text = crate::pdf::extract_text(&bytes);
        for absent in ["203.0.113.7", "Mozilla", "Browser"] {
            assert!(
                !text.contains(absent),
                "{absent:?} in the redacted record: {text}"
            );
        }
        for present in [
            record.checksum.as_str(),
            "Pat[31m Example",
            "Online",
            "2026-10-06 17:04:11 UTC",
            "Sam Example",
            "Initech keeps them in",
            "the full record.",
        ] {
            assert!(text.contains(present), "{present:?} missing from {text}");
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn generating_writes_the_local_copy_beside_the_version_pdf_and_leaves_it_alone() {
        let (dir, conn) = test_conn();
        let (id, _, _) = sent_document_with_fakes(&conn, dir.path());
        record_manual_accept(&conn, id, "Pat Example", "2026-10-06").unwrap();
        record_countersign(&conn, id, "Sam Example", "2026-10-07").unwrap();
        let version = crate::documents::store::latest_version(&conn, id).unwrap();
        let signed = std::fs::read(dir.path().join(&version.file_path)).unwrap();

        let generated = generate_signing_record(&conn, dir.path(), id, "Initech")
            .unwrap()
            .unwrap();
        assert_eq!(generated.filename, "Website-rebuild-v1-signing-record.pdf");
        assert!(dir
            .path()
            .join(format!("documents/{id}/v1-signing-record.pdf"))
            .exists());
        let read = read_signing_record(&conn, dir.path(), id).unwrap().unwrap();
        assert_eq!(read.bytes, generated.bytes);
        assert_eq!(
            std::fs::read(dir.path().join(&version.file_path)).unwrap(),
            signed
        );
        assert_eq!(
            crate::documents::model::checksum_of(&signed),
            version.checksum
        );
    }
}
