use std::path::Path;

use comfy_table::{Cell, Table};

use nigel_core::db::get_connection;
use nigel_core::documents::guards::{ensure_allowed, ensure_client_active_for_documents, Action};
use nigel_core::documents::kinds::{add_kind, deactivate_kind, list_kinds, rename_kind};
use nigel_core::documents::lifecycle::{
    republish_after_change, revise_with_republish, withdraw_with_teardown,
};
use nigel_core::documents::model::{
    parse_recipient, validate_moment, DocumentKind, DocumentListRow, DocumentRecord,
    DocumentStatus, Method, NewRecipient, RecipientRole, SignatureRole,
};
use nigel_core::documents::record::{
    record_countersign, record_decline, record_manual_accept, record_manual_change_request,
};
use nigel_core::documents::send::{
    default_signer, send_document_traced, write_preview, DocumentSendFailure, SendContext,
};
use nigel_core::documents::store::{
    document_record, file_document, get_document, latest_version, list_documents, DocumentFilter,
    NewDocument,
};
use nigel_core::documents::sync::{sync_documents, DocumentSyncReport};
use nigel_core::error::{NigelError, Result};
use nigel_core::invoicing::clients::{ensure_client_exists, get_client};
use nigel_core::invoicing::gateway::{DocumentPublisher, Mailer, ResponseSource};
use nigel_core::invoicing::wiring::{
    build_document_clients, company_name, optional_document_publisher, optional_response_source,
};
use nigel_core::settings::{documents_config, documents_status, get_data_dir};
use rusqlite::Connection;

use crate::cli::invoice::publish_host;
use crate::cli::{confirm_or_refuse, DocumentKindsCommands};

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

pub(crate) fn resolve_recipients(
    conn: &Connection,
    client_id: i64,
    signer: Option<&str>,
    collaborators: &[String],
) -> Result<Vec<NewRecipient>> {
    let signer = match signer {
        Some(raw) => parse_recipient(raw, RecipientRole::Signer)?,
        None => default_signer(conn, client_id)?,
    };
    let mut recipients = vec![signer];
    for raw in collaborators {
        recipients.push(parse_recipient(raw, RecipientRole::Collaborator)?);
    }
    Ok(recipients)
}

pub fn format_send_summary(
    title: &str,
    client: &str,
    version: i64,
    recipients: &[NewRecipient],
    publish_host: Option<&str>,
    response_form: bool,
) -> String {
    let mut out = format!("Document: {title} ({client}), version {version}\nRecipients:\n");
    for r in recipients {
        out.push_str(&format!(
            "  {} <{}> ({})\n",
            r.name,
            r.email,
            r.role.as_str()
        ));
    }
    let host = match publish_host {
        Some(host) => format!("publishes each recipient's page and the PDF to {host}"),
        None => "publishes each recipient's page and the PDF".to_string(),
    };
    let response = if response_form {
        "Recipients can accept or request changes on their page."
    } else {
        "There is no response form configured, so recipients can only read it."
    };
    out.push_str(&format!(
        "Sending {host} and emails each their own link. {response} This cannot be undone."
    ));
    out
}

pub(crate) fn send_with<P: DocumentPublisher, M: Mailer, R: ResponseSource>(
    conn: &Connection,
    id: i64,
    recipients: &[NewRecipient],
    ctx: &SendContext<'_>,
    publisher: &P,
    mailer: &M,
    source: &R,
) -> Result<String> {
    let outcome = match send_document_traced(conn, id, recipients, ctx, publisher, mailer, source) {
        Ok(outcome) => outcome,
        Err(failure) => {
            eprintln!("{}", format_send_failure(&failure));
            return Err(failure.source);
        }
    };
    let mut out = format!("Sent document #{id} v{}:\n", outcome.version);
    for link in &outcome.links {
        out.push_str(&format!(
            "  {:<12}  {} <{}>  {}\n",
            link.role.as_str(),
            link.name,
            link.email,
            link.url
        ));
    }
    for warning in &outcome.warnings {
        out.push_str(&format!("notice: {warning}\n"));
    }
    Ok(out.trim_end().to_string())
}

