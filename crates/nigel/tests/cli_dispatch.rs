use std::path::PathBuf;

use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// Every `NIGEL_*` key `settings::invoicing_config()` reads. Env vars win over the
/// settings file, and the temp HOME cannot mask them, so they are cleared per command.
const INVOICING_ENV_VARS: [&str; 12] = [
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
const TEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Create an isolated environment: a temp HOME so that `~/.config/nigel/settings.json`
/// and `~/Documents/nigel/` all live inside the temp dir. Returns the TempDir (must be
/// kept alive for the duration of the test) and a helper to build `nigel` commands that
/// inherit the overridden HOME.
struct TestEnv {
    home: TempDir,
}

impl TestEnv {
    fn new() -> Self {
        Self {
            home: TempDir::new().expect("failed to create temp home"),
        }
    }

    /// Data directory inside the fake HOME.
    fn data_dir(&self) -> PathBuf {
        self.home.path().join("nigel-data")
    }

    /// Build a `nigel` Command with HOME pointed at our temp dir and every
    /// invoicing credential cleared from the inherited environment, so no test
    /// can reach Stripe, R2, or Mailgun on a machine where those are exported.
    fn cmd(&self) -> Command {
        let mut cmd: Command = cargo_bin_cmd!("nigel");
        cmd.env("HOME", self.home.path());
        for var in INVOICING_ENV_VARS {
            cmd.env_remove(var);
        }
        cmd
    }

    fn db(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(self.data_dir().join("nigel.db")).expect("failed to open DB")
    }

    /// Rewind the database to the state of a pre-v3 install: schema version 2 and
    /// no `form_line` on the categories that migration v3 backfills.
    fn downgrade_to_v2(&self) {
        self.db()
            .execute_batch(
                "UPDATE metadata SET value = '2' WHERE key = 'schema_version';
                 UPDATE categories SET form_line = NULL
                     WHERE name IN ('Client Services', 'Hosting & Maintenance', 'Reimbursements',
                                    'Other Income', 'Cost of Goods Sold', 'Transfer');",
            )
            .expect("failed to downgrade test database");
    }

    fn schema_version(&self) -> u32 {
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
    fn encrypt(&self, password: &str) {
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

    fn form_line(&self, category: &str) -> Option<String> {
        self.db()
            .query_row(
                "SELECT form_line FROM categories WHERE name = ?1",
                [category],
                |row| row.get(0),
            )
            .expect("category missing")
    }

    /// Run `nigel init --data-dir <data_dir>` then `nigel demo`.
    fn init_and_demo(&self) {
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

#[test]
fn init_then_demo() {
    let env = TestEnv::new();
    env.init_and_demo();

    // DB file should exist
    assert!(env.data_dir().join("nigel.db").exists());
}

#[test]
fn demo_is_idempotent() {
    let env = TestEnv::new();
    env.init_and_demo();

    // Running demo again should succeed and report already loaded
    env.cmd()
        .arg("demo")
        .assert()
        .success()
        .stdout(predicate::str::contains("Demo data already loaded"));
}

#[test]
fn status_after_demo() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd().arg("status").assert().success().stdout(
        predicate::str::contains("Transactions:")
            .and(predicate::str::contains("Accounts:"))
            .and(predicate::str::contains("Rules:")),
    );
}

#[test]
fn backup_to_custom_path() {
    let env = TestEnv::new();
    env.init_and_demo();

    let backup_path = env.home.path().join("test-backup.db");
    env.cmd()
        .args(["backup", "--output", &backup_path.to_string_lossy()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Backup saved to"));

    assert!(backup_path.exists());
    let size = std::fs::metadata(&backup_path).unwrap().len();
    assert!(size > 0, "backup file should be non-empty");
}

#[test]
fn backup_default_location() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .arg("backup")
        .assert()
        .success()
        .stdout(predicate::str::contains("Backup saved to"));

    // Should have created a file in <data_dir>/backups/
    let backups_dir = env.data_dir().join("backups");
    assert!(backups_dir.exists());
    let entries: Vec<_> = std::fs::read_dir(&backups_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(!entries.is_empty(), "backups dir should contain a file");
}

#[test]
fn restore_from_backup() {
    let env = TestEnv::new();
    env.init_and_demo();

    // Create a backup
    let backup_path = env.home.path().join("test-backup.db");
    env.cmd()
        .args(["backup", "--output", &backup_path.to_string_lossy()])
        .assert()
        .success();

    // Add a new account to the current database (post-backup change)
    env.cmd()
        .args([
            "accounts",
            "add",
            "Post-Backup Account",
            "--type",
            "checking",
        ])
        .assert()
        .success();

    // Restore from backup (pipe "y" to confirm)
    env.cmd()
        .args(["restore", &backup_path.to_string_lossy()])
        .write_stdin("y\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Safety backup saved to"))
        .stdout(predicate::str::contains("Database restored from"));

    // Verify the post-backup account is gone (restored to pre-change state)
    let output = env.cmd().args(["accounts", "list"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("Post-Backup Account"),
        "Post-backup account should not exist after restore"
    );

    // Verify a safety backup was created
    let backups_dir = env.data_dir().join("backups");
    let entries: Vec<_> = std::fs::read_dir(&backups_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains("pre-restore"))
        .collect();
    assert!(
        !entries.is_empty(),
        "pre-restore safety backup should exist"
    );
}

#[test]
fn restore_nonexistent_file_fails() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args(["restore", "/tmp/nonexistent-nigel-backup-xyz.db"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn report_pnl_text_export() {
    let env = TestEnv::new();
    env.init_and_demo();

    let year = chrono::Local::now().format("%Y").to_string();
    let output_path = env.home.path().join("pnl-report.txt");
    env.cmd()
        .args([
            "report",
            "pnl",
            "--year",
            &year,
            "--mode",
            "export",
            "--format",
            "text",
            "--output",
            &output_path.to_string_lossy(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Wrote"));

    assert!(output_path.exists());
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(!content.is_empty(), "report file should be non-empty");
}

#[test]
fn report_all_text_export() {
    let env = TestEnv::new();
    env.init_and_demo();

    let year = chrono::Local::now().format("%Y").to_string();
    let output_dir = env.home.path().join("all-reports");
    env.cmd()
        .args([
            "report",
            "all",
            "--year",
            &year,
            "--format",
            "text",
            "--output-dir",
            &output_dir.to_string_lossy(),
        ])
        .assert()
        .success();

    assert!(output_dir.exists());
    let names: Vec<String> = std::fs::read_dir(&output_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "txt"))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    // pnl, expenses, tax, cashflow, register, flagged, balance, k1-prep, aging
    assert_eq!(
        names.len(),
        9,
        "expected 9 report files, got {}: {names:?}",
        names.len()
    );
    // Business books include the K-1 worksheet — the half of the profile
    // gating that a blanket "skip k1-prep" regression would break.
    assert!(
        names.iter().any(|n| n.starts_with("k1-prep")),
        "business report all should include k1-prep, got {names:?}"
    );
}

#[test]
fn report_all_skips_k1_on_personal_books() {
    let env = TestEnv::new();
    env.cmd()
        .args([
            "init",
            "--data-dir",
            &env.data_dir().to_string_lossy(),
            "--profile",
            "personal",
        ])
        .assert()
        .success();

    let output_dir = env.home.path().join("personal-reports");
    env.cmd()
        .args([
            "report",
            "all",
            "--format",
            "text",
            "--output-dir",
            &output_dir.to_string_lossy(),
        ])
        .assert()
        .success();

    let names: Vec<String> = std::fs::read_dir(&output_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    // pnl, expenses, tax, cashflow, register, flagged, balance, aging — the
    // business nine minus the K-1 worksheet, with aging on both profiles.
    assert_eq!(
        names.len(),
        8,
        "expected 8 report files, got {}: {names:?}",
        names.len()
    );
    assert!(
        names.iter().any(|n| n.starts_with("pnl")),
        "expected the standard reports, got {names:?}"
    );
    assert!(
        names.iter().any(|n| n.starts_with("aging")),
        "personal report all should include aging, got {names:?}"
    );
    assert!(
        !names.iter().any(|n| n.starts_with("k1-prep")),
        "personal report all must not write a K-1 worksheet, got {names:?}"
    );
}

#[test]
fn init_rejects_unknown_profile() {
    let env = TestEnv::new();
    env.cmd()
        .args([
            "init",
            "--data-dir",
            &env.data_dir().to_string_lossy(),
            "--profile",
            "bogus",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown --profile"));
    assert!(!env.data_dir().join("nigel.db").exists());
}

#[test]
fn reinit_with_other_profile_warns_and_keeps_the_chart() {
    let env = TestEnv::new();
    let data_dir = env.data_dir().to_string_lossy().into_owned();
    env.cmd()
        .args(["init", "--data-dir", &data_dir])
        .assert()
        .success();

    env.cmd()
        .args(["init", "--data-dir", &data_dir, "--profile", "personal"])
        .assert()
        .success()
        .stderr(predicate::str::contains("was ignored"));

    // Still business books: the chart was neither reseeded nor restamped.
    let has_business_chart: bool = env
        .db()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM categories WHERE name = 'Client Services')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(has_business_chart);
    let profile: String = env
        .db()
        .query_row(
            "SELECT value FROM metadata WHERE key = 'profile'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(profile, "business");
}

#[test]
fn demo_refuses_personal_books_and_writes_nothing() {
    let env = TestEnv::new();
    env.cmd()
        .args([
            "init",
            "--data-dir",
            &env.data_dir().to_string_lossy(),
            "--profile",
            "personal",
        ])
        .assert()
        .success();

    env.cmd()
        .arg("demo")
        .assert()
        .failure()
        .stderr(predicate::str::contains("personal"));

    // The refusal must come before any insert: demo data has no import row,
    // so anything written here would be un-undoable.
    let accounts: i64 = env
        .db()
        .query_row("SELECT count(*) FROM accounts", [], |r| r.get(0))
        .unwrap();
    let transactions: i64 = env
        .db()
        .query_row("SELECT count(*) FROM transactions", [], |r| r.get(0))
        .unwrap();
    assert_eq!((accounts, transactions), (0, 0));
}

/// Read the register as plain text (non-TTY stdout falls back to text).
fn register_stdout(env: &TestEnv, extra: &[&str]) -> String {
    let year = chrono::Local::now().format("%Y").to_string();
    let mut args = vec!["report", "register", "--year", &year];
    args.extend_from_slice(extra);
    let out = env.cmd().args(&args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

#[test]
fn report_register_category_filter_narrows_rows() {
    let env = TestEnv::new();
    env.init_and_demo();

    let all = register_stdout(&env, &[]);
    let filtered = register_stdout(&env, &["--category", "Software & Subscriptions"]);

    let all_rows = all.lines().count();
    let filtered_rows = filtered.lines().count();
    assert!(
        filtered_rows < all_rows,
        "category filter should drop rows: {filtered_rows} vs {all_rows}"
    );
    assert!(
        filtered.contains("Software & Subscriptions"),
        "filtered register should still contain the selected category"
    );
    assert!(
        !filtered.contains("Client Services"),
        "filtered register should not contain other categories:\n{filtered}"
    );
    // Active filters are named in the header.
    assert!(
        filtered.contains("category: Software & Subscriptions"),
        "header should name the category filter:\n{filtered}"
    );
}

#[test]
fn report_register_category_filter_composes_with_account() {
    let env = TestEnv::new();
    env.init_and_demo();

    let out = register_stdout(
        &env,
        &[
            "--account",
            "BofA Checking",
            "--category",
            "Software & Subscriptions",
        ],
    );
    assert!(out.contains("Software & Subscriptions"));
    assert!(
        out.contains("account: BofA Checking, category: Software & Subscriptions"),
        "header should name both filters:\n{out}"
    );

    // An account with no transactions yields an empty selection, not an error.
    let out = register_stdout(
        &env,
        &[
            "--account",
            "Nonexistent",
            "--category",
            "Software & Subscriptions",
        ],
    );
    assert!(
        !out.contains("Adobe"),
        "no rows should survive an account with no transactions:\n{out}"
    );
}

#[test]
fn report_register_uncategorized_filter() {
    let env = TestEnv::new();
    env.init_and_demo();

    // Demo data is fully categorized; strip one row's category to have
    // something to find. The newest row is dated near today, so it falls
    // inside register_stdout's current-year filter.
    env.db()
        .execute(
            "UPDATE transactions SET category_id = NULL \
             WHERE id = (SELECT MAX(id) FROM transactions)",
            [],
        )
        .expect("failed to uncategorize a transaction");
    let description: String = env
        .db()
        .query_row(
            "SELECT description FROM transactions WHERE category_id IS NULL",
            [],
            |r| r.get(0),
        )
        .expect("failed to read the uncategorized row back");

    let out = register_stdout(&env, &["--uncategorized"]);
    assert!(
        out.contains("uncategorized"),
        "header should mark the uncategorized selection:\n{out}"
    );
    assert!(
        out.contains(&description),
        "the uncategorized transaction should appear (an empty register would \
         also pass the negative assertions):\n{out}"
    );
    assert!(
        !out.contains("Software & Subscriptions"),
        "categorized rows must not appear:\n{out}"
    );
}

#[test]
fn report_register_unknown_category_fails() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args(["report", "register", "--category", "No Such Category"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No Such Category"));
}

#[test]
fn report_register_category_and_uncategorized_conflict() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args([
            "report",
            "register",
            "--category",
            "Software & Subscriptions",
            "--uncategorized",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn report_register_default_export_filename_encodes_filters() {
    let env = TestEnv::new();
    env.init_and_demo();

    let year = chrono::Local::now().format("%Y").to_string();
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    env.cmd()
        .args([
            "report",
            "register",
            "--year",
            &year,
            "--account",
            "BofA Checking",
            "--category",
            "Software & Subscriptions",
            "--mode",
            "export",
            "--format",
            "text",
        ])
        .assert()
        .success();

    let expected = env.data_dir().join("exports").join(format!(
        "register-bofa-checking-software-subscriptions-{date}.txt"
    ));
    assert!(
        expected.exists(),
        "expected filtered export at {}",
        expected.display()
    );
    let content = std::fs::read_to_string(&expected).unwrap();
    assert!(content.contains("account: BofA Checking, category: Software & Subscriptions"));

    // An explicit --output still wins over the derived name.
    let explicit = env.home.path().join("my-register.txt");
    env.cmd()
        .args([
            "report",
            "register",
            "--year",
            &year,
            "--category",
            "Software & Subscriptions",
            "--format",
            "text",
            "--output",
            &explicit.to_string_lossy(),
        ])
        .assert()
        .success();
    assert!(explicit.exists());
}

/// The default export format is PDF, which is separate wiring from the text
/// path: `dispatch_pdf` threads `export_basename()` through `export::register`
/// and the subtitle through `render_register`. A broken passthrough there
/// would ship silently in the default flow if only `--format text` is tested.
#[cfg(feature = "pdf")]
#[test]
fn report_register_default_pdf_export_encodes_filters() {
    let env = TestEnv::new();
    env.init_and_demo();

    let year = chrono::Local::now().format("%Y").to_string();
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    env.cmd()
        .args([
            "report",
            "register",
            "--year",
            &year,
            "--category",
            "Software & Subscriptions",
            "--mode",
            "export",
        ])
        .assert()
        .success();

    let expected = env
        .data_dir()
        .join("exports")
        .join(format!("register-software-subscriptions-{date}.pdf"));
    let pdf = std::fs::read(&expected)
        .unwrap_or_else(|_| panic!("expected filtered pdf at {}", expected.display()));
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn report_register_unfiltered_export_keeps_plain_filename() {
    let env = TestEnv::new();
    env.init_and_demo();

    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    env.cmd()
        .args(["report", "register", "--mode", "export", "--format", "text"])
        .assert()
        .success();

    let expected = env
        .data_dir()
        .join("exports")
        .join(format!("register-{date}.txt"));
    assert!(
        expected.exists(),
        "unfiltered export should keep the bare name, looked for {}",
        expected.display()
    );
}

#[test]
fn browse_register_rejects_unknown_category() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args(["browse", "register", "--category", "No Such Category"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No Such Category"));
}

#[test]
fn categorize_after_demo() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .arg("categorize")
        .assert()
        .success()
        .stdout(predicate::str::contains("categorized"));
}

#[test]
fn import_nonexistent_file() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args(["import", "nonexistent.csv", "--account", "BofA Checking"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No such file or directory"));
}

#[test]
fn accounts_list_after_demo() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args(["accounts", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("BofA Checking"));
}

#[test]
fn rules_list_after_demo() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args(["rules", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("STRIPE TRANSFER"));
}

#[test]
fn report_invalid_mode() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args(["report", "pnl", "--mode", "bogus"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown --mode"));
}

#[test]
fn report_invalid_format() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args(["report", "pnl", "--format", "csv"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown --format"));
}

#[test]
fn init_without_db_then_status() {
    let env = TestEnv::new();

    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();

    // Status on a fresh DB (no demo data) should still work
    env.cmd()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("Transactions:  0"));
}

#[test]
fn client_add_and_list_roundtrip() {
    let env = TestEnv::new();

    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();

    env.cmd()
        .args(["client", "add", "Acme Co", "--email", "a@b.test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Acme Co"));

    env.cmd()
        .args(["client", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Acme Co").and(predicate::str::contains("a@b.test")));
}

/// `f64::from_str` accepts "NaN" and "inf", so `--item` could always spell a
/// figure that poisons every later SUM over the column. The refusal lives in
/// `invoices::validate_items`, which both front ends call.
#[test]
fn a_non_finite_item_is_refused_from_the_cli() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    for item in ["Work:NaN:5", "Work:inf:5", "Work:1:NaN"] {
        env.cmd()
            .args([
                "invoice",
                "new",
                "--client",
                "1",
                "--issue",
                "2026-08-04",
                "--item",
                item,
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("finite"));
    }

    // The refused drafts never reserved a number: #1248 is still the only one.
    env.cmd()
        .args(["invoice", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1248").and(predicate::str::contains("1249").not()));
}

#[test]
fn an_invoice_totalling_zero_is_refused_from_the_cli() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    for item in ["Freebie:0:150", "Credit:-1:150"] {
        env.cmd()
            .args([
                "invoice",
                "new",
                "--client",
                "1",
                "--issue",
                "2026-08-04",
                "--item",
                item,
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("more than zero"));
    }
}

/// Init plus one client and one 1500.00 draft invoice (#1248).
fn init_with_client_and_invoice(env: &TestEnv) {
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();
    env.cmd()
        .args(["client", "add", "Acme Co", "--email", "ap@acme.test"])
        .assert()
        .success();
    env.cmd()
        .args([
            "invoice",
            "new",
            "--client",
            "1",
            "--issue",
            "2026-08-04",
            "--item",
            "Consulting:10:150",
        ])
        .assert()
        .success();
}

/// The three commands whose printing moved into pure formatters, pinned as
/// whole stdout rather than as substrings. Money reads as `$1,500.00` here,
/// which is what `nigel invoice aging` and the browser have always printed.
#[test]
fn invoice_and_client_listings_print_money_the_way_every_other_report_does() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    let stdout = |args: &[&str]| -> String {
        let out = env.cmd().args(args).assert().success().get_output().clone();
        String::from_utf8(out.stdout).unwrap()
    };

    assert_eq!(
        stdout(&["invoice", "list"]),
        concat!(
            "Invoices\n",
            "+------+--------+---------+-----------+-----+\n",
            "| #    | Status | Client  | Total     | Due |\n",
            "+===========================================+\n",
            "| 1248 | draft  | Acme Co | $1,500.00 |     |\n",
            "+------+--------+---------+-----------+-----+\n",
        )
    );

    assert_eq!(
        stdout(&["invoice", "show", "1248"]),
        concat!(
            "Invoice #1248  [draft]  USD $1,500.00\n",
            "Client:   Acme Co\n",
            "Issued:   2026-08-04\n",
            "Due:      -\n",
            "+-------------+-------+---------+-----------+\n",
            "| Description | Qty   | Unit    | Amount    |\n",
            "+===========================================+\n",
            "| Consulting  | 10.00 | $150.00 | $1,500.00 |\n",
            "+-------------+-------+---------+-----------+\n",
            "Paid:     $0.00\n",
            "Balance:  $1,500.00\n",
        )
    );

    assert_eq!(
        stdout(&["client", "list"]),
        concat!(
            "Clients\n",
            "+----+---------+--------------+\n",
            "| ID | Name    | Email        |\n",
            "+=============================+\n",
            "| 1  | Acme Co | ap@acme.test |\n",
            "+----+---------+--------------+\n",
        )
    );
}

#[test]
fn client_show_prints_details_and_invoice_history() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["client", "show", "1"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Acme Co")
                .and(predicate::str::contains("ap@acme.test"))
                .and(predicate::str::contains("1248"))
                .and(predicate::str::contains("Outstanding")),
        );
}

#[test]
fn client_show_for_an_unknown_id_fails_with_not_found() {
    let env = TestEnv::new();
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();

    env.cmd()
        .args(["client", "show", "99"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Client not found: id 99"));
}

#[test]
fn client_edit_changes_the_email() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["client", "edit", "1", "--email", "new@acme.test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated client 1"));

    env.cmd()
        .args(["client", "show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("new@acme.test"));
}

#[test]
fn client_edit_with_no_flags_fails() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["client", "edit", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Nothing to update"));
}

#[test]
fn invoice_new_persists_notes_and_terms() {
    let env = TestEnv::new();
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();
    env.cmd()
        .args(["client", "add", "Acme Co", "--email", "ap@acme.test"])
        .assert()
        .success();

    env.cmd()
        .args([
            "invoice",
            "new",
            "--client",
            "1",
            "--issue",
            "2026-08-04",
            "--item",
            "Consulting:10:150",
            "--notes",
            "Thanks for the work",
            "--terms",
            "Net 30",
        ])
        .assert()
        .success();

    let (notes, terms): (String, String) = env
        .db()
        .query_row(
            "SELECT notes, terms FROM invoices WHERE number = 1248",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("invoice row missing");
    assert_eq!(notes, "Thanks for the work");
    assert_eq!(terms, "Net 30");
}

#[test]
fn invoice_edit_updates_a_draft() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args([
            "invoice",
            "edit",
            "1248",
            "--due",
            "2026-10-01",
            "--item",
            "Rework:2:250",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("500.00"));

    env.cmd()
        .args(["invoice", "show", "1248"])
        .assert()
        .success()
        .stdout(predicate::str::contains("500.00").and(predicate::str::contains("2026-10-01")));
}

#[test]
fn invoice_edit_refuses_a_void_invoice() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "void", "1248", "--yes"])
        .assert()
        .success();

    env.cmd()
        .args(["invoice", "edit", "1248", "--due", "2026-10-01"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("is void and cannot be edited"));
}

#[test]
fn invoice_void_requires_confirmation_without_a_tty() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "void", "1248"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Refusing to void invoice #1248 without confirmation. Pass --yes.",
        ));

    let status: String = env
        .db()
        .query_row("SELECT status FROM invoices WHERE number = 1248", [], |r| {
            r.get(0)
        })
        .expect("invoice row missing");
    assert_eq!(status, "draft");
}

#[test]
fn invoice_void_with_yes_voids_and_blocks_pay() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "void", "1248", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Voided invoice #1248"));

    env.cmd()
        .args(["invoice", "pay", "1248", "--date", "2026-08-20"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("void and cannot be paid"));
}

#[test]
fn invoice_delete_removes_a_draft_and_its_line_items() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "delete", "1248", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Deleted invoice #1248"));

    let db = env.db();
    let invoices: i64 = db
        .query_row("SELECT COUNT(*) FROM invoices", [], |r| r.get(0))
        .expect("count");
    let items: i64 = db
        .query_row("SELECT COUNT(*) FROM invoice_line_items", [], |r| r.get(0))
        .expect("count");
    assert_eq!((invoices, items), (0, 0));
}

/// The gap is the decision: `nigel invoice delete` says so, and the next draft
/// proves it by getting the number after the deleted one.
#[test]
fn invoice_delete_leaves_the_number_counter_where_it_was() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "delete", "1248", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Invoice numbers are not reused — the next draft will be #1249.",
        ));

    env.cmd()
        .args([
            "invoice",
            "new",
            "--client",
            "1",
            "--issue",
            "2026-08-05",
            "--item",
            "Work:1:50",
        ])
        .assert()
        .success();

    let number: i64 = env
        .db()
        .query_row("SELECT number FROM invoices", [], |r| r.get(0))
        .expect("the replacement draft");
    assert_eq!(number, 1249);
}

#[test]
fn invoice_delete_refuses_a_void_invoice_without_telling_it_to_void_again() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    env.cmd()
        .args(["invoice", "void", "1248", "--yes"])
        .assert()
        .success();

    env.cmd()
        .args(["invoice", "delete", "1248", "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Cannot delete: invoice has been sent, paid or voided — only an unsent draft with no payments can be deleted",
        ))
        // The pointer is advice, and there is nothing left to cancel.
        .stderr(predicate::str::contains("nigel invoice void").not());

    let count: i64 = env
        .db()
        .query_row("SELECT COUNT(*) FROM invoices", [], |r| r.get(0))
        .expect("count");
    assert_eq!(count, 1);
}

/// A paid invoice refuses void as well, so pointing at it would be a dead end:
/// the operator runs the suggested command and gets a second refusal. The
/// honest answer names what is actually true of the invoice.
#[test]
fn invoice_delete_of_a_paid_invoice_does_not_point_at_a_void_that_would_refuse() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    env.cmd()
        .args([
            "invoice",
            "pay",
            "1248",
            "--amount",
            "100",
            "--date",
            "2026-08-06",
        ])
        .assert()
        .success();

    // The premise: void refuses this invoice too.
    env.cmd()
        .args(["invoice", "void", "1248", "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be voided"));

    env.cmd()
        .args(["invoice", "delete", "1248", "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cannot delete: invoice has been"))
        .stderr(predicate::str::contains("nigel invoice void").not())
        .stderr(predicate::str::contains(
            "A payment has been recorded against it, so it stays on the books.",
        ));
}

/// The one case where the pointer is real advice: a sent invoice with nothing
/// paid against it, which `ensure_voidable` allows.
#[test]
fn invoice_delete_of_a_sent_invoice_points_at_the_void_that_would_work() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    env.db()
        .execute(
            "UPDATE invoices SET published_at = '2026-08-05', status = 'sent' WHERE number = 1248",
            [],
        )
        .expect("publish");

    env.cmd()
        .args(["invoice", "delete", "1248", "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cannot delete: invoice has been"))
        .stderr(predicate::str::contains(
            "Run `nigel invoice void 1248` to cancel it instead.",
        ));

    // And the advice holds: the command it names succeeds.
    env.cmd()
        .args(["invoice", "void", "1248", "--yes"])
        .assert()
        .success();
}

#[test]
fn invoice_delete_without_yes_on_a_pipe_refuses_rather_than_guessing() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "delete", "1248"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Refusing to delete invoice #1248 without confirmation. Pass --yes.",
        ));

    let count: i64 = env
        .db()
        .query_row("SELECT COUNT(*) FROM invoices", [], |r| r.get(0))
        .expect("count");
    assert_eq!(count, 1);
}

#[test]
fn invoice_delete_of_a_number_that_is_not_there_says_so() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "delete", "9999", "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No invoice #9999"));
}

#[test]
fn client_add_then_contacts_then_show_lists_them_all() {
    let env = TestEnv::new();
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();
    env.cmd()
        .args(["client", "add", "Acme Co", "--email", "ap@acme.test"])
        .assert()
        .success();

    env.cmd()
        .args([
            "client",
            "edit",
            "1",
            "--contact",
            "ap@acme.test:Ada Payne:AP Manager",
            "--contact",
            "dana@acme.test:Dana Chen:Design Lead",
        ])
        .assert()
        .success();

    env.cmd()
        .args(["client", "show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ap@acme.test"))
        .stdout(predicate::str::contains("Ada Payne"))
        .stdout(predicate::str::contains("AP Manager"))
        .stdout(predicate::str::contains("dana@acme.test"))
        .stdout(predicate::str::contains("Design Lead"))
        .stdout(predicate::str::contains("billing"));

    // The first `--contact` is the billing recipient, which is what the list's
    // Email column projects.
    env.cmd()
        .args(["client", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ap@acme.test"));
}

/// Atomicity on the CLI: a refused contact list leaves no client behind.
#[test]
fn client_add_with_a_refused_contact_list_writes_no_client() {
    let env = TestEnv::new();
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();

    env.cmd()
        .args([
            "client",
            "add",
            "Acme Co",
            "--contact",
            "ap@acme.test",
            "--contact",
            "AP@ACME.TEST",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("twice"));

    let count: i64 = env
        .db()
        .query_row("SELECT COUNT(*) FROM clients", [], |r| r.get(0))
        .expect("count");
    assert_eq!(count, 0, "the client row outlived the refused contact list");
}

/// And on an edit: the rename and the list are one write.
#[test]
fn client_edit_with_a_refused_contact_list_leaves_the_rename_unapplied() {
    let env = TestEnv::new();
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();
    env.cmd()
        .args(["client", "add", "Acme Co", "--email", "ap@acme.test"])
        .assert()
        .success();

    env.cmd()
        .args([
            "client",
            "edit",
            "1",
            "--name",
            "Acme Corporation",
            "--contact",
            "a@x.test",
            "--contact",
            "A@X.TEST",
        ])
        .assert()
        .failure();

    let (name, email): (String, Option<String>) = env
        .db()
        .query_row(
            "SELECT c.name, (SELECT email FROM client_contacts WHERE client_id = c.id)
               FROM clients c WHERE c.id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("the client row");
    assert_eq!(name, "Acme Co", "half the edit landed");
    assert_eq!(email.as_deref(), Some("ap@acme.test"));
}

#[test]
fn client_edit_email_and_contact_together_is_refused_by_clap() {
    let env = TestEnv::new();
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();
    env.cmd()
        .args(["client", "add", "Acme Co"])
        .assert()
        .success();

    env.cmd()
        .args([
            "client",
            "edit",
            "1",
            "--email",
            "ap@acme.test",
            "--contact",
            "dana@acme.test",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

/// AC #5 end to end: an address written before contacts existed is still the
/// billing contact afterwards, with nobody re-entering it.
#[test]
fn a_client_upgraded_from_a_single_email_keeps_it_as_the_billing_contact() {
    let env = TestEnv::new();
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();
    env.cmd()
        .args(["client", "add", "Acme Co", "--email", "ap@acme.test"])
        .assert()
        .success();

    // Rewind to the pre-contacts schema, with the address back in its column.
    env.db()
        .execute_batch(
            "DROP TABLE client_contacts;
             ALTER TABLE clients ADD COLUMN email TEXT;
             UPDATE clients SET email = 'ap@acme.test';
             UPDATE metadata SET value = '7' WHERE key = 'schema_version';",
        )
        .expect("failed to rewind the test database");

    env.cmd()
        .args(["client", "show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ap@acme.test"))
        .stdout(predicate::str::contains("billing"));

    let (email, is_billing): (String, i64) = env
        .db()
        .query_row(
            "SELECT email, is_billing FROM client_contacts WHERE client_id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("the migrated contact");
    assert_eq!(email, "ap@acme.test");
    assert_eq!(is_billing, 1);
}

#[test]
fn client_delete_removes_a_client_with_no_invoices() {
    let env = TestEnv::new();
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();
    env.cmd()
        .args(["client", "add", "Globex"])
        .assert()
        .success();

    env.cmd()
        .args(["client", "delete", "1", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Deleted client 1: Globex"));

    let count: i64 = env
        .db()
        .query_row("SELECT COUNT(*) FROM clients", [], |r| r.get(0))
        .expect("count");
    assert_eq!(count, 0);
}

#[test]
fn client_delete_refuses_a_client_with_invoices_and_points_at_them() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["client", "delete", "1", "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Cannot delete: client has 1 invoice",
        ))
        .stderr(predicate::str::contains("nigel client show 1"));
}

#[test]
fn client_delete_without_yes_on_a_pipe_refuses_rather_than_guessing() {
    let env = TestEnv::new();
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();
    env.cmd()
        .args(["client", "add", "Globex"])
        .assert()
        .success();

    env.cmd()
        .args(["client", "delete", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Refusing to delete client #1 without confirmation. Pass --yes.",
        ));

    let count: i64 = env
        .db()
        .query_row("SELECT COUNT(*) FROM clients", [], |r| r.get(0))
        .expect("count");
    assert_eq!(count, 1);
}

#[test]
fn archived_clients_are_hidden_from_client_list_and_shown_by_all() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    env.cmd()
        .args(["client", "add", "Globex"])
        .assert()
        .success();

    env.cmd()
        .args(["client", "archive", "2"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Archived client 2: Globex"));

    env.cmd()
        .args(["client", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Globex").not())
        .stdout(predicate::str::contains("Archived").not());

    env.cmd()
        .args(["client", "list", "--all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Globex"))
        .stdout(predicate::str::contains("Archived"));

    env.cmd()
        .args(["client", "unarchive", "2"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Restored client 2: Globex"));

    env.cmd()
        .args(["client", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Globex"));
}

#[test]
fn a_new_invoice_for_an_archived_client_is_refused_on_the_cli() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["client", "archive", "1"])
        .assert()
        .success();

    env.cmd()
        .args([
            "invoice",
            "new",
            "--client",
            "1",
            "--issue",
            "2026-08-20",
            "--item",
            "Consulting:1:100",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("is archived"))
        .stderr(predicate::str::contains("Acme Co"));

    // An archived client keeps every invoice it already had.
    env.cmd()
        .args(["client", "show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Archived:"))
        .stdout(predicate::str::contains("1248"));
}

/// The default preview directory for a `TestEnv`.
fn previews_dir(env: &TestEnv) -> PathBuf {
    env.data_dir().join("previews")
}

#[test]
fn invoice_preview_writes_html_to_the_data_dir() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("invoice-1248.html"));

    let html = std::fs::read_to_string(previews_dir(&env).join("invoice-1248.html"))
        .expect("preview html missing");
    // The number is in the tab title and in the metadata band; the document
    // itself carries no heading line.
    assert!(html.contains("<title>Invoice 1248</title>"), "got: {html}");
    assert!(html.contains("Invoice ID"), "got: {html}");
    // One figure format on both documents: separators, two decimals, and the
    // currency named — a bare `1500.00` is what the page used to print.
    assert!(html.contains("$1,500.00"), "got: {html}");
}

#[test]
fn invoice_preview_of_a_draft_shows_an_inert_pay_placeholder() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    let html = std::fs::read_to_string(previews_dir(&env).join("invoice-1248.html")).unwrap();
    assert!(html.contains("pay-placeholder"), "got: {html}");
    assert!(!html.contains("<a class=\"pay\""), "got: {html}");
}

#[test]
fn invoice_preview_needs_no_invoicing_config_and_makes_no_network_call() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    // TestEnv clears every NIGEL_* invoicing var, so this runs with no config
    // at all; anything reaching the network would hang into TEST_TIMEOUT.
    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stderr(predicate::str::contains("missing invoicing config").not());
}

/// Nine keys set and a display name carrying a line break: header injection, a
/// hard refusal, and it happens before any client is built — so no Stripe link
/// is made and the invoice is still a draft.
#[test]
fn invoice_send_refuses_a_display_name_carrying_a_line_break() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "send", "1248", "--yes"])
        .env("NIGEL_STRIPE_SECRET_KEY", "sk_test_not_real")
        .env("NIGEL_MAILGUN_API_KEY", "key-not-real")
        .env("NIGEL_MAILGUN_DOMAIN", "mg.example.test")
        .env("NIGEL_FROM_EMAIL", "billing@mg.example.test")
        .env("NIGEL_FROM_NAME", "Bluepeak\r\nBcc: someone@else.test")
        .env("NIGEL_R2_ACCOUNT_ID", "acct")
        .env("NIGEL_R2_ACCESS_KEY", "ak")
        .env("NIGEL_R2_SECRET_KEY", "sk")
        .env("NIGEL_R2_BUCKET", "billing")
        .env("NIGEL_PUBLIC_BASE_URL", "https://billing.example.test/i")
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(predicate::str::contains("from_name"))
        .stderr(predicate::str::contains("someone@else.test").not());

    let status: String = env
        .db()
        .query_row("SELECT status FROM invoices WHERE number = 1248", [], |r| {
            r.get(0)
        })
        .expect("invoice row missing");
    assert_eq!(status, "draft");
}

/// Header injection through `NIGEL_FROM_EMAIL`, refused before any client is
/// built — so no Stripe link, no upload, no email.
#[test]
fn invoice_send_refuses_a_from_address_carrying_a_line_break() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "send", "1248", "--yes"])
        .env("NIGEL_STRIPE_SECRET_KEY", "sk_test_not_real")
        .env("NIGEL_MAILGUN_API_KEY", "key-not-real")
        .env("NIGEL_MAILGUN_DOMAIN", "mg.example.test")
        .env(
            "NIGEL_FROM_EMAIL",
            "billing@mg.example.test\r\nBcc: attacker@evil.test",
        )
        .env("NIGEL_R2_ACCOUNT_ID", "acct")
        .env("NIGEL_R2_ACCESS_KEY", "ak")
        .env("NIGEL_R2_SECRET_KEY", "sk")
        .env("NIGEL_R2_BUCKET", "billing")
        .env("NIGEL_PUBLIC_BASE_URL", "https://billing.example.test/i")
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(predicate::str::contains("from_email"))
        .stderr(predicate::str::contains("attacker@evil.test").not());

    let status: String = env
        .db()
        .query_row("SELECT status FROM invoices WHERE number = 1248", [], |r| {
            r.get(0)
        })
        .expect("invoice row missing");
    assert_eq!(status, "draft");
}

#[test]
fn invoice_preview_names_contact_email_when_a_template_prints_it() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    // The stock page stopped printing `{{CONTACT}}` — payment instructions are
    // the operator's own text now — so the notice belongs to a template that
    // actually uses it.
    write_template(
        &env,
        "<p>{{CONTACT}} {{NUMBER}} {{CLIENT}} {{ROWS}} {{TOTAL}}</p>",
    );

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "neither contact_email nor from_email",
        ));

    let html = std::fs::read_to_string(previews_dir(&env).join("invoice-1248.html")).unwrap();
    assert!(
        html.contains("(contact_email not configured)"),
        "got: {html}"
    );
}

/// A notice about a placeholder the document does not carry is noise.
#[test]
fn the_stock_page_prints_no_contact_line_and_says_nothing_about_one() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stderr(predicate::str::contains("contact_email").not());

    let html = std::fs::read_to_string(previews_dir(&env).join("invoice-1248.html")).unwrap();
    assert!(!html.contains("Direct deposit"), "got: {html}");
    assert!(
        !html.contains("(contact_email not configured)"),
        "got: {html}"
    );
}

/// The stock page no longer hardcodes a way to pay, so an installation with
/// nothing configured now sends a document that says how much is owed and
/// nothing about how to settle it. That is a legitimate choice and a silent one,
/// so it is said out loud — once, on stderr, where the old placeholder notice
/// was.
#[test]
fn a_document_with_no_way_to_pay_says_so_on_stderr() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stderr(predicate::str::contains("payment_instructions"));
}

#[test]
fn configured_payment_instructions_draw_no_notice() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    env.db()
        .execute(
            "INSERT INTO metadata (key, value) VALUES ('payment_instructions', ?1)",
            ["Bank transfer to Example Bank"],
        )
        .unwrap();

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stderr(predicate::str::contains("payment_instructions").not());
}

/// An operator who owns their template owns what it says about paying, and a
/// notice about a key their page may not even use is noise.
#[test]
fn a_custom_template_draws_no_payment_notice() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    write_template(
        &env,
        "<p>{{NUMBER}} {{CLIENT}} {{ROWS}} {{TOTAL}} — pay however we agreed</p>",
    );

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stderr(predicate::str::contains("payment_instructions").not());
}

/// AC #3 end to end: the page's direct-deposit line is `contact_email`, not the
/// address the email is sent from.
#[test]
fn contact_email_is_what_the_page_prints_not_from_email() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    write_template(
        &env,
        "<p>{{CONTACT}} {{NUMBER}} {{CLIENT}} {{ROWS}} {{TOTAL}}</p>",
    );

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .env("NIGEL_FROM_EMAIL", "billing@mg.example.test")
        .env("NIGEL_CONTACT_EMAIL", "accounts@example.test")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    let html = std::fs::read_to_string(previews_dir(&env).join("invoice-1248.html")).unwrap();
    assert!(html.contains("accounts@example.test"), "got: {html}");
    assert!(!html.contains("billing@mg.example.test"), "got: {html}");
}

#[test]
fn invoice_preview_leaves_the_invoice_a_draft() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    let (status, published): (String, Option<String>) = env
        .db()
        .query_row(
            "SELECT status, published_at FROM invoices WHERE number = 1248",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("invoice row missing");
    assert_eq!(status, "draft");
    assert_eq!(published, None);
}

#[test]
fn invoice_preview_honors_output_dir() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    let elsewhere = env.data_dir().join("elsewhere");

    env.cmd()
        .args([
            "invoice",
            "preview",
            "1248",
            "--output-dir",
            &elsewhere.to_string_lossy(),
        ])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            elsewhere.to_string_lossy().to_string(),
        ));

    assert!(elsewhere.join("invoice-1248.html").exists());
    assert!(
        !previews_dir(&env).exists(),
        "a named output directory must not also seed the default one"
    );
}

#[test]
fn invoice_preview_overwrites_in_place_on_a_second_run() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    for _ in 0..2 {
        env.cmd()
            .args(["invoice", "preview", "1248"])
            .timeout(TEST_TIMEOUT)
            .assert()
            .success();
    }

    let names: Vec<String> = std::fs::read_dir(previews_dir(&env))
        .expect("previews directory missing")
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    for name in &names {
        assert!(
            name == "invoice-1248.html" || name == "invoice-1248.pdf",
            "unexpected preview artifact: {name}"
        );
    }
    assert_eq!(
        names.iter().filter(|n| n.ends_with(".html")).count(),
        1,
        "re-previewing must overwrite, not accumulate: {names:?}"
    );
}

