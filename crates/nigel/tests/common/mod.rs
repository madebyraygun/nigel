//! Shared end-to-end harness: an isolated HOME with no credentials in reach.

#![allow(dead_code)]

use std::path::PathBuf;

use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// Every `NIGEL_*` key `settings::invoicing_config()` reads. Env vars win over the
/// settings file, and the temp HOME cannot mask them, so they are cleared per command.
pub const INVOICING_ENV_VARS: [&str; 12] = [
    "NIGEL_STRIPE_SECRET_KEY",
    "NIGEL_MAILGUN_API_KEY",
    "NIGEL_MAILGUN_DOMAIN",
    "NIGEL_FROM_EMAIL",
    "NIGEL_FROM_NAME",
    "NIGEL_REPLY_TO_EMAIL",
    "NIGEL_CONTACT_EMAIL",
    "NIGEL_R2_ACCOUNT_ID",
    "NIGEL_R2_ACCESS_KEY",
    "NIGEL_R2_SECRET_KEY",
    "NIGEL_R2_BUCKET",
    "NIGEL_PUBLIC_BASE_URL",
];

/// Bounds any run that could reach the interactive password prompt, so a test
/// inheriting a tty fails instead of blocking on `rpassword` forever.
pub const TEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Create an isolated environment: a temp HOME so that `~/.config/nigel/settings.json`
/// and `~/Documents/nigel/` all live inside the temp dir. Returns the TempDir (must be
/// kept alive for the duration of the test) and a helper to build `nigel` commands that
/// inherit the overridden HOME.
pub struct TestEnv {
    pub home: TempDir,
}

impl TestEnv {
    pub fn new() -> Self {
        Self {
            home: TempDir::new().expect("failed to create temp home"),
        }
    }

    /// Data directory inside the fake HOME.
    pub fn data_dir(&self) -> PathBuf {
        self.home.path().join("nigel-data")
    }

    /// Build a `nigel` Command with HOME pointed at our temp dir and every
    /// invoicing credential cleared from the inherited environment, so no test
    /// can reach Stripe, R2, or Mailgun on a machine where those are exported.
    pub fn cmd(&self) -> Command {
        let mut cmd: Command = cargo_bin_cmd!("nigel");
        cmd.env("HOME", self.home.path());
        for var in INVOICING_ENV_VARS {
            cmd.env_remove(var);
        }
        cmd
    }

    pub fn db(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(self.data_dir().join("nigel.db")).expect("failed to open DB")
    }

    /// Rewind the database to the state of a pre-v3 install: schema version 2 and
    /// no `form_line` on the categories that migration v3 backfills.
    pub fn downgrade_to_v2(&self) {
        self.db()
            .execute_batch(
                "UPDATE metadata SET value = '2' WHERE key = 'schema_version';
                 UPDATE categories SET form_line = NULL
                     WHERE name IN ('Client Services', 'Hosting & Maintenance', 'Reimbursements',
                                    'Other Income', 'Cost of Goods Sold', 'Transfer');",
            )
            .expect("failed to downgrade test database");
    }

    /// Rewind to a pre-v10 install: the class columns gone, the seeded
    /// `Owner Contribution` category gone with them, and the version set back.
    /// What migration v10 will find on a real upgrading installation.
    pub fn downgrade_to_v9(&self) {
        self.db()
            .execute_batch(
                "ALTER TABLE accounts DROP COLUMN class;
                 ALTER TABLE categories DROP COLUMN class;
                 DELETE FROM categories WHERE name = 'Owner Contribution';
                 UPDATE metadata SET value = '9' WHERE key = 'schema_version';",
            )
            .expect("failed to downgrade test database to v9");
    }

    pub fn schema_version(&self) -> u32 {
        self.db()
            .query_row(
                "SELECT value FROM metadata WHERE key = 'schema_version'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("schema_version missing")
            .parse()
            .expect("schema_version not a number")
    }

    /// Encrypt the database in place, the way `nigel password set` does.
    pub fn encrypt(&self, password: &str) {
        let db = self.data_dir().join("nigel.db");
        let tmp = self.data_dir().join("nigel.db.encrypting");
        let conn = self.db();
        conn.execute(
            "ATTACH DATABASE ?1 AS encrypted KEY ?2",
            rusqlite::params![tmp.to_string_lossy(), password],
        )
        .expect("failed to attach encrypted database");
        conn.execute_batch("SELECT sqlcipher_export('encrypted'); DETACH DATABASE encrypted;")
            .expect("failed to export to encrypted database");
        drop(conn);
        let _ = std::fs::remove_file(self.data_dir().join("nigel.db-wal"));
        let _ = std::fs::remove_file(self.data_dir().join("nigel.db-shm"));
        std::fs::rename(&tmp, &db).expect("failed to swap in encrypted database");

        assert!(
            self.db()
                .execute_batch("SELECT count(*) FROM sqlite_master;")
                .is_err(),
            "fixture did not actually encrypt the database"
        );
    }

    pub fn form_line(&self, category: &str) -> Option<String> {
        self.db()
            .query_row(
                "SELECT form_line FROM categories WHERE name = ?1",
                [category],
                |row| row.get(0),
            )
            .expect("category missing")
    }

    /// Run `nigel init --data-dir <data_dir>` then `nigel demo`.
    pub fn init_and_demo(&self) {
        self.cmd()
            .args(["init", "--data-dir", &self.data_dir().to_string_lossy()])
            .assert()
            .success()
            .stdout(predicate::str::contains("Initialized"));

        self.cmd()
            .arg("demo")
            .assert()
            .success()
            .stdout(predicate::str::contains("Demo data loaded"));
    }
}
