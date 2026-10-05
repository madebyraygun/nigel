use std::path::Path;

use comfy_table::{Cell, Table};

use nigel_core::db::get_connection;
use nigel_core::documents::kinds::{add_kind, deactivate_kind, list_kinds, rename_kind};
use nigel_core::documents::model::{
    DocumentKind, DocumentListRow, DocumentRecord, DocumentStatus, Method, SignatureRole,
};
use nigel_core::documents::send::write_preview;
use nigel_core::documents::store::{
    document_record, file_document, get_document, latest_version, list_documents, DocumentFilter,
    NewDocument,
};
use nigel_core::error::{NigelError, Result};
use nigel_core::invoicing::clients::ensure_client_exists;
use nigel_core::invoicing::wiring::company_name;
use nigel_core::settings::{documents_config, get_data_dir};

use crate::cli::DocumentKindsCommands;

pub fn add(client: i64, kind: &str, title: &str, file: &Path, today: &str) -> Result<()> {
    let pdf = std::fs::read(file)
        .map_err(|e| NigelError::Invalid(format!("Could not read {}: {e}", file.display())))?;
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let new = NewDocument {
        client_id: client,
        kind,
        title,
    };
    let id = file_document(&conn, &get_data_dir(), &new, &pdf, today)?;
    let document = get_document(&conn, id)?;
    let version = latest_version(&conn, id)?;
    println!(
        "Filed document #{id}: {} ({}, draft, {})",
        document.title, document.kind, version.checksum
    );
    Ok(())
}

pub fn kinds(command: Option<DocumentKindsCommands>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    match command.unwrap_or(DocumentKindsCommands::List) {
        DocumentKindsCommands::List => {
            println!("{}", format_kind_list(&list_kinds(&conn, true)?));
        }
        DocumentKindsCommands::Add { name } => {
            let id = add_kind(&conn, &name)?;
            println!("Added document kind #{id}: {}", name.trim());
        }
        DocumentKindsCommands::Rename { id, name } => {
            rename_kind(&conn, id, &name)?;
            println!("Renamed document kind #{id} to {}", name.trim());
        }
        DocumentKindsCommands::Deactivate { id } => {
            deactivate_kind(&conn, id)?;
            println!("Deactivated document kind #{id}");
        }
    }
    Ok(())
}

pub fn format_kind_list(kinds: &[DocumentKind]) -> String {
    let mut table = Table::new();
    table.set_header(vec!["ID", "Name", "State"]);
    for kind in kinds {
        table.add_row(vec![
            Cell::new(kind.id),
            Cell::new(&kind.name),
            Cell::new(if kind.active { "active" } else { "inactive" }),
        ]);
    }
    table.to_string()
}

pub fn list(client: Option<i64>, status: Option<&str>, kind: Option<&str>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    if let Some(id) = client {
        ensure_client_exists(&conn, id)?;
    }
    let filter = DocumentFilter {
        client_id: client,
        status: status.map(parse_status).transpose()?,
        kind: kind.map(str::to_string),
    };
    println!("{}", format_document_list(&list_documents(&conn, &filter)?));
    Ok(())
}

pub fn show(id: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    println!("{}", format_document_show(&document_record(&conn, id)?));
    Ok(())
}

pub fn preview(id: i64, output_dir: Option<&str>) -> Result<()> {
    let data_dir = get_data_dir();
    let conn = get_connection(&data_dir.join("nigel.db"))?;
    let out = match output_dir {
        Some(dir) => Path::new(dir).to_path_buf(),
        None => data_dir.join("previews"),
    };
    let config = documents_config();
    let files = write_preview(
        &conn,
        &data_dir,
        id,
        &company_name(&conn),
        config.document_response_url.as_deref(),
        &out,
    )?;
    for page in &files.pages {
        println!("{}", page.display());
    }
    println!("{}", files.pdf.display());
    Ok(())
}

fn parse_status(value: &str) -> Result<DocumentStatus> {
    DocumentStatus::parse(value).ok_or_else(|| {
        let valid: Vec<&str> = DocumentStatus::ALL.iter().map(|s| s.as_str()).collect();
        NigelError::Invalid(format!(
            "Unknown status: {value}. Use one of: {}.",
            valid.join(", ")
        ))
    })
}