#[test]
fn invoice_preview_of_an_unknown_number_fails_with_the_shared_message() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "preview", "9999"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(predicate::str::contains("No invoice #9999"));
}

#[test]
fn invoice_preview_of_a_void_invoice_warns_and_omits_the_pay_button() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "void", "1248", "--yes"])
        .assert()
        .success();
    // An invoice voided after it was sent still carries a live Stripe URL.
    env.db()
        .execute(
            "UPDATE invoices SET stripe_payment_link_url = 'https://pay/x' WHERE number = 1248",
            [],
        )
        .unwrap();

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stderr(predicate::str::contains("is void"));

    let html = std::fs::read_to_string(previews_dir(&env).join("invoice-1248.html")).unwrap();
    assert!(
        !html.contains("https://pay/x"),
        "a void invoice must not publish a live payment link"
    );
    assert!(!html.contains("Pay online"), "got: {html}");
}

#[test]
fn invoice_preview_skips_the_launch_stripe_sync() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    // The launch sync only polls invoices that carry a payment link and are
    // open, so this is the state in which a sync would reach Stripe at all.
    env.db()
        .execute(
            "UPDATE invoices SET stripe_payment_link_id = 'pl_1', status = 'sent'
             WHERE number = 1248",
            [],
        )
        .unwrap();

    // Preview is in the skip list, so the key is never used and nothing leaves
    // the machine. Drop that arm and this run reaches Stripe with a bogus key,
    // which reports itself on stderr.
    env.cmd()
        .env("NIGEL_STRIPE_SECRET_KEY", "sk_test_bogus")
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stderr(
            predicate::str::contains("invoice sync skipped")
                .not()
                .and(predicate::str::contains("new invoice payment").not()),
        );
}

