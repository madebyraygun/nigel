//! The checks `docs/runbooks/verify-account-classification.md` asks an operator
//! to make by hand, made by the suite instead.
//!
//! The runbook has two halves. One asks the operator to read *their own* chart
//! of accounts and judge what the backfill got wrong — that half is judgement
//! and stays theirs. The other is a set of mechanical promises about what
//! migration v10 decides and what the commands do afterwards, and that half
//! belongs here, against a fixture.
//!
//! Every test upgrades a real database through the real binary: `downgrade_to_v9`
//! puts the books back the way an installation that never ran v10 holds them,
//! and the next command to open them migrates. That is the path an operator
//! actually takes, rather than a migration invoked directly.

use predicates::prelude::*;

mod common;
use common::{TestEnv, TEST_TIMEOUT};

/// Books with a chart to classify, rewound to before v10 ran.
fn books_awaiting_the_migration(env: &TestEnv) {
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();
    env.downgrade_to_v9();
}

/// Read a category's class straight out of the database, which is the fact the
/// reports go on to read.
fn class_of_category(env: &TestEnv, name: &str) -> String {
    env.db()
        .query_row(
            "SELECT class FROM categories WHERE name = ?1",
            [name],
            |row| row.get(0),
        )
        .unwrap_or_else(|e| panic!("no class for category {name}: {e}"))
}

fn class_of_account(env: &TestEnv, name: &str) -> String {
    env.db()
        .query_row(
            "SELECT class FROM accounts WHERE name = ?1",
            [name],
            |row| row.get(0),
        )
        .unwrap_or_else(|e| panic!("no class for account {name}: {e}"))
}

/// Insert a category the way a pre-v10 database holds one — no class column to
/// write, because `downgrade_to_v9` dropped it.
fn seed_category(env: &TestEnv, name: &str, category_type: &str) {
    env.db()
        .execute(
            "INSERT INTO categories (name, category_type) VALUES (?1, ?2)",
            [name, category_type],
        )
        .unwrap_or_else(|e| panic!("failed to seed category {name}: {e}"));
}

/// Insert an account directly, so a type outside `ACCOUNT_TYPES` can be tested:
/// every supported surface validates the type, but a legacy or imported row can
/// still carry one, and the runbook tells the operator to go looking for it.
fn seed_account(env: &TestEnv, name: &str, account_type: &str) {
    env.db()
        .execute(
            "INSERT INTO accounts (name, account_type) VALUES (?1, ?2)",
            [name, account_type],
        )
        .unwrap_or_else(|e| panic!("failed to seed account {name}: {e}"));
}

/// Step 1: the operator is told they will have seen the migration go by. The
/// banner is the only notice they get, so its wording is part of the contract.
#[test]
fn upgrading_announces_the_classification_migration_by_name() {
    let env = TestEnv::new();
    books_awaiting_the_migration(&env);

    env.cmd()
        .arg("status")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "Applying migration v10: classify accounts and categories as asset/liability/equity/revenue/expense",
        ));

    assert!(
        env.schema_version() > 9,
        "the books should have been migrated past v9, not left where they were"
    );
}

/// Step 2 and the migration's own table: credit cards and lines of credit are
/// liabilities, everything else is an asset — including a type the current
/// build would not let you create, which is exactly the row the runbook sends
/// the operator looking for.
#[test]
fn the_backfill_reads_account_type_and_defaults_the_rest_to_asset() {
    let env = TestEnv::new();
    books_awaiting_the_migration(&env);

    seed_account(&env, "Business Checking", "checking");
    seed_account(&env, "Company Card", "credit_card");
    seed_account(&env, "Equipment Loan", "line_of_credit");
    seed_account(&env, "Payroll Clearing", "payroll");
    seed_account(&env, "Vehicle Note", "vehicle_loan");

    env.cmd()
        .arg("status")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    assert_eq!(class_of_account(&env, "Business Checking"), "asset");
    assert_eq!(class_of_account(&env, "Company Card"), "liability");
    assert_eq!(class_of_account(&env, "Equipment Loan"), "liability");
    assert_eq!(class_of_account(&env, "Payroll Clearing"), "asset");

    // The sharp edge on the accounts side: a loan under an unrecognised type
    // falls through to `asset`, which inflates cash by its balance and is
    // invisible until someone reads the Class column.
    assert_eq!(class_of_account(&env, "Vehicle Note"), "asset");
}

