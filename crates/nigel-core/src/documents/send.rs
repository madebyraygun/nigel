//! Preview and send share one render path: what `nigel document preview` writes
//! is built by the same functions that build what a client receives.
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::model::RecipientRole;
use super::render::{
    relative_pdf_href, render_document_pages, PageContext, PageRecipient, PageState,
};
use super::store::{get_document, latest_version, read_version_pdf};
use crate::error::Result;
use crate::invoicing::clients::{get_client, list_contacts};

pub struct PreviewFiles {
    pub dir: PathBuf,
    pub pages: Vec<PathBuf>,
    pub pdf: PathBuf,
}

/// The pages a preview stands in for: the client's billing contact as the
/// signer, and a placeholder collaborator so both page shapes are visible.
pub fn preview_recipients(
    conn: &Connection,
    client_id: i64,
) -> Vec<(String, RecipientRole, String)> {
    let signer = list_contacts(conn, client_id)
        .ok()
        .and_then(|contacts| contacts.into_iter().find(|c| c.is_billing))
        .and_then(|c| c.name)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Signer".to_string());
    vec![
        ("signer".into(), RecipientRole::Signer, signer),
        (
            "collaborator".into(),
            RecipientRole::Collaborator,
            "Collaborator".into(),
        ),
    ]
}

pub fn write_preview(
    conn: &Connection,
    data_dir: &Path,
    id: i64,
    company: &str,
    response_url: Option<&str>,
    out_dir: &Path,
) -> Result<PreviewFiles> {
    let document = get_document(conn, id)?;
    let version = latest_version(conn, id)?;
    let pdf_bytes = read_version_pdf(data_dir, &version)?;
    let client = get_client(conn, document.client_id)?;
    let href = relative_pdf_href(version.number);
    let ctx = PageContext {
        company,
        client_name: &client.name,
        kind: &document.kind,
        title: &document.title,
        token: &document.token,
        version: version.number,
        checksum: &version.checksum,
        pdf_href: &href,
    };
    let stand_ins = preview_recipients(conn, document.client_id);
    let recipients: Vec<PageRecipient<'_>> = stand_ins
        .iter()
        .map(|(label, role, name)| PageRecipient {
            token: label,
            role: *role,
            name,
        })
        .collect();
    let pages = render_document_pages(&ctx, &recipients, &PageState::Open { response_url });

    let dir = out_dir.join(format!("document-{id}"));
    let mut written = Vec::new();
    for (label, html) in pages {
        let path = dir.join(&label).join("index.html");
        write_file(&path, html.as_bytes())?;
        written.push(path);
    }
    let pdf = dir
        .join(format!("v{}", version.number))
        .join("document.pdf");
    write_file(&pdf, &pdf_bytes)?;
    Ok(PreviewFiles {
        dir,
        pages: written,
        pdf,
    })
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::testing::{seed_client, seed_document, test_conn};

    #[test]
    fn preview_writes_pages_and_the_pdf_with_no_network_and_no_configuration() {
        let (dir, conn) = test_conn();
        let id = seed_document(
            &conn,
            dir.path(),
            seed_client(&conn, "Cedar Systems"),
            "Website rebuild",
        );
        let out = tempfile::tempdir().unwrap();
        let files = write_preview(&conn, dir.path(), id, "", None, out.path()).unwrap();
        assert_eq!(files.pages.len(), 2);
        let signer = std::fs::read_to_string(&files.pages[0]).unwrap();
        assert!(signer.contains("../v1/document.pdf") && !signer.contains("<form"));
        assert!(files.pdf.ends_with("document-1/v1/document.pdf"));
        assert!(files.pdf.exists());
    }

    #[test]
    fn the_signer_is_the_billing_contact() {
        let (_dir, conn) = test_conn();
        let client = seed_client(&conn, "Cedar Systems");
        let recipients = preview_recipients(&conn, client);
        assert_eq!(recipients[0].2, "Pat Example");
        assert_eq!(recipients[1].0, "collaborator");
    }
}