pub fn format_send_failure(failure: &DocumentSendFailure) -> String {
    let mut out = format!(
        "Sending the document failed at the {} step; the version is unsent.",
        failure.step.as_str()
    );
    if !failure.emailed.is_empty() {
        out.push_str(&format!(
            "\nAlready emailed: {}. Their links are no longer live; tell them to disregard that email.",
            failure.emailed.join(", ")
        ));
    }
    for warning in &failure.cleanup_warnings {
        out.push('\n');
        out.push_str(warning);
    }
    out
}

pub fn send(
    id: i64,
    signer: Option<&str>,
    collaborators: &[String],
    yes: bool,
    today: &str,
) -> Result<()> {
    let data_dir = get_data_dir();
    let conn = get_connection(&data_dir.join("nigel.db"))?;
    let document = get_document(&conn, id)?;
    ensure_allowed(id, document.status, Action::Send)?;
    ensure_client_active_for_documents(&conn, document.client_id)?;
    let version = latest_version(&conn, id)?;
    let client = get_client(&conn, document.client_id)?;
    let recipients = resolve_recipients(&conn, document.client_id, signer, collaborators)?;

    let config = documents_config();
    if !yes {
        confirm_unless_piped(id)?;
        let host = config
            .invoicing
            .public_base_url
            .as_deref()
            .and_then(publish_host);
        println!(
            "{}",
            format_send_summary(
                &document.title,
                &client.name,
                version.number,
                &recipients,
                host.as_deref(),
                config.document_response_url.is_some(),
            )
        );
        if !confirm_or_refuse("Send it? [y/N]", "", false)? {
            println!("Aborted.");
            return Ok(());
        }
    }

    let status = documents_status(&config);
    if !status.missing.is_empty() {
        return Err(NigelError::Invalid(format!(
            "Sending documents is not configured: missing {} (set each one in settings.json or the matching NIGEL_ env var)",
            status.missing.join(", ")
        )));
    }
    let company = company_name(&conn);
    let clients = build_document_clients(config, &company)?;
    for warning in clients.warnings() {
        eprintln!("notice: {warning}");
    }
    let ctx = SendContext {
        data_dir: &data_dir,
        company: &company,
        response_url: clients.response_url(),
        today,
    };
    let out = send_with(
        &conn,
        id,
        &recipients,
        &ctx,
        clients.publisher(),
        clients.mail(),
        clients.source(),
    )?;
    println!("{out}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn revise_with<P: DocumentPublisher, R: ResponseSource>(
    conn: &Connection,
    data_dir: &Path,
    id: i64,
    pdf: &[u8],
    today: &str,
    company: &str,
    publisher: Option<&P>,
    source: Option<&R>,
) -> Result<(String, Vec<String>)> {
    let outcome =
        revise_with_republish(conn, data_dir, id, pdf, today, company, publisher, source)?;
    Ok((
        format!(
            "Revised document #{id}: version {} is a draft.",
            outcome.version
        ),
        outcome.warnings,
    ))
}

pub(crate) fn withdraw_with<P: DocumentPublisher, R: ResponseSource>(
    conn: &Connection,
    id: i64,
    today: &str,
    company: &str,
    publisher: Option<&P>,
    source: Option<&R>,
) -> Result<(String, Vec<String>)> {
    let warnings = withdraw_with_teardown(conn, id, today, company, publisher, source)?;
    Ok((format!("Withdrew document #{id}."), warnings))
}

pub fn revise(id: i64, file: &Path, today: &str) -> Result<()> {
    let pdf = std::fs::read(file)
        .map_err(|e| NigelError::Invalid(format!("Could not read {}: {e}", file.display())))?;
    let data_dir = get_data_dir();
    let conn = get_connection(&data_dir.join("nigel.db"))?;
    let config = documents_config();
    let publisher = optional_document_publisher(&config);
    let source = optional_response_source(&config);
    let (message, warnings) = revise_with(
        &conn,
        &data_dir,
        id,
        &pdf,
        today,
        &company_name(&conn),
        publisher.as_ref(),
        source.as_ref(),
    )?;
    println!("{message}");
    for warning in warnings {
        eprintln!("{warning}");
    }
    Ok(())
}

pub fn withdraw(id: i64, yes: bool, today: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let document = get_document(&conn, id)?;
    ensure_allowed(id, document.status, Action::Withdraw)?;
    let question = format!(
        "Withdraw document #{id} ({})? Its pages are replaced with a withdrawn notice and this cannot be undone. [y/N]",
        document.title
    );
    let refusal = format!("Refusing to withdraw document #{id} without confirmation. Pass --yes.");
    if !confirm_or_refuse(&question, &refusal, yes)? {
        println!("Aborted.");
        return Ok(());
    }
    let config = documents_config();
    let publisher = optional_document_publisher(&config);
    let source = optional_response_source(&config);
    let (message, warnings) = withdraw_with(
        &conn,
        id,
        today,
        &company_name(&conn),
        publisher.as_ref(),
        source.as_ref(),
    )?;
    println!("{message}");
    for warning in warnings {
        eprintln!("{warning}");
    }
    Ok(())
}

fn record_then_republish(
    id: i64,
    date: &str,
    record: impl FnOnce(&Connection, &str) -> Result<()>,
) -> Result<()> {
    let date = validate_moment(date, "date")?;
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    record(&conn, &date)?;
    println!(
        "Recorded: document #{id} is {}.",
        get_document(&conn, id)?.status.as_str()
    );
    let config = documents_config();
    let publisher = optional_document_publisher(&config);
    let source = optional_response_source(&config);
    for warning in republish_after_change(
        &conn,
        id,
        &company_name(&conn),
        publisher.as_ref(),
        source.as_ref(),
    ) {
        eprintln!("{warning}");
    }
    Ok(())
}

/// Replaces every control character but newline, so text a recipient typed
/// cannot move the cursor or rewrite the terminal.
pub fn printable(text: &str) -> String {
    text.chars()
        .map(|c| if c != '\n' && c.is_control() { ' ' } else { c })
        .collect()
}

pub fn format_sync_report(report: &DocumentSyncReport) -> String {
    let mut out = String::new();
    for line in &report.lines {
        let title = printable(&line.title);
        if line.recorded.is_empty() {
            out.push_str(&format!("#{} {title}\n", line.document_id));
        }
        for recorded in &line.recorded {
            out.push_str(&format!(
                "#{} {title}: {} → {}\n",
                line.document_id,
                printable(recorded),
                line.status.as_str()
            ));
        }
        for refused in &line.refused {
            out.push_str(&format!("  refused: {}\n", printable(refused)));
        }
        for warning in &line.warnings {
            out.push_str(&format!("  warning: {}\n", printable(warning)));
        }
    }
    for failure in &report.failures {
        out.push_str(&format!(
            "notice: document sync failed for #{}: {}\n",
            failure.document_id,
            printable(&failure.message)
        ));
    }
    out.push_str(&format!("Recorded {} new response(s)", report.recorded));
    out
}

pub(crate) fn sync_with<R: ResponseSource, P: DocumentPublisher>(
    conn: &Connection,
    company: &str,
    source: &R,
    publisher: Option<&P>,
) -> Result<String> {
    let report = sync_documents(conn, company, source, publisher, None)?;
    Ok(format_sync_report(&report))
}

pub fn sync() -> Result<()> {
    let config = documents_config();
    if !documents_status(&config).sync_configured {
        return Err(NigelError::Invalid(
            "Document sync is not configured: set r2_account_id, r2_access_key, r2_secret_key and r2_private_bucket (in settings.json or the matching NIGEL_ env var)"
                .into(),
        ));
    }
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let publisher = optional_document_publisher(&config);
    let Some(source) = optional_response_source(&config) else {
        return Err(NigelError::Invalid(
            "Document sync is not configured".into(),
        ));
    };
    println!(
        "{}",
        sync_with(&conn, &company_name(&conn), &source, publisher.as_ref())?
    );
    Ok(())
}

pub fn accept(id: i64, name: &str, date: &str) -> Result<()> {
    record_then_republish(id, date, |conn, at| {
        record_manual_accept(conn, id, name, at)
    })
}

pub fn request_changes(id: i64, name: &str, note: &str, date: &str) -> Result<()> {
    record_then_republish(id, date, |conn, at| {
        record_manual_change_request(conn, id, name, note, at)
    })
}

pub fn decline(id: i64, note: Option<&str>, date: &str) -> Result<()> {
    record_then_republish(id, date, |conn, at| record_decline(conn, id, note, at))
}

pub fn countersign(id: i64, name: &str, date: &str) -> Result<()> {
    record_then_republish(id, date, |conn, at| record_countersign(conn, id, name, at))
}

fn confirm_unless_piped(id: i64) -> Result<()> {
    use std::io::IsTerminal;
    if std::io::stdin().is_terminal() {
        return Ok(());
    }
    Err(NigelError::Other(format!(
        "Refusing to send document #{id} without confirmation. Pass --yes."
    )))
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
        ChangeRequest, Document, DocumentVersion, Method, Recipient, Signature, VersionRecord,
    };
    use nigel_core::documents::send::DocumentSendStep;
    use nigel_core::documents::sync::{DocumentSyncFailure, DocumentSyncLine};
    use nigel_core::documents::testing::{
        fixture_pdf, pat, sam, seed_client, seed_document, sent_document_with_fakes, test_conn,
        FakeDocumentPublisher, FakeMailer, FakeResponseSource,
    };

    #[test]
    fn the_sync_report_prints_one_line_per_document() {
        let report = DocumentSyncReport {
            documents_checked: 2,
            recorded: 1,
            lines: vec![DocumentSyncLine {
                document_id: 3,
                title: "Website rebuild".into(),
                recorded: vec!["Pat Example accepted version 2".into()],
                refused: vec!["a response for version 1 does not match version 2".into()],
                warnings: vec![],
                status: DocumentStatus::Accepted,
            }],
            failures: vec![DocumentSyncFailure {
                document_id: 4,
                message: "r2 403: denied".into(),
            }],
        };
        let out = format_sync_report(&report);
        assert!(out.contains("#3 Website rebuild: Pat Example accepted version 2 → accepted"));
        assert!(out.contains("  refused: a response for version 1"));
        assert!(out.contains("Recorded 1 new response(s)"));
    }

    #[test]
    fn the_sync_report_strips_control_characters_from_recipient_text() {
        let report = DocumentSyncReport {
            documents_checked: 1,
            recorded: 1,
            lines: vec![DocumentSyncLine {
                document_id: 3,
                title: "Website rebuild".into(),
                recorded: vec!["Pat\x1b[2J Example accepted version 2".into()],
                refused: vec!["bad\x07 name".into()],
                warnings: vec!["warn\x1b[0m".into()],
                status: DocumentStatus::Accepted,
            }],
            failures: vec![],
        };
        let out = format_sync_report(&report);
        assert!(!out.contains('\x1b') && !out.contains('\x07'), "{out:?}");
        assert!(out.contains("Pat [2J Example"));
        assert_eq!(printable("a\nb\tc\x00"), "a\nb c ");
    }

    #[test]
    fn sync_with_records_an_online_accept() {
        let (dir, conn) = test_conn();
        let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
        let doc = get_document(&conn, id).unwrap();
        let v = latest_version(&conn, id).unwrap();
        let signer = nigel_core::documents::store::recipients(&conn, v.id)
            .unwrap()
            .remove(0);
        src.put_response(
            &doc.token,
            1,
            &signer.token,
            nigel_core::documents::wire::DocumentResponse {
                action: nigel_core::documents::wire::ResponseAction::Accept,
                version: 1,
                checksum: v.checksum.clone(),
                recipient_token: signer.token.clone(),
                typed_name: Some("Pat Example".into()),
                consent: Some(true),
                note: None,
                received_at: "2026-10-05T17:04:11Z".into(),
                ip: None,
                user_agent: None,
            },
        );
        assert!(sync_with(&conn, "Initech", &src, Some(&p))
            .unwrap()
            .contains("→ accepted"));
    }

    #[test]
    fn send_with_prints_each_recipients_link() {
        let _config = nigel_core::settings::TempConfigDir::new();
        let (dir, conn) = test_conn();
        let id = seed_document(
            &conn,
            dir.path(),
            seed_client(&conn, "Cedar Systems"),
            "Website rebuild",
        );
        let ctx = SendContext {
            data_dir: dir.path(),
            company: "Initech",
            response_url: None,
            today: "2026-10-05",
        };
        let (p, m, s) = (
            FakeDocumentPublisher::default(),
            FakeMailer::default(),
            FakeResponseSource::default(),
        );
        let out = send_with(&conn, id, &[pat(), sam()], &ctx, &p, &m, &s).unwrap();
        assert!(out.starts_with("Sent document #1 v1:"));
        assert_eq!(out.matches("/index.html").count(), 2);
        assert!(out.contains("signer") && out.contains("collaborator"));
    }

    #[test]
    fn revise_with_and_withdraw_with_report_and_surface_warnings() {
        let (dir, conn) = test_conn();
        let (id, p, s) = sent_document_with_fakes(&conn, dir.path());
        let (msg, warnings) = revise_with(
            &conn,
            dir.path(),
            id,
            &fixture_pdf("v2"),
            "2026-10-06",
            "Initech",
            Some(&p),
            Some(&s),
        )
        .unwrap();
        assert_eq!(
            msg,
            format!("Revised document #{id}: version 2 is a draft.")
        );
        assert!(warnings.is_empty());
        let (msg, warnings) = withdraw_with::<FakeDocumentPublisher, FakeResponseSource>(
            &conn,
            id,
            "2026-10-07",
            "Initech",
            None,
            None,
        )
        .unwrap();
        assert_eq!(msg, format!("Withdrew document #{id}."));
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn recipients_default_to_the_billing_contact_and_take_collaborators() {
        let (_dir, conn) = test_conn();
        let client = seed_client(&conn, "Cedar Systems");
        assert_eq!(
            resolve_recipients(
                &conn,
                client,
                None,
                &["Sam Example <sam@cedar.test>".into()]
            )
            .unwrap(),
            vec![pat(), sam()]
        );
        let lee =
            resolve_recipients(&conn, client, Some("Lee Example <lee@cedar.test>"), &[]).unwrap();
        assert_eq!(
            (lee.len(), lee[0].name.as_str(), lee[0].role),
            (1, "Lee Example", RecipientRole::Signer)
        );
    }

    #[test]
    fn the_summary_says_what_sending_will_do() {
        let out = format_send_summary(
            "Website rebuild",
            "Cedar Systems",
            1,
            &[pat(), sam()],
            Some("docs.example.test"),
            false,
        );
        assert!(out.contains("Pat Example <pat@cedar.test> (signer)"));
        assert!(out.contains("docs.example.test"));
        assert!(out.contains("no response form"));
    }

    #[test]
    fn a_failure_after_emailing_names_who_was_emailed_and_that_their_links_are_dead() {
        let failure = DocumentSendFailure {
            step: DocumentSendStep::Record,
            completed: vec![],
            emailed: vec!["pat@cedar.test".into(), "sam@cedar.test".into()],
            document_status: None,
            cleanup_warnings: vec!["Warning: manifest not closed.".into()],
            source: NigelError::Other("disk full".into()),
        };
        let out = format_send_failure(&failure);
        assert!(out.contains("record step"));
        assert!(out.contains("pat@cedar.test, sam@cedar.test"));
        assert!(out.contains("no longer live"));
        assert!(out.contains("Warning: manifest not closed."));
    }

    #[test]
    fn a_failure_before_any_email_does_not_mention_emails() {
        let failure = DocumentSendFailure {
            step: DocumentSendStep::Publish,
            completed: vec![],
            emailed: vec![],
            document_status: None,
            cleanup_warnings: vec![],
            source: NigelError::Other("boom".into()),
        };
        assert!(!format_send_failure(&failure).contains("emailed"));
    }

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