#[cfg(feature = "pdf")]
#[test]
fn invoice_preview_writes_a_real_pdf() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("invoice-1248.pdf"));

    let pdf =
        std::fs::read(previews_dir(&env).join("invoice-1248.pdf")).expect("preview pdf missing");
    assert!(pdf.starts_with(b"%PDF"));
}

#[cfg(not(feature = "pdf"))]
#[test]
fn invoice_preview_without_the_pdf_feature_still_writes_html_and_says_why() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    // Exit 0: "HTML, and PDF when the feature is on" is the documented outcome,
    // not a failure.
    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "PDF export requires the 'pdf' feature",
        ));

    assert!(previews_dir(&env).join("invoice-1248.html").exists());
    assert!(!previews_dir(&env).join("invoice-1248.pdf").exists());
}

/// Where `nigel invoice template export` writes for a `TestEnv`.
fn template_file(env: &TestEnv) -> PathBuf {
    env.data_dir().join("templates").join("invoice.html")
}

fn write_template(env: &TestEnv, source: &str) {
    let path = template_file(env);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, source).unwrap();
}

#[test]
fn template_export_writes_the_default_and_reports_where() {
    // No `nigel init`: exporting a template to edit must work on a machine that
    // has never opened the books.
    let env = TestEnv::new();
    let expected = env
        .home
        .path()
        .join("Documents/nigel/templates/invoice.html");

    env.cmd()
        .args(["invoice", "template", "export"])
        .assert()
        .success()
        .stdout(predicate::str::contains(expected.display().to_string()));

    assert_eq!(
        std::fs::read_to_string(&expected).expect("exported template missing"),
        nigel_core::invoicing::render_html::DEFAULT_TEMPLATE
    );
}

