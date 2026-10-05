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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedMail {
    pub to: String,
    pub subject: String,
    pub text: String,
    pub attachment: Option<(String, Vec<u8>)>,
}

/// Records every message instead of sending it. `fail_on_call` makes the
/// call at that 0-based index fail, after the earlier ones were recorded.
#[derive(Default)]
pub struct FakeMailer {
    pub sent: std::cell::RefCell<Vec<CapturedMail>>,
    pub fail_on_call: Option<usize>,
}

impl crate::invoicing::gateway::Mailer for FakeMailer {
    fn send(&self, mail: &crate::invoicing::gateway::OutgoingMail<'_>) -> crate::error::Result<()> {
        if self.fail_on_call == Some(self.sent.borrow().len()) {
            return Err(crate::error::NigelError::Other("mailgun 500: boom".into()));
        }
        self.sent.borrow_mut().push(CapturedMail {
            to: mail.to.to_string(),
            subject: mail.subject.to_string(),
            text: mail.text.to_string(),
            attachment: mail
                .attachment
                .map(|a| (a.filename.to_string(), a.bytes.to_vec())),
        });
        Ok(())
    }
}

/// An in-memory private store. `fail_fetch_for` fails fetches for one document
/// token only.
#[derive(Default)]
pub struct FakeResponseSource {
    pub responses: std::cell::RefCell<
        std::collections::HashMap<String, crate::documents::wire::DocumentResponse>,
    >,
    pub raw_responses: std::cell::RefCell<std::collections::HashMap<String, Vec<u8>>>,
    pub manifests: std::cell::RefCell<Vec<(String, crate::documents::wire::Manifest)>>,
    pub fail_fetch: bool,
    pub fail_fetch_for: Option<String>,
    pub fail_put: bool,
}

impl FakeResponseSource {
    pub fn put_response(
        &self,
        token: &str,
        version: i64,
        rt: &str,
        r: crate::documents::wire::DocumentResponse,
    ) {
        self.responses
            .borrow_mut()
            .insert(crate::documents::wire::response_key(token, version, rt), r);
    }

    /// Store an object body as is, whether or not it is a response.
    pub fn put_raw_response(&self, token: &str, version: i64, rt: &str, body: &[u8]) {
        self.raw_responses.borrow_mut().insert(
            crate::documents::wire::response_key(token, version, rt),
            body.to_vec(),
        );
    }

    pub fn last_manifest(&self, token: &str) -> Option<crate::documents::wire::Manifest> {
        self.manifests
            .borrow()
            .iter()
            .rev()
            .find(|(t, _)| t == token)
            .map(|(_, m)| m.clone())
    }
}

impl crate::invoicing::gateway::ResponseSource for FakeResponseSource {
    fn fetch(
        &self,
        token: &str,
        version: i64,
        recipient_token: &str,
    ) -> crate::error::Result<Option<Vec<u8>>> {
        if self.fail_fetch || self.fail_fetch_for.as_deref() == Some(token) {
            return Err(crate::error::NigelError::Other(
                "r2 500: fake fetch refused".into(),
            ));
        }
        let key = crate::documents::wire::response_key(token, version, recipient_token);
        if let Some(body) = self.raw_responses.borrow().get(&key) {
            return Ok(Some(body.clone()));
        }
        Ok(self
            .responses
            .borrow()
            .get(&key)
            .map(|r| serde_json::to_vec(r).expect("a response serializes")))
    }

    fn put_manifest(
        &self,
        token: &str,
        manifest: &crate::documents::wire::Manifest,
    ) -> crate::error::Result<()> {
        if self.fail_put {
            return Err(crate::error::NigelError::Other(
                "r2 500: fake put refused".into(),
            ));
        }
        self.manifests
            .borrow_mut()
            .push((token.to_string(), manifest.clone()));
        Ok(())
    }
}

pub fn pat() -> crate::documents::model::NewRecipient {
    crate::documents::model::NewRecipient {
        role: crate::documents::model::RecipientRole::Signer,
        name: "Pat Example".into(),
        email: "pat@cedar.test".into(),
    }
}

pub fn sam() -> crate::documents::model::NewRecipient {
    crate::documents::model::NewRecipient {
        role: crate::documents::model::RecipientRole::Collaborator,
        name: "Sam Example".into(),
        email: "sam@cedar.test".into(),
    }
}

const FIXTURE_CAST: [&str; 6] = [
    "Cedar Systems",
    "Juniper Labs",
    "Harbor & Vale",
    "Acme",
    "Globex",
    "Initech",
];

/// Files "Website rebuild" for the first fixture-cast client not yet in
/// `clients` and sends it to `pat()` and `sam()` through the fakes.
pub fn sent_document_with_fakes(
    conn: &rusqlite::Connection,
    dir: &std::path::Path,
) -> (i64, FakeDocumentPublisher, FakeResponseSource) {
    let taken: Vec<String> = conn
        .prepare("SELECT name FROM clients")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let name = FIXTURE_CAST
        .iter()
        .find(|n| !taken.iter().any(|t| t == **n))
        .expect("every fixture-cast client is taken");
    let id = seed_document(conn, dir, seed_client(conn, name), "Website rebuild");
    let (publisher, source) = (
        FakeDocumentPublisher::default(),
        FakeResponseSource::default(),
    );
    let ctx = crate::documents::send::SendContext {
        data_dir: dir,
        company: "Initech",
        response_url: Some("https://docs.example.test/d/respond"),
        today: "2026-10-05",
    };
    crate::documents::send::send_document(
        conn,
        id,
        &[pat(), sam()],
        &ctx,
        &publisher,
        &FakeMailer::default(),
        &source,
    )
    .unwrap();
    (id, publisher, source)
}