pub fn format_document_list(rows: &[DocumentListRow]) -> String {
    let mut table = Table::new();
    table.set_header(vec![
        "ID", "Title", "Kind", "Client", "Status", "Version", "Sent", "Updated",
    ]);
    for row in rows {
        table.add_row(vec![
            Cell::new(row.id),
            Cell::new(&row.title),
            Cell::new(&row.kind),
            Cell::new(row.client_name.as_deref().unwrap_or("")),
            Cell::new(row.status.as_str()),
            Cell::new(format!("v{}", row.latest_version)),
            Cell::new(row.sent_at.as_deref().unwrap_or("")),
            Cell::new(date_part(&row.updated_at)),
        ]);
    }
    table.to_string()
}

fn date_part(moment: &str) -> &str {
    moment.get(..10).unwrap_or(moment)
}

fn push_note(out: &mut String, note: &str) {
    for line in note.lines() {
        out.push_str("    | ");
        out.push_str(line);
        out.push('\n');
    }
}

fn evidence(method: Method, ip: Option<&str>) -> String {
    match (method, ip) {
        (Method::Online, Some(ip)) => format!(", ip {ip}"),
        _ => String::new(),
    }
}

pub fn format_document_show(record: &DocumentRecord) -> String {
    let document = &record.document;
    let mut out = format!(
        "Document #{}  [{}]  {}\nTitle:    {}\nClient:   {}\nFiled:    {}\n",
        document.id,
        document.status.as_str(),
        document.kind,
        document.title,
        record.client_name,
        date_part(&document.created_at),
    );
    if let Some(declined_at) = &document.declined_at {
        out.push_str(&format!("Declined: {}\n", date_part(declined_at)));
        push_note(&mut out, document.decline_note.as_deref().unwrap_or(""));
    }
    out.push('\n');
    for record in &record.versions {
        let version = &record.version;
        let state = match &version.sent_at {
            Some(sent_at) => format!("sent {}", date_part(sent_at)),
            None => "draft".to_string(),
        };
        out.push_str(&format!(
            "Version {}  {}  {state}\n",
            version.number, version.checksum
        ));
        for recipient in &record.recipients {
            out.push_str(&format!(
                "  {:<12}  {} <{}>\n",
                recipient.role.as_str(),
                recipient.name,
                recipient.email
            ));
        }
        for signature in &record.signatures {
            let verb = match signature.role {
                SignatureRole::Client => "accepted",
                SignatureRole::Countersign => "countersigned",
            };
            out.push_str(&format!(
                "  {verb} by {} ({}) at {}",
                signature.name,
                signature.method.as_str(),
                signature.signed_at
            ));
            if let Some(typed) = &signature.typed_name {
                out.push_str(&format!(", typed \"{typed}\""));
            }
            out.push_str(&evidence(signature.method, signature.ip.as_deref()));
            out.push('\n');
        }
        for request in &record.change_requests {
            out.push_str(&format!(
                "  changes requested by {} ({}) at {}{}\n",
                request.name,
                request.method.as_str(),
                request.requested_at,
                evidence(request.method, request.ip.as_deref())
            ));
            push_note(&mut out, &request.note);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use nigel_core::documents::model::{
        ChangeRequest, Document, DocumentVersion, Method, Recipient, RecipientRole, Signature,
        SignatureRole, VersionRecord,
    };

    #[test]
    fn the_kind_list_marks_inactive_rows() {
        let kinds = vec![
            DocumentKind {
                id: 1,
                name: "Proposal".into(),
                active: true,
                position: 0,
            },
            DocumentKind {
                id: 4,
                name: "SOW".into(),
                active: false,
                position: 3,
            },
        ];
        let out = format_kind_list(&kinds);
        assert!(out.contains("Proposal") && out.contains("SOW") && out.contains("inactive"));
    }

    fn fixture_record() -> DocumentRecord {
        let version =
            |id: i64, number: i64, checksum: &str, sent_at: Option<&str>| DocumentVersion {
                id,
                document_id: 1,
                number,
                file_path: format!("documents/1/v{number}.pdf"),
                checksum: checksum.into(),
                sent_at: sent_at.map(Into::into),
                created_at: "2026-10-05T09:00:00Z".into(),
            };
        let recipient = |id: i64, role: RecipientRole, name: &str, email: &str| Recipient {
            id,
            version_id: 1,
            role,
            name: name.into(),
            email: email.into(),
            token: "t".into(),
            position: id,
        };
        DocumentRecord {
            document: Document {
                id: 1,
                client_id: 1,
                kind_id: 1,
                kind: "Proposal".into(),
                title: "Website rebuild".into(),
                token: "tok".into(),
                declined_at: None,
                decline_note: None,
                withdrawn_at: None,
                created_at: "2026-10-05T09:00:00Z".into(),
                updated_at: "2026-10-05T17:04:11Z".into(),
                status: DocumentStatus::ChangesRequested,
            },
            client_name: "Cedar Systems".into(),
            versions: vec![
                VersionRecord {
                    version: version(1, 1, "sha256:aa", Some("2026-10-05")),
                    recipients: vec![
                        recipient(1, RecipientRole::Signer, "Pat Example", "pat@cedar.test"),
                        recipient(
                            2,
                            RecipientRole::Collaborator,
                            "Sam Example",
                            "sam@cedar.test",
                        ),
                    ],
                    signatures: vec![],
                    change_requests: vec![ChangeRequest {
                        id: 1,
                        version_id: 1,
                        recipient_id: Some(2),
                        name: "Sam Example".into(),
                        email: Some("sam@cedar.test".into()),
                        method: Method::Online,
                        requested_at: "2026-10-05T17:04:11Z".into(),
                        note: "Fix the dates".into(),
                        ip: Some("203.0.113.7".into()),
                        user_agent: None,
                        checksum: "sha256:cc".into(),
                    }],
                },
                VersionRecord {
                    version: version(2, 2, "sha256:bb", None),
                    recipients: vec![],
                    signatures: vec![],
                    change_requests: vec![],
                },
            ],
        }
    }

    fn signature(role: SignatureRole, method: Method) -> Signature {
        Signature {
            id: 1,
            version_id: 1,
            recipient_id: None,
            role,
            name: "Pat Example".into(),
            email: None,
            method,
            signed_at: "2026-10-06T10:00:00Z".into(),
            typed_name: (method == Method::Online).then(|| "Pat E.".into()),
            ip: (method == Method::Online).then(|| "203.0.113.9".into()),
            user_agent: None,
            checksum: "sha256:dd".into(),
        }
    }

    #[test]
    fn show_prints_every_version_with_its_checksum_recipients_and_responses() {
        let out = format_document_show(&fixture_record());
        assert!(out.contains("Document #1  [changes_requested]  Proposal"));
        assert!(out.contains("Client:   Cedar Systems"));
        assert!(out.contains("Version 1  sha256:aa  sent 2026-10-05"));
        assert!(out.contains("signer        Pat Example <pat@cedar.test>"));
        assert!(out.contains("collaborator  Sam Example <sam@cedar.test>"));
        assert!(out.contains(
            "changes requested by Sam Example (online) at 2026-10-05T17:04:11Z, ip 203.0.113.7"
        ));
        assert!(out.contains("    | Fix the dates"));
        assert!(out.contains("Version 2  sha256:bb  draft"));
    }

    #[test]
    fn a_note_prints_as_its_own_text() {
        let mut record = fixture_record();
        record.versions[0].change_requests[0].note = "<b>x</b>\nsecond line".into();
        let out = format_document_show(&record);
        assert!(out.contains("    | <b>x</b>\n    | second line"));
    }

    #[test]
    fn signatures_print_their_method_and_online_evidence() {
        let mut record = fixture_record();
        record.versions[0].signatures = vec![
            signature(SignatureRole::Client, Method::Online),
            signature(SignatureRole::Countersign, Method::Manual),
        ];
        let out = format_document_show(&record);
        assert!(out.contains(
            "accepted by Pat Example (online) at 2026-10-06T10:00:00Z, typed \"Pat E.\", ip 203.0.113.9"
        ));
        assert!(out.contains("countersigned by Pat Example (manual) at 2026-10-06T10:00:00Z\n"));
    }

    #[test]
    fn a_decline_prints_its_date_and_note() {
        let mut record = fixture_record();
        record.document.declined_at = Some("2026-10-07".into());
        record.document.decline_note = Some("Out of budget".into());
        let out = format_document_show(&record);
        assert!(out.contains("Declined: 2026-10-07\n    | Out of budget"));
    }

    #[test]
    fn the_list_shows_status_kind_client_and_version() {
        let rows = vec![DocumentListRow {
            id: 1,
            title: "Website rebuild".into(),
            kind: "Proposal".into(),
            client_id: 1,
            client_name: Some("Cedar Systems".into()),
            status: DocumentStatus::Sent,
            latest_version: 2,
            sent_at: Some("2026-10-05".into()),
            updated_at: "2026-10-05".into(),
        }];
        let out = format_document_list(&rows);
        for needle in ["Website rebuild", "Proposal", "Cedar Systems", "sent", "v2"] {
            assert!(out.contains(needle), "{needle}");
        }
    }

    #[test]
    fn an_unknown_status_names_the_valid_ones() {
        let err = parse_status("bogus").unwrap_err().to_string();
        assert!(err.contains(
            "Unknown status: bogus. Use one of: draft, sent, changes_requested, accepted, declined, executed, withdrawn."
        ));
    }
}