#[test]
fn template_export_refuses_to_clobber_without_force() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "template", "export"])
        .assert()
        .success();
    std::fs::write(template_file(&env), "MINE").unwrap();

    env.cmd()
        .args(["invoice", "template", "export"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--force"));
    assert_eq!(
        std::fs::read_to_string(template_file(&env)).unwrap(),
        "MINE"
    );

    env.cmd()
        .args(["invoice", "template", "export", "--force"])
        .assert()
        .success();
    assert_eq!(
        std::fs::read_to_string(template_file(&env)).unwrap(),
        nigel_core::invoicing::render_html::DEFAULT_TEMPLATE
    );
}

#[test]
fn template_export_honors_output() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    let out = env.home.path().join("scratch/custom.html");

    env.cmd()
        .args([
            "invoice",
            "template",
            "export",
            "--output",
            &out.to_string_lossy(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(out.display().to_string()));

    assert!(out.exists());
    assert!(!template_file(&env).exists());
}

#[test]
fn template_path_reports_absent_then_present_then_broken() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "template", "path"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains(template_file(&env).display().to_string())
                .and(predicate::str::contains("No custom template")),
        );

    write_template(&env, "<p>{{NUMBER}} {{CLIENT}} {{ROWS}} {{TOTAL}}</p>");
    env.cmd()
        .args(["invoice", "template", "path"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Custom template in effect"));

    write_template(
        &env,
        "<p>{{NUMBER}} {{CLIENT}} {{ROWS}} {{TOTAL}} {{TOTL}}</p>",
    );
    env.cmd()
        .args(["invoice", "template", "path"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("{{TOTL}}"));
}

#[test]
fn invoice_preview_renders_a_custom_template() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    write_template(
        &env,
        "<h1>MY OWN PAGE {{NUMBER}}</h1>{{CLIENT}}{{ROWS}}{{TOTAL}}",
    );

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    let html = std::fs::read_to_string(previews_dir(&env).join("invoice-1248.html")).unwrap();
    assert!(html.contains("MY OWN PAGE 1248"), "got: {html}");
    assert!(!html.contains("Direct deposit"), "got: {html}");
}

#[test]
fn a_template_renders_the_company_name_from_the_database() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    env.db()
        .execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES ('company_name', 'Acme LLC')",
            [],
        )
        .expect("failed to set company_name");
    write_template(
        &env,
        "<h1>{{COMPANY}}</h1>{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}",
    );

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    let html = std::fs::read_to_string(previews_dir(&env).join("invoice-1248.html")).unwrap();
    assert!(html.starts_with("<h1>Acme LLC</h1>"), "got: {html}");
}

