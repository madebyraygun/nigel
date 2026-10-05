//! Fixtures and fakes for document tests in this crate, the CLI crate and the
//! route tests. Compiled only for tests and the `testutil` feature.
pub fn test_conn() -> (tempfile::TempDir, rusqlite::Connection) {
    let dir = tempfile::tempdir().unwrap();
    let conn = crate::db::get_connection(&dir.path().join("t.db")).unwrap();
    crate::db::init_db(&conn).unwrap();
    (dir, conn)
}