/// Step 3, and the reason the runbook exists: the equity promotion matches two
/// **exact** names. Every near miss an operator might plausibly have typed
/// stays on `expense`, silently, which is money paid to the owner being
/// deducted from business income.
///
/// Pinned in both directions on purpose. If the match is ever loosened to catch
/// these, that is a decision someone should make deliberately, with this test
/// in front of them.
#[test]
fn the_backfill_promotes_only_the_two_exact_names_to_equity() {
    let env = TestEnv::new();
    books_awaiting_the_migration(&env);

    let near_misses = [
        "Distributions",
        "Owner's Draw",
        "Owner Draw",
        "Shareholder Distribution",
        "Member Draw",
        "Owner Draw / Distributions",
        "owner draw / distribution",
    ];
    for name in near_misses {
        seed_category(&env, name, "expense");
    }
    seed_category(&env, "Capital Contribution", "income");

    env.cmd()
        .arg("status")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    // The two literals the migration names.
    assert_eq!(
        class_of_category(&env, "Owner Draw / Distribution"),
        "equity"
    );
    assert_eq!(class_of_category(&env, "Owner Contribution"), "equity");

    for name in near_misses {
        assert_eq!(
            class_of_category(&env, name),
            "expense",
            "{name} is a near miss on the exact-name rule and must stay where the general rule put it"
        );
    }
    assert_eq!(class_of_category(&env, "Capital Contribution"), "revenue");
}

/// Step 1's actual instruction: the proof v10 has run is a Class column, and
/// the runbook tells the operator where to look for it.
#[test]
fn accounts_list_carries_the_class_between_type_and_institution() {
    let env = TestEnv::new();
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();
    env.cmd()
        .args(["accounts", "add", "Business Checking", "--type", "checking"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    let listed = env
        .cmd()
        .args(["accounts", "list"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();
    let printed = String::from_utf8(listed.get_output().stdout.clone()).expect("stdout is utf-8");

    let header = printed
        .lines()
        .find(|line| line.contains("Type") && line.contains("Class"))
        .unwrap_or_else(|| panic!("no header row with a Class column:\n{printed}"));

    let type_at = header.find("Type").expect("a Type column");
    let class_at = header.find("Class").expect("a Class column");
    let institution_at = header.find("Institution").expect("an Institution column");
    assert!(
        type_at < class_at && class_at < institution_at,
        "Class belongs between Type and Institution, got: {header}"
    );
}

/// Step 2's promise about `accounts edit`: a partial update that changes the
/// class and leaves everything else where it was — institution and last four
/// included, which is what the runbook says and what nothing proved.
#[test]
fn accounts_edit_sets_the_class_and_leaves_the_rest_alone() {
    let env = TestEnv::new();
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();
    env.cmd()
        .args([
            "accounts",
            "add",
            "Equipment Loan",
            "--type",
            "checking",
            "--institution",
            "Harbor & Vale",
            "--last-four",
            "4821",
        ])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    env.cmd()
        .args(["accounts", "edit", "1", "--class", "liability"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    let (name, class, institution, last_four): (String, String, Option<String>, Option<String>) =
        env.db()
            .query_row(
                "SELECT name, class, institution, last_four FROM accounts WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read the edited account");

    assert_eq!(class, "liability");
    assert_eq!(name, "Equipment Loan");
    assert_eq!(institution.as_deref(), Some("Harbor & Vale"));
    assert_eq!(last_four.as_deref(), Some("4821"));
}

/// Step 3's warning, which the runbook states too broadly: `categories update`
/// replaces the **line fields** it is not given, writing them empty — but it
/// keeps the class when `--class` is absent rather than blanking that too.
///
/// Both halves matter to an operator following the instructions. The first is
/// why they are told to copy the tax line out first; the second means setting a
/// class alone does not have to restate one.
#[test]
fn categories_update_blanks_the_line_fields_it_is_not_given_but_keeps_the_class() {
    let env = TestEnv::new();
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();

    let id: i64 = env
        .db()
        .query_row(
            "SELECT id FROM categories WHERE name = 'Owner Draw / Distribution'",
            [],
            |row| row.get(0),
        )
        .expect("the business chart seeds a distributions category");

    env.db()
        .execute(
            "UPDATE categories SET tax_line = 'Not deductible', form_line = 'K-16d' WHERE id = ?1",
            [id],
        )
        .expect("give the category both line fields to lose");

    env.cmd()
        .args([
            "categories",
            "update",
            &id.to_string(),
            "Owner Distributions",
            "--type",
            "expense",
        ])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    let (tax_line, form_line, class): (Option<String>, Option<String>, String) = env
        .db()
        .query_row(
            "SELECT tax_line, form_line, class FROM categories WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read the updated category");

    assert!(
        tax_line.as_deref().unwrap_or("").is_empty(),
        "an omitted --tax-line is written empty, which is why the runbook says to restate it"
    );
    assert!(
        form_line.as_deref().unwrap_or("").is_empty(),
        "an omitted --form-line is written empty too"
    );
    assert_eq!(
        class, "equity",
        "the class survives an update that does not name one"
    );
}