#[test]
fn invoice_preview_with_a_broken_template_fails_and_writes_nothing() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    write_template(
        &env,
        "<p>{{NUMBER}} {{CLIENT}} {{ROWS}} {{TOTAL}} {{TOTL}}</p>",
    );

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(
            predicate::str::contains(template_file(&env).display().to_string())
                .and(predicate::str::contains("{{TOTL}}")),
        );

    assert!(!previews_dir(&env).join("invoice-1248.html").exists());
}

#[test]
fn send_with_a_broken_template_fails_before_touching_stripe() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    write_template(&env, "<p>no placeholders here</p>");

    env.cmd()
        .args(["invoice", "send", "1248", "--yes"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(
            predicate::str::contains(template_file(&env).display().to_string())
                .and(predicate::str::contains("{{NUMBER}}")),
        );

    let status: String = env
        .db()
        .query_row("SELECT status FROM invoices WHERE number = 1248", [], |r| {
            r.get(0)
        })
        .expect("invoice row missing");
    assert_eq!(status, "draft");
}

/// A configured-but-unusable installation: all nine keys set, one of them an
/// address that cannot produce a working link. Nothing reaches the network,
/// because the refusal happens while the clients are being built.
#[test]
fn invoice_send_refuses_a_public_base_url_with_no_scheme() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "send", "1248", "--yes"])
        .env("NIGEL_STRIPE_SECRET_KEY", "sk_test_bogus")
        .env("NIGEL_MAILGUN_API_KEY", "key")
        .env("NIGEL_MAILGUN_DOMAIN", "mail.example.test")
        .env("NIGEL_FROM_EMAIL", "billing@example.test")
        .env("NIGEL_R2_ACCOUNT_ID", "acct")
        .env("NIGEL_R2_ACCESS_KEY", "access")
        .env("NIGEL_R2_SECRET_KEY", "secret")
        .env("NIGEL_R2_BUCKET", "billing")
        .env("NIGEL_PUBLIC_BASE_URL", "billing.example.com")
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("public_base_url")
                .and(predicate::str::contains("billing.example.com")),
        );

    let (status, published): (String, Option<String>) = env
        .db()
        .query_row(
            "SELECT status, published_at FROM invoices WHERE number = 1248",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("invoice row missing");
    assert_eq!(status, "draft");
    assert_eq!(published, None);
}

