//! Fixtures and fakes for document tests in this crate, the CLI crate and the
//! route tests. Compiled only for tests and the `testutil` feature.
pub fn test_conn() -> (tempfile::TempDir, rusqlite::Connection) {
    let dir = tempfile::tempdir().unwrap();
    let conn = crate::db::get_connection(&dir.path().join("t.db")).unwrap();
    crate::db::init_db(&conn).unwrap();
    (dir, conn)
}

pub fn fixture_pdf(seed: &str) -> Vec<u8> {
    format!("%PDF-1.4\n% fixture {seed}\n%%EOF\n").into_bytes()
}

pub fn seed_client(conn: &rusqlite::Connection, name: &str) -> i64 {
    let id = crate::invoicing::clients::add_client(conn, name, None, None, None).unwrap();
    let first = name.split_whitespace().next().unwrap().to_lowercase();
    crate::invoicing::clients::set_contacts(
        conn,
        id,
        &[crate::invoicing::clients::NewContact {
            email: format!("pat@{first}.test"),
            name: Some("Pat Example".into()),
            title: None,
            is_billing: true,
        }],
    )
    .unwrap();
    id
}

pub fn seed_document(
    conn: &rusqlite::Connection,
    data_dir: &std::path::Path,
    client_id: i64,
    title: &str,
) -> i64 {
    crate::documents::store::file_document(
        conn,
        data_dir,
        &crate::documents::store::NewDocument {
            client_id,
            kind: "Proposal",
            title,
        },
        &fixture_pdf(&format!("{client_id}:{title}")),
        "2026-10-01",
    )
    .unwrap()
}

/// Records every object a document publish writes, in order.
#[derive(Default)]
pub struct FakeDocumentPublisher {
    pub objects: std::cell::RefCell<Vec<(String, Vec<u8>)>>,
    pub fail_when_key_contains: Option<String>,
}

impl FakeDocumentPublisher {
    const BASE: &'static str = "https://docs.example.test/d";

    fn write(&self, key: String, bytes: &[u8]) -> crate::error::Result<()> {
        if let Some(needle) = &self.fail_when_key_contains {
            if key.contains(needle.as_str()) {
                return Err(crate::error::NigelError::Other(format!(
                    "fake publisher refused {key}"
                )));
            }
        }
        self.objects.borrow_mut().push((key, bytes.to_vec()));
        Ok(())
    }

    pub fn page(&self, token: &str, rt: &str) -> Option<String> {
        let key = crate::invoicing::r2::document_page_key(token, rt);
        self.objects
            .borrow()
            .iter()
            .rev()
            .find(|(k, _)| *k == key)
            .map(|(_, bytes)| String::from_utf8_lossy(bytes).into_owned())
    }

    pub fn keys(&self) -> Vec<String> {
        self.objects
            .borrow()
            .iter()
            .map(|(k, _)| k.clone())
            .collect()
    }
}

impl crate::invoicing::gateway::DocumentPublisher for FakeDocumentPublisher {
    fn publish_pdf(&self, token: &str, version: i64, pdf: &[u8]) -> crate::error::Result<String> {
        self.write(crate::invoicing::r2::document_pdf_key(token, version), pdf)?;
        Ok(crate::invoicing::r2::document_pdf_url(
            Self::BASE,
            token,
            version,
        ))
    }

    fn publish_page(
        &self,
        token: &str,
        recipient_token: &str,
        html: &[u8],
    ) -> crate::error::Result<String> {
        self.write(
            crate::invoicing::r2::document_page_key(token, recipient_token),
            html,
        )?;
        Ok(crate::invoicing::r2::document_page_url(
            Self::BASE,
            token,
            recipient_token,
        ))
    }

    fn public_base(&self) -> &str {
        Self::BASE
    }
}
