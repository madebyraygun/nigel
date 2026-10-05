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