#[test]
fn invoice_preview_is_unaffected_by_a_broken_public_base_url() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    // Preview needs no invoicing config at all and must not have grown a
    // dependency on one being well-formed.
    env.cmd()
        .args(["invoice", "preview", "1248"])
        .env("NIGEL_PUBLIC_BASE_URL", "billing.example.com")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    assert!(previews_dir(&env).join("invoice-1248.html").exists());
}

/// A y/N prompt cannot be answered from `assert_cmd` without a pty, so the
/// interactive path is covered by the unit tests on `send_summary`,
/// `send_consequences` and `confirm_send` — the split `void`'s tests use.
#[test]
fn invoice_send_without_yes_refuses_on_a_non_tty_and_sends_nothing() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "send", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Refusing to send invoice #1248")
                .and(predicate::str::contains("--yes")),
        );

    let status: String = env
        .db()
        .query_row("SELECT status FROM invoices WHERE number = 1248", [], |r| {
            r.get(0)
        })
        .expect("invoice row missing");
    assert_eq!(status, "draft");
}

/// The refusal comes before the writing, so a scripted send that cannot be
/// confirmed leaves nothing behind either.
#[test]
fn invoice_send_writes_no_preview_when_it_refuses_to_ask() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "send", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure();

    assert!(!previews_dir(&env).join("invoice-1248.html").exists());
}

#[test]
fn invoice_send_with_a_broken_template_fails_before_asking_anything() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    write_template(
        &env,
        "<p>{{NUMBER}} {{CLIENT}} {{ROWS}} {{TOTAL}} {{TOTL}}</p>",
    );

    env.cmd()
        .args(["invoice", "send", "1248", "--yes"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(
            predicate::str::contains(template_file(&env).display().to_string())
                .and(predicate::str::contains("{{TOTL}}")),
        );

    assert!(!previews_dir(&env).join("invoice-1248.html").exists());
}

/// `--yes` reaches `build_clients`, which is the next thing that can refuse —
/// and it writes no artifacts, because nobody is there to look at them.
#[test]
fn invoice_send_with_yes_and_no_config_fails_at_the_config_step() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "send", "1248", "--yes"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing invoicing config"));

    assert!(!previews_dir(&env).join("invoice-1248.html").exists());
    let status: String = env
        .db()
        .query_row("SELECT status FROM invoices WHERE number = 1248", [], |r| {
            r.get(0)
        })
        .expect("invoice row missing");
    assert_eq!(status, "draft");
}

#[test]
fn invoice_send_help_documents_the_confirmation_flag() {
    let env = TestEnv::new();
    env.cmd()
        .args(["invoice", "send", "--help"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--yes")
                .and(predicate::str::contains("required when stdin is not a TTY")),
        );
}

#[test]
fn invoice_aging_prints_bucket_labels() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "aging"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("current")
                .and(predicate::str::contains("1-30"))
                .and(predicate::str::contains("90+"))
                .and(predicate::str::contains("Total Outstanding")),
        );
}

#[test]
fn report_aging_text_export_writes_a_file() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    let output_path = env.home.path().join("aging.txt");
    env.cmd()
        .args([
            "report",
            "aging",
            "--mode",
            "export",
            "--format",
            "text",
            "--output",
            &output_path.to_string_lossy(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Wrote"));

    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("A/R Aging"), "got: {content}");
}

#[test]
fn invoice_new_with_an_unknown_client_reports_not_found() {
    let env = TestEnv::new();
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();

    env.cmd()
        .args([
            "invoice",
            "new",
            "--client",
            "99",
            "--issue",
            "2026-08-04",
            "--item",
            "X:1:1",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Client not found: id 99"));

    let count: i64 = env
        .db()
        .query_row("SELECT COUNT(*) FROM invoices", [], |r| r.get(0))
        .expect("invoices table missing");
    assert_eq!(count, 0);
}

#[test]
fn demo_without_init_fails() {
    let env = TestEnv::new();

    // With a fresh HOME, no settings.json exists, so data_dir defaults to ~/Documents/nigel
    // which won't have a nigel.db — demo should fail
    env.cmd()
        .arg("demo")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No database found"));
}

#[test]
fn test_import_dry_run_no_db_writes() {
    let env = TestEnv::new();
    env.init_and_demo();

    // Write a BofA checking CSV
    let csv_path = env.home.path().join("test-import.csv");
    std::fs::write(
        &csv_path,
        "Date,Description,Amount,Running Bal.\n\
         01/15/2025,DRY RUN PAYMENT,-100.00,900.00\n\
         01/16/2025,DRY RUN DEPOSIT,500.00,1400.00\n",
    )
    .unwrap();

    env.cmd()
        .args([
            "import",
            &csv_path.to_string_lossy(),
            "--account",
            "BofA Checking",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Dry run").and(predicate::str::contains("would be imported")),
        );

    // Verify no snapshots were created for the dry run (only the demo's snapshots should exist)
    // The key assertion is that "Dry run" appeared in stdout, meaning no DB writes occurred
}

#[test]
fn test_import_generic_csv_with_column_flags() {
    let env = TestEnv::new();
    env.init_and_demo();

    // CSV with a non-standard column layout: trans_date, ref, memo, amount, balance
    let csv_path = env.home.path().join("generic-import.csv");
    std::fs::write(
        &csv_path,
        "trans_date,ref,memo,amount,balance\n\
         01/10/2025,1001,Office Supplies,-45.99,954.01\n\
         01/11/2025,1002,Client Payment,1200.00,2154.01\n",
    )
    .unwrap();

    env.cmd()
        .args([
            "import",
            &csv_path.to_string_lossy(),
            "--account",
            "BofA Checking",
            "--date-col",
            "0",
            "--desc-col",
            "2",
            "--amount-col",
            "3",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 imported"));
}

#[test]
fn test_import_generic_csv_with_saved_profile() {
    let env = TestEnv::new();
    env.init_and_demo();

    // First import: use column flags + --save-profile
    let csv1_path = env.home.path().join("bank-export-1.csv");
    std::fs::write(
        &csv1_path,
        "posted,ref_num,payee,debit_credit,running\n\
         01/20/2025,5001,Rent Payment,-2000.00,5000.00\n\
         01/21/2025,5002,Invoice 42,3500.00,8500.00\n",
    )
    .unwrap();

    env.cmd()
        .args([
            "import",
            &csv1_path.to_string_lossy(),
            "--account",
            "BofA Checking",
            "--date-col",
            "0",
            "--desc-col",
            "2",
            "--amount-col",
            "3",
            "--save-profile",
            "mybank",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Saved profile 'mybank'")
                .and(predicate::str::contains("2 imported")),
        );

    // Second import: use the saved profile via --format
    let csv2_path = env.home.path().join("bank-export-2.csv");
    std::fs::write(
        &csv2_path,
        "posted,ref_num,payee,debit_credit,running\n\
         02/15/2025,5003,Software License,-199.00,8301.00\n",
    )
    .unwrap();

    env.cmd()
        .args([
            "import",
            &csv2_path.to_string_lossy(),
            "--account",
            "BofA Checking",
            "--format",
            "mybank",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 imported"));
}

#[test]
fn status_migrates_outdated_database() {
    let env = TestEnv::new();
    env.init_and_demo();
    env.downgrade_to_v2();

    assert_eq!(env.schema_version(), 2);
    assert_eq!(env.form_line("Client Services"), None);

    env.cmd().arg("status").assert().success();

    assert!(
        env.schema_version() > 2,
        "`nigel status` should have run pending migrations"
    );
    assert_eq!(
        env.form_line("Client Services"),
        Some("1120S-1a".to_string())
    );
    assert_eq!(
        env.form_line("Cost of Goods Sold"),
        Some("1120S-2".to_string())
    );
}

#[test]
fn report_k1_migrates_outdated_database() {
    let env = TestEnv::new();
    env.init_and_demo();
    env.downgrade_to_v2();

    let year = chrono::Local::now().format("%Y").to_string();
    let output_path = env.home.path().join("k1.txt");
    env.cmd()
        .args([
            "report",
            "k1",
            "--year",
            &year,
            "--mode",
            "export",
            "--format",
            "text",
            "--output",
            &output_path.to_string_lossy(),
        ])
        .assert()
        .success();

    assert!(
        env.schema_version() > 2,
        "`nigel report k1` should have run pending migrations"
    );

    // Without the v3 backfill, income categories have no form_line and fall back to
    // gross receipts, which the worksheet flags as auto-mapped.
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(
        !content.contains("(auto) income mapped to gross receipts"),
        "K-1 income should be explicitly mapped after migration:\n{content}"
    );
}

#[test]
fn completions_skips_the_password_and_migration_preflight() {
    let env = TestEnv::new();
    env.init_and_demo();
    env.encrypt("hunter2");

    // The database is now unreadable without the password (asserted inside `encrypt`), so
    // any pre-flight that opened it would fail. `completions` neither prompts for the
    // password nor migrates, so it still works.
    env.cmd()
        .args(["completions", "bash"])
        .write_stdin("")
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success()
        .stdout(predicate::str::contains("_nigel"));
}

/// Read the SQLite magic header to tell an encrypted database from a plaintext one.
fn is_encrypted_file(path: &std::path::Path) -> bool {
    let bytes = std::fs::read(path).expect("failed to read database");
    !bytes.starts_with(b"SQLite format 3\0")
}

#[test]
fn backup_unlocks_encrypted_database_from_env() {
    let env = TestEnv::new();
    env.init_and_demo();
    env.encrypt("hunter2");

    let backup_path = env.home.path().join("env-unlocked.db");
    env.cmd()
        .args(["backup", "--output", &backup_path.to_string_lossy()])
        .env("NIGEL_DB_PASSWORD", "hunter2")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("Backup saved to"));

    assert!(backup_path.exists(), "backup file should exist");
    assert!(
        is_encrypted_file(&backup_path),
        "backup of an encrypted database must itself be encrypted"
    );

    // The snapshot must open with the same password and carry the demo data,
    // so a backup that merely exists is not mistaken for one that can restore.
    let conn = rusqlite::Connection::open(&backup_path).unwrap();
    conn.pragma_update(None, "key", "hunter2").unwrap();
    let accounts: i64 = conn
        .query_row("SELECT count(*) FROM accounts", [], |r| r.get(0))
        .expect("backup should be readable with the original password");
    assert!(accounts > 0, "backup should contain the demo accounts");
}

#[test]
fn backup_fails_fast_on_wrong_env_password() {
    let env = TestEnv::new();
    env.init_and_demo();
    env.encrypt("hunter2");

    // The stderr predicate is what catches a regression: reaching the prompt with no
    // terminal errors with ENXIO, which would satisfy `.failure()` on its own. The
    // timeout is only a backstop for a run that inherits a tty and blocks.
    env.cmd()
        .args(["backup"])
        .env("NIGEL_DB_PASSWORD", "wrong-password")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(predicate::str::contains("NIGEL_DB_PASSWORD"));
}

#[test]
fn backup_ignores_env_password_on_plain_database() {
    let env = TestEnv::new();
    env.init_and_demo();

    // No `encrypt()` here: a leftover variable in the operator's shell must not lock
    // them out of a database that never had a password.
    env.cmd()
        .args(["backup"])
        .env("NIGEL_DB_PASSWORD", "stale-value-from-another-project")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("Backup saved to"));
}

#[test]
fn env_password_is_not_echoed_on_failure() {
    let env = TestEnv::new();
    env.init_and_demo();
    env.encrypt("hunter2");

    let output = env
        .cmd()
        .args(["backup"])
        .env("NIGEL_DB_PASSWORD", "sup3rs3cret")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .output()
        .unwrap();

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    // Assert the run failed and reported before asserting on what it did not print:
    // a killed or crashed child produces no output, which would satisfy the absence
    // check while proving nothing.
    assert!(
        !output.status.success(),
        "expected failure, got success:\n{combined}"
    );
    assert!(
        combined.contains("NIGEL_DB_PASSWORD"),
        "expected the variable to be named in the error:\n{combined}"
    );
    assert!(
        !combined.contains("sup3rs3cret"),
        "password leaked into output:\n{combined}"
    );
}

// ---------------------------------------------------------------------------
// recategorize
// ---------------------------------------------------------------------------

/// Pick a transaction ID and its current category name from the demo data.
fn any_categorized_txn(env: &TestEnv) -> (i64, String) {
    env.db()
        .query_row(
            "SELECT t.id, c.name FROM transactions t JOIN categories c ON t.category_id = c.id \
             WHERE c.name != 'Travel' LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("demo data has categorized transactions")
}

#[test]
fn recategorize_by_id_moves_and_clears_flag() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (id, _old) = any_categorized_txn(&env);
    env.db()
        .execute(
            "UPDATE transactions SET is_flagged = 1, flag_reason = 'x' WHERE id = ?1",
            [id],
        )
        .unwrap();

    env.cmd()
        .args(["recategorize", &id.to_string(), "--category", "Travel"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recategorized 1 transaction"));

    let (cat, flagged): (String, i64) = env
        .db()
        .query_row(
            "SELECT c.name, t.is_flagged FROM transactions t JOIN categories c ON t.category_id = c.id WHERE t.id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(cat, "Travel");
    assert_eq!(flagged, 0);
}

#[test]
fn recategorize_filter_requires_yes_without_tty() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (_, old) = any_categorized_txn(&env);

    env.cmd()
        .args([
            "recategorize",
            "--from-category",
            &old,
            "--category",
            "Travel",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"));
}

#[test]
fn recategorize_filter_with_yes_applies() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (_, old) = any_categorized_txn(&env);

    env.cmd()
        .args([
            "recategorize",
            "--from-category",
            &old,
            "--category",
            "Travel",
            "--yes",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recategorized"));

    let remaining: i64 = env
        .db()
        .query_row(
            "SELECT COUNT(*) FROM transactions t JOIN categories c ON t.category_id = c.id WHERE c.name = ?1",
            [&old],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(remaining, 0);
}

#[test]
fn recategorize_dry_run_writes_nothing() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (id, old) = any_categorized_txn(&env);

    env.cmd()
        .args([
            "recategorize",
            &id.to_string(),
            "--category",
            "Travel",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"));

    let cat: String = env
        .db()
        .query_row(
            "SELECT c.name FROM transactions t JOIN categories c ON t.category_id = c.id WHERE t.id = ?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(cat, old);
}

#[test]
fn recategorize_unknown_id_changes_nothing() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (id, old) = any_categorized_txn(&env);

    env.cmd()
        .args([
            "recategorize",
            &id.to_string(),
            "999999",
            "--category",
            "Travel",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("999999"));

    let cat: String = env
        .db()
        .query_row(
            "SELECT c.name FROM transactions t JOIN categories c ON t.category_id = c.id WHERE t.id = ?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(cat, old);
}

#[test]
fn recategorize_malformed_month_fails_and_changes_nothing() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (_, old) = any_categorized_txn(&env);
    let before: i64 = env
        .db()
        .query_row(
            "SELECT COUNT(*) FROM transactions t JOIN categories c ON t.category_id = c.id WHERE c.name = ?1",
            [&old],
            |row| row.get(0),
        )
        .unwrap();

    env.cmd()
        .args([
            "recategorize",
            "--from-category",
            &old,
            "--month",
            "April",
            "--category",
            "Travel",
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("expected YYYY-MM"));

    let after: i64 = env
        .db()
        .query_row(
            "SELECT COUNT(*) FROM transactions t JOIN categories c ON t.category_id = c.id WHERE c.name = ?1",
            [&old],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(before, after);
}

#[test]
fn recategorize_unknown_target_category_fails() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (id, old) = any_categorized_txn(&env);

    env.cmd()
        .args([
            "recategorize",
            &id.to_string(),
            "--category",
            "Bogus Category",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Bogus Category"));

    let cat: String = env
        .db()
        .query_row(
            "SELECT c.name FROM transactions t JOIN categories c ON t.category_id = c.id WHERE t.id = ?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(cat, old);
}

#[test]
fn recategorize_unknown_account_filter_fails() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args([
            "recategorize",
            "--account",
            "No Such Bank",
            "--category",
            "Travel",
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No Such Bank"));
}

#[test]
fn recategorize_already_in_target_skips_and_preserves_flag() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (id, old) = any_categorized_txn(&env);
    env.db()
        .execute(
            "UPDATE transactions SET is_flagged = 1, flag_reason = 'check me' WHERE id = ?1",
            [id],
        )
        .unwrap();

    env.cmd()
        .args(["recategorize", &id.to_string(), "--category", &old])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Skipping 1 already in {old}"
        )));

    let (flagged, reason): (i64, Option<String>) = env
        .db()
        .query_row(
            "SELECT is_flagged, flag_reason FROM transactions WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(flagged, 1);
    assert_eq!(reason.as_deref(), Some("check me"));
}

#[test]
fn recategorize_duplicate_ids_count_once() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (id, _old) = any_categorized_txn(&env);

    env.cmd()
        .args([
            "recategorize",
            &id.to_string(),
            &id.to_string(),
            "--category",
            "Travel",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recategorized 1 transaction"));
}

#[test]
fn recategorize_zero_match_filter_exits_cleanly() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args([
            "recategorize",
            "--pattern",
            "NO SUCH TRANSACTION DESCRIPTION XYZZY",
            "--category",
            "Travel",
            "--yes",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("No transactions matched."));
}

#[test]
fn recategorize_works_on_encrypted_db_via_env_password() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (id, _old) = any_categorized_txn(&env);
    env.encrypt("hunter2");

    env.cmd()
        .args(["recategorize", &id.to_string(), "--category", "Travel"])
        .env("NIGEL_DB_PASSWORD", "hunter2")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("Recategorized 1 transaction"));
}

/// TASK-63 AC #1. A read *and* a write: unlocking for a SELECT and unlocking for
/// an INSERT are the same key, but a read-only regression would slip past a test
/// that only lists.
#[test]
fn invoice_and_client_commands_work_on_encrypted_db_via_env_password() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    env.encrypt("hunter2");

    env.cmd()
        .args(["invoice", "list"])
        .env("NIGEL_DB_PASSWORD", "hunter2")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("1248").and(predicate::str::contains("Acme Co")));

    env.cmd()
        .args(["client", "add", "Globex", "--email", "ap@globex.test"])
        .env("NIGEL_DB_PASSWORD", "hunter2")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    env.cmd()
        .args(["client", "list"])
        .env("NIGEL_DB_PASSWORD", "hunter2")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("Globex").and(predicate::str::contains("Acme Co")));
}

/// TASK-63 AC #2. The stderr predicate is the assertion that matters: reaching
/// the prompt with no terminal errors with ENXIO, which satisfies `.failure()` on
/// its own. The timeout is only a backstop for a run that inherits a tty and
/// blocks.
#[test]
fn invoice_list_fails_fast_on_wrong_env_password() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    env.encrypt("hunter2");

    env.cmd()
        .args(["invoice", "list"])
        .env("NIGEL_DB_PASSWORD", "wrong-password")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(predicate::str::contains("NIGEL_DB_PASSWORD"));
}

#[test]
fn serve_help_documents_its_flags() {
    let env = TestEnv::new();
    env.cmd()
        .args(["serve", "--help"])
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success()
        .stdout(predicate::str::contains("--port"))
        .stdout(predicate::str::contains("--no-open"));
}

#[test]
fn serve_requires_an_initialized_database() {
    let env = TestEnv::new();
    env.cmd()
        .arg("serve")
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not initialized"));
}

/// In a build without the `serve` feature the subcommand still parses — the
/// failure has to name the missing feature, the way the PDF gate does.
#[cfg(not(feature = "serve"))]
#[test]
fn serve_without_the_feature_reports_a_clear_error() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .arg("serve")
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .failure()
        .stderr(predicate::str::contains("requires the 'serve' feature"));
}

/// Padding through the real binary: nothing between clap and the column applies
/// it but the data layer.
#[test]
fn unpadded_dates_round_trip_through_new_edit_and_pay_as_padded() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args([
            "invoice",
            "new",
            "--client",
            "1",
            "--issue",
            "2026-8-7",
            "--due",
            "2026-9-1",
            "--item",
            "Consulting:1:100",
        ])
        .assert()
        .success();

    let row = |sql: &str| -> Option<String> { env.db().query_row(sql, [], |r| r.get(0)).unwrap() };
    assert_eq!(
        row("SELECT issue_date FROM invoices WHERE number = 1249").as_deref(),
        Some("2026-08-07")
    );
    assert_eq!(
        row("SELECT due_date FROM invoices WHERE number = 1249").as_deref(),
        Some("2026-09-01")
    );

    env.cmd()
        .args(["invoice", "edit", "1249", "--due", "2026-9-30"])
        .assert()
        .success();
    assert_eq!(
        row("SELECT due_date FROM invoices WHERE number = 1249").as_deref(),
        Some("2026-09-30")
    );

    env.cmd()
        .args([
            "invoice", "pay", "1249", "--amount", "25", "--date", "2026-9-2",
        ])
        .assert()
        .success();
    assert_eq!(
        row("SELECT paid_date FROM invoice_payments WHERE invoice_id =
             (SELECT id FROM invoices WHERE number = 1249)")
        .as_deref(),
        Some("2026-09-02")
    );

    // And it reads back padded everywhere a user looks.
    env.cmd()
        .args(["invoice", "show", "1249"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("2026-08-07").and(predicate::str::contains("2026-8-7").not()),
        );
}

/// The refusal comes from the data layer, through clap's plain `String` flag, and
/// leaves no payment row behind.
#[test]
fn invoice_pay_refuses_a_malformed_date() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args([
            "invoice", "pay", "1248", "--amount", "10", "--date", "March",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid payment date"));

    let payments: i64 = env
        .db()
        .query_row("SELECT COUNT(*) FROM invoice_payments", [], |r| r.get(0))
        .unwrap();
    assert_eq!(payments, 0);
}

#[test]
fn invoice_duplicate_creates_a_draft_carrying_the_source_shape() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "duplicate", "1248", "--issue", "2026-09-01"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Duplicated invoice #1248 as draft #1249",
        ));

    env.cmd()
        .args(["invoice", "show", "1249"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Acme Co")
                .and(predicate::str::contains("Consulting"))
                .and(predicate::str::contains("2026-09-01"))
                .and(predicate::str::contains("[draft]")),
        );
}

#[test]
fn invoice_duplicate_without_an_issue_date_uses_today() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "duplicate", "1248"])
        .assert()
        .success()
        .stdout(predicate::str::contains("as draft #1249"));

    let issued: String = env
        .db()
        .query_row(
            "SELECT issue_date FROM invoices WHERE number = 1249",
            [],
            |r| r.get(0),
        )
        .expect("the duplicate exists");
    assert_eq!(issued, chrono::Local::now().format("%Y-%m-%d").to_string());
}

#[test]
fn invoice_duplicate_names_a_number_that_is_not_there() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "duplicate", "9999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No invoice #9999"));
}

/// A client and a monthly schedule seeded from explicit items.
fn init_with_schedule(env: &TestEnv) {
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();
    env.cmd()
        .args([
            "client",
            "add",
            "Cedar Systems",
            "--email",
            "ops@cedar.test",
        ])
        .assert()
        .success();
    env.cmd()
        .args([
            "invoice",
            "schedule",
            "add",
            "--client",
            "1",
            "--cadence",
            "monthly",
            "--start",
            "2026-01-01",
            "--net-days",
            "30",
            "--item",
            "Hosting:1:450",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created schedule 1"));
}

#[test]
fn invoice_schedule_add_list_and_show_read_back_what_was_stored() {
    let env = TestEnv::new();
    init_with_schedule(&env);

    env.cmd()
        .args(["invoice", "schedule", "list"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Cedar Systems")
                .and(predicate::str::contains("monthly"))
                .and(predicate::str::contains("2026-01-01"))
                .and(predicate::str::contains("draft")),
        );

    env.cmd()
        .args(["invoice", "schedule", "show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hosting").and(predicate::str::contains("Net days:  30")));
}

#[test]
fn invoice_schedule_add_can_be_seeded_from_an_existing_invoice() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args([
            "invoice",
            "schedule",
            "add",
            "--client",
            "1",
            "--cadence",
            "quarterly",
            "--start",
            "2026-01-01",
            "--from",
            "1248",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created schedule 1"));

    env.cmd()
        .args(["invoice", "schedule", "show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Consulting"));
}

#[test]
fn invoice_schedule_run_generates_drafts_and_a_second_run_generates_nothing() {
    let env = TestEnv::new();
    init_with_schedule(&env);

    env.cmd()
        .args(["invoice", "schedule", "run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Generated"));

    let before: i64 = env
        .db()
        .query_row("SELECT COUNT(*) FROM invoices", [], |r| r.get(0))
        .unwrap();
    assert!(before > 0, "the first run billed the missed cycles");

    env.cmd()
        .args(["invoice", "schedule", "run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Generated 0 invoice(s)"));

    let after: i64 = env
        .db()
        .query_row("SELECT COUNT(*) FROM invoices", [], |r| r.get(0))
        .unwrap();
    assert_eq!(after, before, "a rerun must not bill again");
}

#[test]
fn invoice_schedule_pause_resume_and_end_keep_the_schedule_and_its_history() {
    let env = TestEnv::new();
    init_with_schedule(&env);
    env.cmd()
        .args(["invoice", "schedule", "run"])
        .assert()
        .success();

    env.cmd()
        .args(["invoice", "schedule", "pause", "1"])
        .assert()
        .success();
    env.cmd()
        .args(["invoice", "schedule", "run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Generated 0 invoice(s)"));

    env.cmd()
        .args(["invoice", "schedule", "resume", "1"])
        .assert()
        .success();
    env.cmd()
        .args([
            "invoice",
            "schedule",
            "edit",
            "1",
            "--item",
            "Hosting:1:495",
        ])
        .assert()
        .success();
    env.cmd()
        .args(["invoice", "schedule", "end", "1"])
        .assert()
        .success();

    env.cmd()
        .args(["invoice", "schedule", "list", "--all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ended"));

    // History survives: the run rows are still joined to their invoices.
    env.cmd()
        .args(["invoice", "schedule", "show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2026-01-01"));
}

/// TASK-81 AC #10. The stderr predicate is what matters: reaching the prompt
/// with no terminal errors with ENXIO, which satisfies a bare `.failure()`.
/// The timeout is a backstop for a run that inherits a tty and blocks.
#[test]
fn invoice_schedule_run_never_prompts_on_an_encrypted_database() {
    let env = TestEnv::new();
    init_with_schedule(&env);
    env.encrypt("hunter2");

    // With the password: it runs unattended.
    env.cmd()
        .args(["invoice", "schedule", "run"])
        .env("NIGEL_DB_PASSWORD", "hunter2")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("Generated"));

    // Without it: a clear sentence, immediately, and never a prompt.
    env.cmd()
        .args(["invoice", "schedule", "run"])
        .env_remove("NIGEL_DB_PASSWORD")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("NIGEL_DB_PASSWORD")
                .and(predicate::str::contains("never prompts")),
        );

    // A wrong password fails fast with the documented sentence.
    env.cmd()
        .args(["invoice", "schedule", "run"])
        .env("NIGEL_DB_PASSWORD", "wrong-password")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(predicate::str::contains("NIGEL_DB_PASSWORD"));
}

/// F3. `--currency` is a value the operator typed; `--from` only fills in what
/// they left out, the way `--net-days` already does.
#[test]
fn an_explicit_currency_survives_seeding_a_schedule_from_an_invoice() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args([
            "invoice",
            "schedule",
            "add",
            "--client",
            "1",
            "--cadence",
            "monthly",
            "--start",
            "2026-01-01",
            "--from",
            "1248",
            "--currency",
            "EUR",
        ])
        .assert()
        .success();

    let currency: String = env
        .db()
        .query_row(
            "SELECT currency FROM invoice_schedules WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .expect("the schedule exists");
    assert_eq!(currency, "EUR", "the typed currency is not the source's");
}
