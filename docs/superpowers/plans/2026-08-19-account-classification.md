# Account Classification Implementation Plan (TASK-9.1)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One accounting-class vocabulary — `asset`, `liability`, `equity`, `revenue`, `expense` — carried by both accounts and categories, with every report classifying from it instead of from account-type strings or category-name checks. Owner distributions stop being reported as deductions, owner contributions get somewhere correct to go, and Schedule L (TASK-102.1) gets the field it needs.

**Architecture:** A Rust enum first, a column second. `nigel_core::db::AccountClass` is a closed set in the shape `db::Profile` already established — `as_str()` / `parse()` — plus serde derives so the HTTP layer validates at deserialization rather than in a hand-written checker (the direction TASK-60 names). `accounts.class` and `categories.class` are `TEXT NOT NULL` with a `CHECK (class IN (…))`, so the closed set holds against a hand-edited database too. One migration (v10) backfills both tables in the task's exact order — the general mapping first, then `Owner Draw / Distribution` → `equity` by name — and idempotently seeds an `Owner Contribution` equity category on business-profile databases. The sign convention lives in exactly one function, `reports::natural_balance`, with a five-row table test. Every site that matches a class matches it exhaustively: **no `_ =>` arm on an `AccountClass` match anywhere**, because that arm is precisely how distributions came to be counted as deductions.

`category_type` and `account_type` both stay. They are the user-facing vocabulary the UI organizes by; `class` is accounting structure underneath. No table merger, no journal lines, no Schedule L — those are TASK-9.2 and TASK-102.1.

**Tech Stack:** Rust 2021, rusqlite (bundled-sqlcipher), axum, serde, ratatui; TypeScript, Lit 3, Web Awesome, vitest, axe.

**Spec:** `docs/superpowers/specs/2026-08-19-account-classification-design.md` (binding; where it is silent the task text is binding). Task: `backlog task 9.1 --plain`. Companion direction: `backlog task 60 --plain`.

This plan covers acceptance criteria **#1–#10**.

**Delivery: the PR from `feat/account-classification` is opened as a DRAFT and stays a draft until the operator reviews it** (AC #10, and the spec's delivery note). Do not mark it ready for review.

## Flagged questions for the operator

These are classifications the spec's mapping cannot express. They are recorded, not invented around — the plan implements the mapping as written and leaves the behaviour otherwise unchanged.

1. **`Transfer` has no class that describes it.** The stock `Transfer` category (both profiles) is `category_type = 'expense'`, so the mapping lands it on `class = 'expense'`. A transfer between own accounts is neither an expense nor equity; it is a movement between two assets. After this change, `form_line = 'excluded'` is still the *only* thing keeping it out of P&L, the expense breakdown and cash flow — the class does not express it. The plan keeps `EXCLUDE_TRANSFERS` in place beside the class filters for exactly that reason. Whether a sixth class, a `class = 'asset'` category, or a separate "excluded from the income statement" flag is the right answer belongs with TASK-9.2's journal lines.

2. **`Uncategorized` maps to `expense`.** Same mechanism: seeded as `category_type = 'expense'`, so the mapping classes unreviewed activity as a deduction. That matches today's behaviour exactly (nothing changes on the fixture books), but "not yet classified" is a state the five-class vocabulary cannot say.

3. **An unrecognized `account_type` has no mapping.** `ACCOUNT_TYPES` is `checking`, `credit_card`, `line_of_credit`, `payroll`; the spec's backfill list also names `savings`, which the data layer does not accept (it appears only in `wc-balance-list.preview.ts` fixture data). Every type that exists is covered. A value from outside that set — a database written by another tool — has no mapping, and `NOT NULL` demands one. The migration lands it on `asset`, which is the choice that can never be counted as a deduction, and the plan documents that as the fallback. If the operator would rather the migration refuse such a row, say so and Task 1 changes.

4. **`Equipment` stays an expense.** Capitalized purchases are an asset in accounting structure and a deduction line on the return; the mapping follows `category_type` and keeps it on `expense`. Depreciation and fixed assets are out of scope here.

## Global Constraints

- **⛔ No real book data, in any file, fixture, test name, doc or commit message.** Fictional cast only: Acme, Cedar Systems, Juniper Labs, Harbor & Vale, Globex, Initech, with invented amounts. Statutory figures every filer shares (the 50% meals limit, the $800 CA minimum) are allowed. Sweep with `./scripts/check-no-real-data.sh --staged` and **judge it by its exit status, never by grepping its output**.
- **Tests run serially:** `cargo test -- --test-threads=1`. The DB password is a process global.
- **The CI variants that matter here**, all of which must pass locally before a task is done:
  ```bash
  cargo fmt --check
  cargo clippy -- -D warnings
  cargo test -- --test-threads=1
  cargo test --no-default-features -- --test-threads=1
  cargo test --no-default-features --features serve -- --test-threads=1
  cargo test -p nigel-core -- --test-threads=1
  ```
  The `-p nigel-core` run is not redundant: a root-level run unifies its dependencies' features with `nigel`'s and masks what `nigel-core` actually ships.
- **Web changes run** `npm test`, `npm run lint`, `npm run typecheck` from `web/` — all three, all green.
- **Component-first UI (MANDATORY).** Every visual change ships through `@nigel/ui`: the component lives in `web/packages/ui/src/components/wc-*.ts`, a co-located `wc-*.preview.ts` covers the visible states, and `wc-*.test.ts` calls `describePreviewA11y(preview)` for zero axe violations. No bespoke component implementations in `web/apps/app/src/components/`. A component rendering a `wa-*` primitive adopts `controlsCss`.
- **No `_ =>` arm on any `match` over `AccountClass`.** The compiler is the guard against a sixth class being silently absorbed. The one permitted default is `AccountClass::parse` answering `None` for a string that is not a class — which the caller turns into an error, never into a class.
- **No debit/credit vocabulary on any user-facing surface** (AC #6). The five class words are the entire vocabulary, in the CLI, the TUI, the web UI, and every label and help string.
- **No provenance comments.** No "added by migration v10", "renamed because…", "don't change this back". Describe the current state; `git log` and `backlog/decisions/` carry history. The same rule applies to `docs/`.
- **Never edit a task file directly.** Every read goes through `backlog task … --plain`.
- **The PR is a DRAFT.**

---

### Task 1: `AccountClass`, the columns, and the migration

**Files:**
- Modify: `crates/nigel-core/src/db.rs` (the enum, `SCHEMA`, `BUSINESS_CATEGORIES`, `PERSONAL_CATEGORIES`, the seeding insert)
- Modify: `crates/nigel-core/src/migrations.rs` (migration v10)
- Test: `crates/nigel-core/src/db.rs` `mod tests`, `crates/nigel-core/src/migrations.rs` `mod tests`

**Interfaces:**
- Consumes: `db::Profile`, `db::get_metadata`/`set_metadata`, `migrations::apply_migrations`.
- Produces:
  - `pub enum db::AccountClass { Asset, Liability, Equity, Revenue, Expense }`, `Copy`, `Serialize`/`Deserialize` as lowercase.
  - `AccountClass::as_str(self) -> &'static str`
  - `AccountClass::parse(value: &str) -> Option<Self>`
  - `AccountClass::from_db(value: &str) -> Result<Self>` — the fallible read used by every query.
  - `AccountClass::ALL: [AccountClass; 5]`
  - `db::class_for_account_type(account_type: &str) -> AccountClass`
  - `db::class_for_category_type(category_type: &str) -> AccountClass`
  - `db::OWNER_CONTRIBUTION: &str = "Owner Contribution"`
  - Columns `accounts.class` and `categories.class`, schema version `10`.

The enum is `db::Profile`'s shape with two additions. `Serialize`/`Deserialize` are what make the HTTP layer's validation free: `ApiJson` turns a serde failure into a `400` in the error envelope, so an unknown class never reaches the data layer and there is no hand-written checker to keep in step with the CHECK constraint. `from_db` is the third method because a class read out of SQLite is not the same event as a class read off a request — a value the CHECK constraint should have refused is a damaged database, and it surfaces as an error rather than as a default.

`class_for_account_type` takes `&str` rather than a closed type because `ACCOUNT_TYPES` is still a `&[&str]` (TASK-60's other half), and because a database written by another tool can hold anything. Its fallback is `Asset` — see flagged question 3.

- [ ] **Step 1: Write the failing tests**

Add to `crates/nigel-core/src/db.rs`, inside `mod tests`:

```rust
    #[test]
    fn every_class_round_trips_through_its_stored_string() {
        for class in AccountClass::ALL {
            assert_eq!(AccountClass::parse(class.as_str()), Some(class));
        }
        assert_eq!(AccountClass::ALL.len(), 5);
    }

    #[test]
    fn a_string_that_is_not_a_class_is_an_error_rather_than_a_default() {
        assert_eq!(AccountClass::parse("Asset"), None);
        assert_eq!(AccountClass::parse(""), None);
        let err = AccountClass::from_db("contra-asset").unwrap_err();
        assert!(
            matches!(err, crate::error::NigelError::Invalid(_)),
            "got: {err}"
        );
    }

    #[test]
    fn account_types_and_category_types_map_to_the_classes_the_task_names() {
        assert_eq!(class_for_account_type("checking"), AccountClass::Asset);
        assert_eq!(class_for_account_type("savings"), AccountClass::Asset);
        assert_eq!(class_for_account_type("payroll"), AccountClass::Asset);
        assert_eq!(
            class_for_account_type("credit_card"),
            AccountClass::Liability
        );
        assert_eq!(
            class_for_account_type("line_of_credit"),
            AccountClass::Liability
        );
        // Nothing else has a mapping; asset is the reading that can never be
        // counted as a deduction.
        assert_eq!(class_for_account_type("brokerage"), AccountClass::Asset);

        assert_eq!(class_for_category_type("income"), AccountClass::Revenue);
        assert_eq!(class_for_category_type("expense"), AccountClass::Expense);
    }

    #[test]
    fn the_seeded_business_chart_carries_its_classes() {
        let (_dir, conn) = test_db();
        let class_of = |name: &str| -> String {
            conn.query_row(
                "SELECT class FROM categories WHERE name = ?1",
                [name],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(class_of("Client Services"), "revenue");
        assert_eq!(class_of("Office Expense"), "expense");
        assert_eq!(class_of("Owner Draw / Distribution"), "equity");
        assert_eq!(class_of("Owner Contribution"), "equity");
    }

    #[test]
    fn the_personal_chart_carries_its_classes_and_seeds_no_equity() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("personal.db")).unwrap();
        init_db_with_profile(&conn, Profile::Personal).unwrap();

        let equity: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM categories WHERE class = 'equity'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(equity, 0, "a household chart has no owner equity");

        let salary: String = conn
            .query_row(
                "SELECT class FROM categories WHERE name = 'Salary & Wages'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(salary, "revenue");
    }

    #[test]
    fn the_check_constraint_refuses_a_class_outside_the_set() {
        let (_dir, conn) = test_db();
        let err = conn
            .execute(
                "INSERT INTO categories (name, category_type, class) \
                 VALUES ('Bogus', 'expense', 'contra-asset')",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().contains("CHECK constraint failed"), "{err}");

        let err = conn
            .execute(
                "INSERT INTO accounts (name, account_type, class) \
                 VALUES ('Bogus', 'checking', 'contra-asset')",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().contains("CHECK constraint failed"), "{err}");
    }
```

Add to `crates/nigel-core/src/migrations.rs`, inside `mod tests`:

```rust
    /// A pre-v10 database: the current schema with the class columns dropped
    /// back off, which is what an installation upgrading into this migration
    /// actually holds.
    fn db_without_classes() -> (tempfile::TempDir, Connection) {
        let (dir, conn) = test_db();
        conn.execute_batch(
            "ALTER TABLE accounts DROP COLUMN class;
             ALTER TABLE categories DROP COLUMN class;
             DELETE FROM categories WHERE name = 'Owner Contribution';",
        )
        .unwrap();
        set_metadata(&conn, "schema_version", "9").unwrap();
        (dir, conn)
    }

    fn class_of(conn: &Connection, table: &str, name: &str) -> String {
        conn.query_row(
            &format!("SELECT class FROM {table} WHERE name = ?1"),
            [name],
            |r| r.get(0),
        )
        .unwrap_or_else(|e| panic!("{table}.{name}: {e}"))
    }

    #[test]
    fn v10_backfills_every_account_and_category_on_the_task_mapping() {
        let (_dir, conn) = db_without_classes();
        conn.execute_batch(
            "INSERT INTO accounts (name, account_type) VALUES ('Harbor Checking', 'checking');
             INSERT INTO accounts (name, account_type) VALUES ('Harbor Card', 'credit_card');
             INSERT INTO accounts (name, account_type) VALUES ('Harbor LOC', 'line_of_credit');
             INSERT INTO accounts (name, account_type) VALUES ('Harbor Payroll', 'payroll');",
        )
        .unwrap();

        run_migrations(&conn).unwrap();

        assert_eq!(class_of(&conn, "accounts", "Harbor Checking"), "asset");
        assert_eq!(class_of(&conn, "accounts", "Harbor Payroll"), "asset");
        assert_eq!(class_of(&conn, "accounts", "Harbor Card"), "liability");
        assert_eq!(class_of(&conn, "accounts", "Harbor LOC"), "liability");

        assert_eq!(class_of(&conn, "categories", "Client Services"), "revenue");
        assert_eq!(class_of(&conn, "categories", "Office Expense"), "expense");
        assert_eq!(class_of(&conn, "categories", "Transfer"), "expense");

        let unclassified: i64 = conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM accounts WHERE class IS NULL) \
                      + (SELECT COUNT(*) FROM categories WHERE class IS NULL)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(unclassified, 0, "nothing needs re-categorizing by hand");
    }

    #[test]
    fn v10_lands_distributions_on_equity_after_the_general_rule() {
        let (_dir, conn) = db_without_classes();
        run_migrations(&conn).unwrap();
        // The seeded row is category_type = 'expense', so the general rule
        // reaches it first and the by-name rule has to run after it.
        assert_eq!(
            class_of(&conn, "categories", "Owner Draw / Distribution"),
            "equity"
        );
    }

    #[test]
    fn v10_seeds_owner_contribution_once_on_a_business_database() {
        let (_dir, conn) = db_without_classes();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap();

        let rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM categories WHERE name = 'Owner Contribution'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rows, 1, "a replay adds no second copy");
        assert_eq!(class_of(&conn, "categories", "Owner Contribution"), "equity");
        let ctype: String = conn
            .query_row(
                "SELECT category_type FROM categories WHERE name = 'Owner Contribution'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ctype, "income", "money coming into the business");
    }

    #[test]
    fn v10_seeds_no_owner_contribution_on_a_personal_database() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("personal.db")).unwrap();
        crate::db::init_db_with_profile(&conn, crate::db::Profile::Personal).unwrap();
        conn.execute_batch(
            "ALTER TABLE accounts DROP COLUMN class;
             ALTER TABLE categories DROP COLUMN class;",
        )
        .unwrap();
        set_metadata(&conn, "schema_version", "9").unwrap();

        run_migrations(&conn).unwrap();

        let rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM categories WHERE name = 'Owner Contribution'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rows, 0);
    }

    #[test]
    fn a_fresh_install_and_a_migrated_one_have_the_same_class_columns() {
        let (_dir, fresh) = test_db();
        let (_dir2, migrated) = db_without_classes();
        run_migrations(&migrated).unwrap();

        for table in ["accounts", "categories"] {
            let column = |conn: &Connection| -> (String, i64, Option<String>) {
                conn.query_row(
                    &format!(
                        "SELECT type, \"notnull\", dflt_value \
                         FROM pragma_table_info('{table}') WHERE name = 'class'"
                    ),
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap_or_else(|e| panic!("{table}: {e}"))
            };
            assert_eq!(column(&fresh), column(&migrated), "{table}.class");
        }
    }
```

- [ ] **Step 2: Run them and watch them fail**

```bash
cargo test -p nigel-core -- --test-threads=1 class
```
Expected: FAIL — `cannot find type 'AccountClass' in this scope`, and the migration tests fail to compile for the same reason.

- [ ] **Step 3: Add the enum and the two mappings**

In `crates/nigel-core/src/db.rs`, directly below the `Profile` impl (after `get_profile`), add:

```rust
/// Where a thing sits in the accounting structure: the five classes every
/// account and every category carries.
///
/// Stored as text with a `CHECK` constraint, read through this type. Serde
/// carries it over the wire, which is what makes an unknown class a `400` from
/// the extractor instead of a value the data layer has to check for.
///
/// Nothing matching on this may use a catch-all arm. A sixth class must be a
/// compile error at every site that decides what a class means, because an
/// unhandled class falling into an `else` and being counted as an expense is
/// how owner distributions came to be reported as deductions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountClass {
    Asset,
    Liability,
    Equity,
    Revenue,
    Expense,
}

impl AccountClass {
    /// Every class, in the order the UIs offer them.
    pub const ALL: [AccountClass; 5] = [
        AccountClass::Asset,
        AccountClass::Liability,
        AccountClass::Equity,
        AccountClass::Revenue,
        AccountClass::Expense,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            AccountClass::Asset => "asset",
            AccountClass::Liability => "liability",
            AccountClass::Equity => "equity",
            AccountClass::Revenue => "revenue",
            AccountClass::Expense => "expense",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "asset" => Some(AccountClass::Asset),
            "liability" => Some(AccountClass::Liability),
            "equity" => Some(AccountClass::Equity),
            "revenue" => Some(AccountClass::Revenue),
            "expense" => Some(AccountClass::Expense),
            _ => None,
        }
    }

    /// A class read out of the database. The `CHECK` constraint should have
    /// refused anything else, so a value this cannot read is a damaged
    /// database and is reported as one rather than defaulted.
    pub fn from_db(value: &str) -> Result<Self> {
        Self::parse(value).ok_or_else(|| {
            crate::error::NigelError::Invalid(format!(
                "Unknown account class in the database: {value}"
            ))
        })
    }
}

/// The class an account type sits in.
///
/// `asset` is the answer for anything outside the known set: a database
/// written by another tool still has to land somewhere, and an asset is the
/// reading that can never be counted as a deduction.
pub fn class_for_account_type(account_type: &str) -> AccountClass {
    match account_type {
        "credit_card" | "line_of_credit" => AccountClass::Liability,
        _ => AccountClass::Asset,
    }
}

/// The class a category type sits in. `category_type` is the user-facing
/// income/expense split the UI organizes by; this is the structure under it.
pub fn class_for_category_type(category_type: &str) -> AccountClass {
    match category_type {
        "income" => AccountClass::Revenue,
        _ => AccountClass::Expense,
    }
}

/// The equity category for money the owner puts into the business. Seeded on
/// business-profile databases beside `Owner Draw / Distribution`.
pub const OWNER_CONTRIBUTION: &str = "Owner Contribution";
```

- [ ] **Step 4: Add the columns to `SCHEMA`**

In `crates/nigel-core/src/db.rs`, in `SCHEMA`, the `accounts` table becomes:

```
CREATE TABLE IF NOT EXISTS accounts (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    account_type TEXT NOT NULL,
    class TEXT NOT NULL DEFAULT 'asset'
        CHECK (class IN ('asset', 'liability', 'equity', 'revenue', 'expense')),
    institution TEXT,
    last_four TEXT,
    created_at TEXT DEFAULT (datetime('now'))
);
```

and the `categories` table:

```
CREATE TABLE IF NOT EXISTS categories (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    parent_id INTEGER,
    category_type TEXT NOT NULL,
    class TEXT NOT NULL DEFAULT 'expense'
        CHECK (class IN ('asset', 'liability', 'equity', 'revenue', 'expense')),
    tax_line TEXT,
    form_line TEXT,
    description TEXT,
    is_active INTEGER DEFAULT 1,
    FOREIGN KEY (parent_id) REFERENCES categories(id)
);
```

The `DEFAULT` is not a convenience: SQLite refuses `ALTER TABLE … ADD COLUMN … NOT NULL` without one, so the migration must carry it and the fresh-install schema has to match or the two diverge — which `a_fresh_install_and_a_migrated_one_have_the_same_class_columns` fails on. Every Rust write path names the class explicitly; the default is reachable only from hand-written SQL.

- [ ] **Step 5: Add classes to the seeded charts**

In `crates/nigel-core/src/db.rs`, widen `CategoryDef` to carry the class:

```rust
type CategoryDef = (
    &'static str,
    Option<i64>,
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
    &'static str,
    AccountClass,
);
```

Append the class to every entry in `BUSINESS_CATEGORIES` and `PERSONAL_CATEGORIES`. `income` rows take `AccountClass::Revenue`, `expense` rows take `AccountClass::Expense`, and `Owner Draw / Distribution` takes `AccountClass::Equity`. Add the contribution category to `BUSINESS_CATEGORIES`, directly above `Owner Draw / Distribution`:

```rust
    (
        OWNER_CONTRIBUTION,
        None,
        "income",
        Some("Not taxable"),
        None,
        "Money the owner puts into the business",
        AccountClass::Equity,
    ),
```

Then the seeding insert in `init_db_with_profile`:

```rust
            tx.execute(
                "INSERT INTO categories (name, parent_id, category_type, tax_line, form_line, description, class) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![cat.0, cat.1, cat.2, cat.3, cat.4, cat.5, cat.6.as_str()],
            )?;
```

- [ ] **Step 6: Write migration v10**

In `crates/nigel-core/src/migrations.rs`, append to `MIGRATIONS` after v9:

```rust
    Migration {
        version: 10,
        description: "classify accounts and categories as asset/liability/equity/revenue/expense",
        up: classify_accounts_and_categories,
    },
```

and add, beside `seed_payment_instructions`:

```rust
/// The `CHECK` every class column carries, written once so the two tables and
/// the fresh-install schema cannot drift apart.
const CLASS_CHECK: &str =
    "CHECK (class IN ('asset', 'liability', 'equity', 'revenue', 'expense'))";

/// Add the column when it is not already there. SQLite has no
/// `ADD COLUMN IF NOT EXISTS`, and a fresh database created from `SCHEMA` runs
/// every migration too, so the probe is what makes the two paths agree.
fn add_class_column(conn: &Connection, table: &str, default: AccountClass) -> Result<()> {
    let has_column: bool = conn.query_row(
        &format!("SELECT COUNT(*) > 0 FROM pragma_table_info('{table}') WHERE name = 'class'"),
        [],
        |r| r.get(0),
    )?;
    if !has_column {
        conn.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN class TEXT NOT NULL DEFAULT '{}' {CLASS_CHECK}",
            default.as_str()
        ))?;
    }
    Ok(())
}

/// Give every existing account and category its accounting class.
///
/// The order is what makes it correct. The general rule runs first and lands
/// `Owner Draw / Distribution` on `expense` along with every other
/// `category_type = 'expense'` row; the by-name rule then moves it to `equity`.
/// Reversing the two would classify distributions as deductions, which is the
/// defect this migration exists to end.
///
/// `Owner Contribution` is seeded only where the business chart is, and only
/// when it is absent, so a replay and an installation that already added one
/// by hand both come out with exactly one.
fn classify_accounts_and_categories(conn: &Connection) -> Result<()> {
    add_class_column(conn, "accounts", AccountClass::Asset)?;
    add_class_column(conn, "categories", AccountClass::Expense)?;

    conn.execute_batch(
        "UPDATE accounts SET class = 'asset'
             WHERE account_type NOT IN ('credit_card', 'line_of_credit');
         UPDATE accounts SET class = 'liability'
             WHERE account_type IN ('credit_card', 'line_of_credit');
         UPDATE categories SET class = 'revenue' WHERE category_type = 'income';
         UPDATE categories SET class = 'expense' WHERE category_type = 'expense';
         UPDATE categories SET class = 'equity' WHERE name = 'Owner Draw / Distribution';",
    )?;

    if crate::db::get_profile(conn) == Profile::Business {
        conn.execute(
            "INSERT INTO categories (name, category_type, tax_line, description, class)
             SELECT ?1, 'income', 'Not taxable',
                    'Money the owner puts into the business', 'equity'
             WHERE NOT EXISTS (SELECT 1 FROM categories WHERE name = ?1)",
            [crate::db::OWNER_CONTRIBUTION],
        )?;
    }
    Ok(())
}
```

Add the imports at the top of `migrations.rs`:

```rust
use crate::db::{set_metadata, AccountClass, Profile};
```
(replacing the existing `use crate::db::set_metadata;`).

- [ ] **Step 7: Run the tests and watch them pass**

```bash
cargo test -p nigel-core -- --test-threads=1 class
cargo test -p nigel-core -- --test-threads=1 v10
cargo test -p nigel-core -- --test-threads=1 migrat
```
Expected: PASS, including `test_fresh_install_at_latest_version` — `LATEST_VERSION` is now 10.

- [ ] **Step 8: Name the class in the two production paths that write rows by hand**

`nigel demo` and the fixture capture both insert with raw SQL and must not lean on the column default.

In `crates/nigel/src/cli/demo.rs:486`:

```rust
    conn.execute(
        "INSERT INTO accounts (name, account_type, institution, class) VALUES (?1, 'checking', 'Bank of America', 'asset')",
        [ACCOUNT_NAME],
    )?;
```

In `crates/nigel/src/cli/fixture_capture.rs:88` and `:96`:

```rust
    conn.execute(
        "INSERT INTO categories (name, category_type, tax_line, form_line, class) \
         VALUES ('Studio Sundries', 'expense', NULL, NULL, 'expense')",
        [],
    )
    .expect("unmapped expense category");
```

```rust
    conn.execute(
        "INSERT INTO categories (name, category_type, tax_line, form_line, class) \
         VALUES ('Workshop Fees', 'income', NULL, NULL, 'revenue')",
        [],
    )
    .expect("unmapped income category");
```

- [ ] **Step 9: Full verification**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test -- --test-threads=1
cargo test -p nigel-core -- --test-threads=1
```
Expected: all green. The seeded-chart count assertion in `crates/nigel-core/src/server/routes/categories.rs` (`rows.len() >= 29`) still holds — the chart grew by one.

---

### Task 2: `natural_balance` — the sign convention, in one place

**Files:**
- Modify: `crates/nigel-core/src/reports/mod.rs`
- Test: `crates/nigel-core/src/reports/mod.rs` `mod tests`

**Interfaces:**
- Consumes: `db::AccountClass`.
- Produces:
  - `pub fn reports::natural_balance(class: AccountClass, raw_sum: f64) -> f64`
  - `AccountBalance` gains `pub class: AccountClass` and `pub natural_balance: f64` (`class` and `naturalBalance` on the wire).

`balance` and `total` keep exactly the meaning they have: the raw signed sum, which is the cash position the "Cash Position" report is named for, and a total that only adds up because every account is signed the same way. `natural_balance` is the second reading beside it — "positive means more of what this class is" — and it is the one Schedule L, the balance surfaces and anything else that has to say how much is owed will call. Nothing re-derives it from an account-type string.

The asymmetry between `Liability` and `Equity` is real and is what the doc comment has to say: a liability's register is the liability's own, so a card charge is a negative row that grows what is owed; an equity or revenue or expense amount is recorded on the *cash* side, so an owner contribution is a positive row that grows equity.

- [ ] **Step 1: Write the failing test**

Add to `crates/nigel-core/src/reports/mod.rs`, inside `mod tests`:

```rust
    #[test]
    fn natural_balance_reads_every_class_positive_when_there_is_more_of_it() {
        use crate::db::AccountClass::*;
        let table = [
            // class, raw signed sum, what the class says it means
            (Asset, 4_928.01, 4_928.01),
            (Liability, -3_184.90, 3_184.90),
            (Equity, 12_000.00, 12_000.00),
            (Revenue, 7_500.00, 7_500.00),
            (Expense, -250.00, 250.00),
        ];
        for (class, raw, expected) in table {
            assert_eq!(
                natural_balance(class, raw),
                expected,
                "{} on {raw}",
                class.as_str()
            );
        }
        assert_eq!(table.len(), crate::db::AccountClass::ALL.len());
    }

    #[test]
    fn the_balance_report_carries_each_accounts_class_and_natural_reading() {
        let (_dir, conn) = test_db();
        conn.execute_batch(
            "INSERT INTO accounts (name, account_type, class) \
                 VALUES ('Harbor Checking', 'checking', 'asset');
             INSERT INTO accounts (name, account_type, class) \
                 VALUES ('Harbor Card', 'credit_card', 'liability');",
        )
        .unwrap();
        let card: i64 = conn
            .query_row("SELECT id FROM accounts WHERE name = 'Harbor Card'", [], |r| r.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount) \
             VALUES (?1, '2025-04-02', 'GLOBEX SUPPLIES', -1200.0)",
            [card],
        )
        .unwrap();

        let report = get_balance(&conn).unwrap();
        let card_row = report
            .accounts
            .iter()
            .find(|a| a.name == "Harbor Card")
            .expect("the card");
        assert_eq!(card_row.class, crate::db::AccountClass::Liability);
        assert_eq!(card_row.balance, -1200.0, "the register keeps its signs");
        assert_eq!(card_row.natural_balance, 1200.0, "money owed reads positive");
        // The cash position is still the cash position.
        assert_eq!(report.total, -1200.0);
    }
```

- [ ] **Step 2: Run it and watch it fail**

```bash
cargo test -p nigel-core -- --test-threads=1 natural_balance
```
Expected: FAIL — `cannot find function 'natural_balance' in this scope`.

- [ ] **Step 3: Add the function**

In `crates/nigel-core/src/reports/mod.rs`, directly above the `AccountBalance` struct:

```rust
/// What "the balance" of a class means, decided once.
///
/// Transactions keep their bank-statement signs everywhere else in the app —
/// the register, the importers and the cash-position total all read the raw
/// sum, and none of them change. This is the second reading beside it: the
/// amount stated so that more of what the class is reads positive. A liability
/// with money owed reports positive; an asset with money in it reports
/// positive.
///
/// `Liability` and `Equity` differ because the register they are summed from
/// differs. A liability's rows are its own — a card charge is a negative row
/// that grows what is owed. Equity, revenue and expense are summed off the cash
/// side, where an owner contribution and a client payment are both positive
/// rows and a distribution and a software bill are both negative ones.
///
/// Everything that needs a sign calls this. Nothing re-derives one from an
/// account type or a category name.
pub fn natural_balance(class: AccountClass, raw_sum: f64) -> f64 {
    match class {
        AccountClass::Asset | AccountClass::Equity | AccountClass::Revenue => raw_sum,
        AccountClass::Liability | AccountClass::Expense => -raw_sum,
    }
}
```

Add `use crate::db::AccountClass;` to the imports at the top of `reports/mod.rs`.

- [ ] **Step 4: Carry the class into the balance report**

`AccountBalance` becomes:

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountBalance {
    pub name: String,
    pub account_type: String,
    pub class: AccountClass,
    /// The account's own register, summed with the signs it was imported with.
    pub balance: f64,
    /// The same figure through `natural_balance`: money owed reads positive.
    pub natural_balance: f64,
}
```

and `get_balance`'s query and row mapping:

```rust
    let mut stmt = conn.prepare(
        "SELECT a.id, a.name, a.account_type, a.class, COALESCE(SUM(t.amount), 0) as balance \
         FROM accounts a LEFT JOIN transactions t ON a.id = t.account_id \
         GROUP BY a.id ORDER BY a.name",
    )?;
    let accounts = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, f64>(4)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let accounts: Vec<AccountBalance> = accounts
        .into_iter()
        .map(|(name, account_type, class, balance)| {
            let class = AccountClass::from_db(&class)?;
            Ok(AccountBalance {
                name,
                account_type,
                class,
                balance,
                natural_balance: natural_balance(class, balance),
            })
        })
        .collect::<Result<Vec<_>>>()?;
```

The two-step collect is what lets `from_db`'s error out: `query_map`'s closure can only answer a `rusqlite::Error`, and a class the CHECK constraint should have refused is not one.

- [ ] **Step 5: Run the tests and watch them pass**

```bash
cargo test -p nigel-core -- --test-threads=1 natural_balance
cargo test -p nigel-core -- --test-threads=1 balance
cargo test -- --test-threads=1 balance
```
Expected: PASS. `crates/nigel-core/src/pdf.rs`, `reports/text.rs` and `crates/nigel/src/cli/report/view.rs` all read `balance` and are unaffected.

---

### Task 3: The reports audit — every report classifies from `class`

**Files:**
- Modify: `crates/nigel-core/src/reports/mod.rs` (P&L, expense breakdown, tax summary, balance's YTD net income, K-1)
- Modify: `crates/nigel/src/cli/report/view.rs` (the tax summary's row styling)
- Test: `crates/nigel-core/src/reports/mod.rs` `mod tests`

**Interfaces:**
- Consumes: `db::AccountClass`, `reports::natural_balance`.
- Produces:
  - `query_category_totals(conn, clause, params, class: AccountClass, order) -> Result<Vec<PnlItem>>`
  - `TaxItem` gains `pub class: AccountClass`; `category_type` stays as the word the "Type" column prints.
  - `K1Mapping` gains an `Equity` variant.
  - `resolve_k1_mapping(form_line: Option<&str>, class: AccountClass) -> K1Mapping` — **signature change**.

This is the task the whole change exists for. Every `c.category_type = '…'` that decides accounting meaning becomes `c.class = '…'`; the K-1's income fallback stops asking whether a category is `income` and asks whether it is `revenue`; and equity is refused a path into deductions in every one of them.

`category_type` is *not* removed from the reports — the tax summary keeps printing it, because "income" and "expense" are the words a freelancer reads and the key constraint is that classification is structure, not vocabulary. What changes is that nothing *branches* on it.

`EXCLUDE_TRANSFERS` stays exactly as it is. `Transfer` classes as `expense` under the mapping, so the `form_line = 'excluded'` predicate is still the only thing keeping a movement between own accounts out of the income statement — flagged question 1.

Two figures change on real books, both of them corrections:
- **P&L `total_expenses`** stops including owner distributions.
- **Balance `ytd_net_income`** stops including equity movements — it was summing every non-transfer transaction, distributions and contributions included, and calling the result net income.

- [ ] **Step 1: Write the failing tests**

Add to `crates/nigel-core/src/reports/mod.rs`, inside `mod tests`:

```rust
    /// One account, one client payment, one software bill, one owner draw and
    /// one owner contribution — the shape that made distributions read as
    /// deductions.
    fn db_with_equity() -> (tempfile::TempDir, Connection) {
        let (dir, conn) = test_db();
        conn.execute(
            "INSERT INTO accounts (name, account_type, class) \
             VALUES ('Cedar Checking', 'checking', 'asset')",
            [],
        )
        .unwrap();
        let account = conn.last_insert_rowid();
        let cat = |name: &str| -> i64 {
            conn.query_row("SELECT id FROM categories WHERE name = ?1", [name], |r| {
                r.get(0)
            })
            .unwrap()
        };
        let rows: [(&str, &str, f64, i64); 4] = [
            ("2025-01-10", "CEDAR SYSTEMS INVOICE", 8_000.0, cat("Client Services")),
            ("2025-02-05", "SOFTWARE RENEWAL", -300.0, cat("Software & Subscriptions")),
            ("2025-03-01", "OWNER DRAW", -2_000.0, cat("Owner Draw / Distribution")),
            ("2025-03-02", "OWNER FUNDS IN", 1_000.0, cat("Owner Contribution")),
        ];
        for (date, desc, amount, category) in rows {
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, category_id, vendor) \
                 VALUES (?1, ?2, ?3, ?4, ?5, 'Cedar Systems')",
                rusqlite::params![account, date, desc, amount, category],
            )
            .unwrap();
        }
        (dir, conn)
    }

    #[test]
    fn the_pnl_leaves_owner_equity_out_of_both_columns() {
        let (_dir, conn) = db_with_equity();
        let report = get_pnl(&conn, Some(2025), None, None, None).unwrap();

        assert_eq!(report.total_income, 8_000.0, "the contribution is not revenue");
        assert_eq!(report.total_expenses, -300.0, "the draw is not a deduction");
        assert_eq!(report.net, 7_700.0);
        assert!(!report
            .expenses
            .iter()
            .any(|item| item.name == "Owner Draw / Distribution"));
        assert!(!report
            .income
            .iter()
            .any(|item| item.name == "Owner Contribution"));
    }

    #[test]
    fn the_expense_breakdown_leaves_owner_equity_out() {
        let (_dir, conn) = db_with_equity();
        let report = get_expense_breakdown(&conn, Some(2025), None).unwrap();
        assert_eq!(report.total, -300.0);
        assert!(!report
            .categories
            .iter()
            .any(|item| item.name == "Owner Draw / Distribution"));
        // The vendor rollup shares the filter, or the draw rides in under a
        // vendor name instead of under its category.
        let cedar: f64 = report
            .top_vendors
            .iter()
            .filter(|v| v.vendor == "Cedar Systems")
            .map(|v| v.total)
            .sum();
        assert_eq!(cedar, -300.0);
    }

    #[test]
    fn ytd_net_income_is_revenue_and_expense_only() {
        let (_dir, conn) = test_db();
        let this_year = Datelike::year(&chrono::Local::now());
        conn.execute(
            "INSERT INTO accounts (name, account_type, class) \
             VALUES ('Cedar Checking', 'checking', 'asset')",
            [],
        )
        .unwrap();
        let account = conn.last_insert_rowid();
        let cat = |name: &str| -> i64 {
            conn.query_row("SELECT id FROM categories WHERE name = ?1", [name], |r| {
                r.get(0)
            })
            .unwrap()
        };
        for (day, amount, category) in [
            ("01-10", 8_000.0, cat("Client Services")),
            ("02-05", -300.0, cat("Software & Subscriptions")),
            ("03-01", -2_000.0, cat("Owner Draw / Distribution")),
        ] {
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, category_id) \
                 VALUES (?1, ?2, 'x', ?3, ?4)",
                rusqlite::params![account, format!("{this_year}-{day}"), amount, category],
            )
            .unwrap();
        }

        let report = get_balance(&conn).unwrap();
        assert_eq!(report.ytd_net_income, 7_700.0, "a draw is not a loss");
        // The cash position still counts every dollar that moved.
        assert_eq!(report.total, 5_700.0);
    }

    #[test]
    fn the_tax_summary_carries_each_lines_class_and_lists_equity_after_revenue() {
        let (_dir, conn) = db_with_equity();
        let report = get_tax_summary(&conn, Some(2025)).unwrap();

        let class_of = |name: &str| {
            report
                .line_items
                .iter()
                .find(|i| i.name == name)
                .unwrap_or_else(|| panic!("{name} missing"))
                .class
        };
        assert_eq!(class_of("Client Services"), AccountClass::Revenue);
        assert_eq!(
            class_of("Owner Draw / Distribution"),
            AccountClass::Equity
        );
        assert_eq!(class_of("Software & Subscriptions"), AccountClass::Expense);

        let order: Vec<AccountClass> = report.line_items.iter().map(|i| i.class).collect();
        let first_expense = order
            .iter()
            .position(|c| *c == AccountClass::Expense)
            .unwrap();
        let last_revenue = order
            .iter()
            .rposition(|c| *c == AccountClass::Revenue)
            .unwrap();
        assert!(last_revenue < first_expense, "revenue first: {order:?}");
        // The user-facing word is untouched.
        assert_eq!(
            report
                .line_items
                .iter()
                .find(|i| i.name == "Client Services")
                .unwrap()
                .category_type,
            "income"
        );
    }

    #[test]
    fn the_k1_routes_equity_to_schedule_k_and_never_to_deductions() {
        let (_dir, conn) = db_with_equity();
        // An equity category a user mapped to a deduction line by hand: the
        // class has to outrank the form line, or the old defect comes back
        // through the chart of accounts.
        let stray = k1_cat(&conn, "Owner Perks", "expense", Some("1120S-19"));
        conn.execute(
            "UPDATE categories SET class = 'equity' WHERE id = ?1",
            [stray],
        )
        .unwrap();
        let account: i64 = conn
            .query_row("SELECT id FROM accounts LIMIT 1", [], |r| r.get(0))
            .unwrap();
        k1_txn(&conn, account, "2025-04-01", -500.0, stray);

        let r = get_k1_prep(&conn, Some(2025)).unwrap();

        assert_eq!(r.gross_receipts, 8_000.0);
        assert_eq!(r.total_deductions, 300.0, "software only");
        assert!(r
            .deduction_lines
            .iter()
            .all(|d| d.category_name != "Owner Draw / Distribution"
                && d.category_name != "Owner Perks"));
        assert!(r
            .other_deductions
            .iter()
            .all(|d| d.category_name != "Owner Perks"));
        // Money out to the owner is a distribution wherever it was mapped.
        assert_eq!(r.validation.distributions, 2_500.0);
        // Money in from the owner is not.
        assert!(r
            .schedule_k_items
            .iter()
            .any(|k| k.category_name == "Owner Contribution"));
        assert_eq!(r.ordinary_business_income, 7_700.0);
    }

    #[test]
    fn k1_mapping_reads_the_class_before_the_form_line() {
        use K1Mapping::*;
        assert_eq!(
            resolve_k1_mapping(Some("1120S-19"), AccountClass::Equity),
            Equity
        );
        assert_eq!(resolve_k1_mapping(None, AccountClass::Equity), Equity);
        assert_eq!(
            resolve_k1_mapping(Some("excluded"), AccountClass::Expense),
            Excluded
        );
        assert_eq!(
            resolve_k1_mapping(None, AccountClass::Revenue),
            AutoGrossReceipts
        );
        assert_eq!(resolve_k1_mapping(None, AccountClass::Expense), Unmapped);
        assert_eq!(
            resolve_k1_mapping(Some("1120S-2"), AccountClass::Expense),
            Explicit("1120S-2".to_string())
        );
        // A category on a balance-sheet class is not income-statement activity.
        assert_eq!(
            resolve_k1_mapping(Some("1120S-19"), AccountClass::Asset),
            Excluded
        );
        assert_eq!(resolve_k1_mapping(None, AccountClass::Liability), Excluded);
    }

    /// AC #7. A category on each class in turn, all four carrying the same
    /// amount: the expense totals may move for `expense` and for nothing else.
    /// This is the test a sixth class has to keep passing, and the reason no
    /// class match may carry a catch-all arm.
    #[test]
    fn no_class_but_expense_can_reach_the_expense_totals() {
        for class in AccountClass::ALL {
            let (_dir, conn) = test_db();
            conn.execute(
                "INSERT INTO accounts (name, account_type, class) \
                 VALUES ('Juniper Checking', 'checking', 'asset')",
                [],
            )
            .unwrap();
            let account = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO categories (name, category_type, class) \
                 VALUES ('Probe', 'expense', ?1)",
                [class.as_str()],
            )
            .unwrap();
            let probe = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, category_id) \
                 VALUES (?1, '2025-05-01', 'PROBE', -1000.0, ?2)",
                rusqlite::params![account, probe],
            )
            .unwrap();

            let expected = if class == AccountClass::Expense {
                -1000.0
            } else {
                0.0
            };
            assert_eq!(
                get_pnl(&conn, Some(2025), None, None, None).unwrap().total_expenses,
                expected,
                "P&L expenses absorbed a {} category",
                class.as_str()
            );
            assert_eq!(
                get_expense_breakdown(&conn, Some(2025), None).unwrap().total,
                expected,
                "the expense breakdown absorbed a {} category",
                class.as_str()
            );
            let deductions = get_k1_prep(&conn, Some(2025)).unwrap().total_deductions;
            assert_eq!(
                deductions, 0.0,
                "the K-1 deducted a {} category with no form line",
                class.as_str()
            );
        }
    }
```

- [ ] **Step 2: Run them and watch them fail**

```bash
cargo test -p nigel-core -- --test-threads=1 equity
cargo test -p nigel-core -- --test-threads=1 no_class_but_expense
```
Expected: FAIL — `the_pnl_leaves_owner_equity_out_of_both_columns` reports `total_expenses: -2300.0`, `ytd_net_income_is_revenue_and_expense_only` reports `5700.0`, and the `TaxItem`/`resolve_k1_mapping` tests do not compile.

- [ ] **Step 3: P&L and the expense breakdown classify from `class`**

In `crates/nigel-core/src/reports/mod.rs`, `get_pnl`'s two calls:

```rust
    let income = query_category_totals(conn, &clause, &params, AccountClass::Revenue, "total DESC")?;
    let expenses = query_category_totals(conn, &clause, &params, AccountClass::Expense, "total ASC")?;
```

and `query_category_totals`:

```rust
fn query_category_totals(
    conn: &Connection,
    clause: &str,
    params: &[String],
    class: AccountClass,
    order: &str,
) -> Result<Vec<PnlItem>> {
    let class = class.as_str();
    let sql = format!(
        "SELECT c.name, SUM(t.amount) as total \
         FROM transactions t JOIN categories c ON t.category_id = c.id \
         WHERE {clause} AND c.class = '{class}' AND {EXCLUDE_TRANSFERS} \
         GROUP BY c.name ORDER BY {order}"
    );
```

In `get_expense_breakdown`, both interpolated predicates become `AND c.class = 'expense'`:

```rust
    let sql = format!(
        "SELECT c.name, SUM(t.amount) as total, COUNT(*) as count \
         FROM transactions t JOIN categories c ON t.category_id = c.id \
         WHERE {clause} AND c.class = 'expense' AND {EXCLUDE_TRANSFERS} \
         GROUP BY c.name ORDER BY total ASC"
    );
```

```rust
    let vendor_sql = format!(
        "SELECT t.vendor, SUM(t.amount) as total, COUNT(*) as count \
         FROM transactions t JOIN categories c ON t.category_id = c.id \
         WHERE {clause} AND c.class = 'expense' AND t.vendor IS NOT NULL \
           AND {EXCLUDE_TRANSFERS} \
         GROUP BY t.vendor ORDER BY total ASC LIMIT 10"
    );
```

- [ ] **Step 4: The tax summary carries the class and orders by it**

`TaxItem` becomes:

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxItem {
    pub name: String,
    pub tax_line: Option<String>,
    /// The word the "Type" column prints — income or expense, unchanged.
    pub category_type: String,
    /// Where the line sits in the accounting structure.
    pub class: AccountClass,
    pub total: f64,
}
```

and `get_tax_summary`:

```rust
/// Where a class sits in the tax summary's listing: what was earned, then what
/// the owner took out, then what was spent, then the balance-sheet classes a
/// category should not be on at all — visible at the bottom rather than hidden.
fn tax_summary_rank(class: AccountClass) -> u8 {
    match class {
        AccountClass::Revenue => 0,
        AccountClass::Equity => 1,
        AccountClass::Expense => 2,
        AccountClass::Asset => 3,
        AccountClass::Liability => 4,
    }
}

pub fn get_tax_summary(conn: &Connection, year: Option<i32>) -> Result<TaxSummary> {
    let (clause, params) = date_filter(year, None, None, None)?;

    let sql = format!(
        "SELECT c.name, c.tax_line, c.category_type, c.class, SUM(t.amount) as total \
         FROM transactions t JOIN categories c ON t.category_id = c.id \
         WHERE {clause} \
         GROUP BY c.name, c.tax_line, c.category_type, c.class \
         ORDER BY c.tax_line"
    );
    let mut stmt = conn.prepare(&sql)?;
    let param_values = to_sql_params(&params);
    let raw: Vec<(String, Option<String>, String, String, f64)> = stmt
        .query_map(param_values.as_slice(), |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut items: Vec<TaxItem> = raw
        .into_iter()
        .map(|(name, tax_line, category_type, class, total)| {
            Ok(TaxItem {
                name,
                tax_line,
                category_type,
                class: AccountClass::from_db(&class)?,
                total,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    // Ranked here rather than in SQL: an ordering built from an exhaustive
    // match is one the compiler rechecks when a class is added.
    items.sort_by_key(|item| (tax_summary_rank(item.class), item.tax_line.clone()));

    Ok(TaxSummary { line_items: items })
}
```

- [ ] **Step 5: YTD net income is revenue and expense only**

In `get_balance`:

```rust
    let current_year = chrono::Local::now().year();
    let ytd_net_income: f64 = conn.query_row(
        &format!(
            "SELECT COALESCE(SUM(t.amount), 0) as net \
             FROM transactions t JOIN categories c ON t.category_id = c.id \
             WHERE t.date LIKE ?1 AND c.class IN ('revenue', 'expense') AND {EXCLUDE_TRANSFERS}"
        ),
        [format!("{current_year}%")],
        |row| row.get(0),
    )?;
```

The `LEFT JOIN` becomes a `JOIN`: an uncategorized transaction has no class, and a figure called net income cannot be built from rows nobody has said are income or spending.

- [ ] **Step 6: The K-1 reads the class before the form line**

`K1Mapping` gains a variant:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum K1Mapping {
    Excluded,
    Explicit(String),
    AutoGrossReceipts,
    Unmapped,
    /// Owner equity: a Schedule K item, never a deduction, whatever form line
    /// the category carries.
    Equity,
}
```

and `resolve_k1_mapping`:

```rust
/// Where a category's activity lands on the worksheet.
///
/// The class is read first and it is final. A form line is a mapping the
/// operator can edit; the class is what the row *is*, and an equity row
/// carrying a deduction form line is a chart-of-accounts mistake rather than a
/// deduction. Asset and liability categories are not income-statement activity
/// at all.
pub fn resolve_k1_mapping(form_line: Option<&str>, class: AccountClass) -> K1Mapping {
    match class {
        AccountClass::Equity => return K1Mapping::Equity,
        AccountClass::Asset | AccountClass::Liability => return K1Mapping::Excluded,
        AccountClass::Revenue | AccountClass::Expense => {}
    }
    match form_line {
        Some("excluded") => K1Mapping::Excluded,
        Some(fl) => K1Mapping::Explicit(fl.to_string()),
        None if class == AccountClass::Revenue => K1Mapping::AutoGrossReceipts,
        None => K1Mapping::Unmapped,
    }
}
```

In `get_k1_prep`, the query and the loop:

```rust
    let sql = format!(
        "SELECT c.form_line, c.name, c.category_type, c.class, SUM(t.amount) as total \
         FROM transactions t JOIN categories c ON t.category_id = c.id \
         WHERE {clause} \
         GROUP BY c.form_line, c.name, c.category_type, c.class ORDER BY c.form_line"
    );
    let mut stmt = conn.prepare(&sql)?;
    let param_values = to_sql_params(&params);
    let rows: Vec<(Option<String>, String, String, f64)> = stmt
        .query_map(param_values.as_slice(), |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(3)?, row.get(4)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
```

`category_type` leaves the tuple: nothing in this report reads it any more. The loop header and the two new arms:

```rust
    for (form_line, name, class, total) in &rows {
        let class = AccountClass::from_db(class)?;
        let mapping = resolve_k1_mapping(form_line.as_deref(), class);
        let line = match mapping {
            K1Mapping::Excluded => continue,
            K1Mapping::Equity => {
                // Money out to the owner is a distribution; money in is a
                // contribution and reduces nothing.
                if *total < 0.0 {
                    distributions += -total;
                }
                schedule_k_items.push(K1LineItem {
                    form_line: form_line.clone().unwrap_or_else(|| "\u{2014}".to_string()),
                    category_name: name.clone(),
                    total: *total,
                });
                continue;
            }
            K1Mapping::AutoGrossReceipts => {
```

and the `K-` arm loses its by-name distribution special case, which the `Equity` arm now owns:

```rust
            fl if fl.starts_with("K-") => {
                schedule_k_items.push(K1LineItem {
                    form_line: line.clone(),
                    category_name: name.clone(),
                    total: *total,
                });
            }
```

- [ ] **Step 7: The tax summary's TUI styling reads the class**

In `crates/nigel/src/cli/report/view.rs`, in `build_tax`, replace the string comparison:

```rust
        let style = match item.class {
            AccountClass::Revenue | AccountClass::Asset => AMOUNT_POS_STYLE,
            AccountClass::Expense | AccountClass::Liability | AccountClass::Equity => {
                AMOUNT_NEG_STYLE
            }
        };
```

with `use nigel_core::db::AccountClass;` added to the file's imports. The printed cell still reads `item.category_type` — the words on screen do not change.

- [ ] **Step 8: Run the tests and watch them pass**

```bash
cargo test -p nigel-core -- --test-threads=1 equity
cargo test -p nigel-core -- --test-threads=1 no_class_but_expense
cargo test -p nigel-core -- --test-threads=1 k1
cargo test -p nigel-core -- --test-threads=1 tax
cargo test -- --test-threads=1
```
Expected: all PASS, **and the existing report suite passes unchanged** — that is the regression assertion the spec asks for. `pnl_excludes_transfer_categories`, `expense_breakdown_excludes_transfer_categories` and `cashflow_excludes_transfer_categories` still hold (`Transfer` is an `expense` class and `EXCLUDE_TRANSFERS` is what keeps it out), the `seeded_db` fixture's P&L and tax-summary figures are the figures they were, and the K-1 fixtures in `test_k1_cogs_and_gross_profit` and `test_k1_custom_chart_income_falls_back_and_unmapped_surfaces` are untouched. The classification is structure, not new math: the only figures that move are the ones distributions were being counted into. If any other total changes, stop — a mapping is wrong, not the test.

- [ ] **Step 9: Confirm there is no catch-all on any class match**

```bash
rg -n 'match .*class|match self' crates/nigel-core/src crates/nigel/src | rg -v '^\s*//'
rg -n --multiline 'AccountClass::[A-Za-z]+ =>[\s\S]{0,400}?_ =>' crates/nigel-core/src crates/nigel/src
```
Expected: the second search prints nothing. Every `AccountClass` match names all five variants, so a sixth is a compile error at every site.

- [ ] **Step 10: Full verification**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
cargo test --no-default-features --features serve -- --test-threads=1
cargo test -p nigel-core -- --test-threads=1
```
Expected: all green. `crates/nigel-core/src/pdf.rs`'s `render_tax` reads `category_type` and is unaffected.

---

### Task 4: The data layer and the API surface

**Files:**
- Modify: `crates/nigel-core/src/models.rs` (`Account`)
- Modify: `crates/nigel-core/src/categories.rs` (`CategoryRow`, `add_category`, `update_category`, `rename_category`, `validate_fields`)
- Modify: `crates/nigel-core/src/accounts.rs` (`list_accounts`, `get_account`, `add_account`, new `update_account`, `rename_account`)
- Modify: `crates/nigel-core/src/server/routes/categories.rs`, `crates/nigel-core/src/server/routes/accounts.rs`
- Test: the `mod tests` in `routes/categories.rs` and `routes/accounts.rs`

**Interfaces:**
- Consumes: `db::AccountClass`, `db::class_for_account_type`, `db::class_for_category_type`.
- Produces:
  - `Account` gains `pub class: AccountClass`; `CategoryRow` gains `pub class: AccountClass`. Both serialize as `class`.
  - `accounts::add_account(conn, name, account_type, class: Option<AccountClass>, institution, last_four) -> Result<i64>`
  - `accounts::update_account(conn, id, name: Option<&str>, class: Option<AccountClass>) -> Result<()>`; `rename_account` stays, delegating.
  - `categories::add_category(conn, name, category_type, class: Option<AccountClass>, tax_line, form_line) -> Result<i64>`
  - `categories::update_category(conn, id, name, category_type, class: AccountClass, tax_line, form_line) -> Result<()>`
  - `POST /api/accounts` accepts `class?`; `PATCH /api/accounts/:id` becomes a partial update taking `name?` and `class?`.
  - `POST /api/categories` accepts `class?`; `PATCH /api/categories/:id` accepts `class?`.

`class` arrives as `Option<AccountClass>` on the two creators. `None` means "derive it", which is what keeps every existing caller — the CLI without a `--class`, an HTTP client that has never heard of the field, the TUI's add form before it grows a selector — working byte for byte. `Some` is an explicit choice and is taken as given: a savings-style checking account the operator wants classed as an asset and a `payroll` account they want classed as a liability are both legitimate, and the derivation is a default rather than a rule.

Serde does the validation. `NewCategory.class: Option<AccountClass>` means `"class": "contra-asset"` is a `400` out of `ApiJson` with no hand-written checker, and `CategoryPatch.class: Option<AccountClass>` means the same on an edit. That is TASK-60's shape applied to the field this task adds.

- [ ] **Step 1: Write the failing tests**

Add to `crates/nigel-core/src/server/routes/categories.rs`, inside `mod tests`:

```rust
    #[tokio::test]
    async fn a_category_defaults_its_class_from_its_type_and_accepts_an_explicit_one() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        // No class named: the type decides, so a client that predates the
        // field keeps working.
        let (status, derived) = post_json(
            &app,
            "/api/categories",
            &token,
            &serde_json::json!({ "name": "Workshop Fees", "categoryType": "income" }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{derived}");
        assert_eq!(derived["class"], "revenue");

        // Named explicitly: taken as given.
        let (status, chosen) = post_json(
            &app,
            "/api/categories",
            &token,
            &serde_json::json!({
                "name": "Owner Loan Repayment",
                "categoryType": "expense",
                "class": "liability",
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{chosen}");
        assert_eq!(chosen["class"], "liability");

        // And it is patchable on its own.
        let id = chosen["id"].as_i64().unwrap();
        let (status, patched) = patch_json(
            &app,
            &format!("/api/categories/{id}"),
            &token,
            &serde_json::json!({ "class": "equity" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{patched}");
        assert_eq!(patched["class"], "equity");
        assert_eq!(patched["categoryType"], "expense", "the word is untouched");
        assert_eq!(patched["name"], "Owner Loan Repayment");
    }

    #[tokio::test]
    async fn a_class_outside_the_set_is_a_bad_request() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = post_json(
            &app,
            "/api/categories",
            &token,
            &serde_json::json!({
                "name": "Nope",
                "categoryType": "expense",
                "class": "contra-asset",
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["code"], "bad_request");
    }
```

and extend the existing list assertion in `categories_list_matches_the_data_layer_and_hides_inactive_rows`:

```rust
        for key in ["categoryType", "taxLine", "formLine", "class"] {
            assert!(rows[0].get(key).is_some(), "missing {key}");
        }
```

Add to `crates/nigel-core/src/server/routes/accounts.rs`, inside `mod tests`:

```rust
    #[tokio::test]
    async fn an_account_defaults_its_class_from_its_type_and_can_be_reclassified() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, card) = post_json(
            &app,
            "/api/accounts",
            &token,
            &serde_json::json!({ "name": "Globex Card", "accountType": "credit_card" }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{card}");
        assert_eq!(card["class"], "liability");

        let (status, checking) = post_json(
            &app,
            "/api/accounts",
            &token,
            &serde_json::json!({ "name": "Globex Checking", "accountType": "checking" }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{checking}");
        assert_eq!(checking["class"], "asset");

        // Name and class are each patchable alone; neither blanks the other.
        let id = checking["id"].as_i64().unwrap();
        let (status, reclassified) = patch_json(
            &app,
            &format!("/api/accounts/{id}"),
            &token,
            &serde_json::json!({ "class": "liability" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{reclassified}");
        assert_eq!(reclassified["class"], "liability");
        assert_eq!(reclassified["name"], "Globex Checking");
        assert_eq!(reclassified["accountType"], "checking");

        let (status, renamed) = patch_json(
            &app,
            &format!("/api/accounts/{id}"),
            &token,
            &serde_json::json!({ "name": "Globex Operating" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{renamed}");
        assert_eq!(renamed["name"], "Globex Operating");
        assert_eq!(renamed["class"], "liability");
    }

    #[tokio::test]
    async fn an_empty_account_patch_and_an_unknown_class_are_both_refused() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let id = ok_json(&app, "/api/accounts", &token).await[0]["id"]
            .as_i64()
            .unwrap();

        let (status, body) = patch_json(
            &app,
            &format!("/api/accounts/{id}"),
            &token,
            &serde_json::json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

        let (status, body) = patch_json(
            &app,
            &format!("/api/accounts/{id}"),
            &token,
            &serde_json::json!({ "class": "contra-asset" }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
```

and extend the list assertion in `accounts_list_matches_the_data_layer`:

```rust
        for key in ["accountType", "lastFour", "class"] {
            assert!(rows[0].get(key).is_some(), "missing {key} in {rows:?}");
        }
```

- [ ] **Step 2: Run them and watch them fail**

```bash
cargo test -p nigel-core -- --test-threads=1 class
```
Expected: FAIL — `assertion failed: rows[0].get("class").is_some()` and the new tests do not compile.

- [ ] **Step 3: Carry the class on the two row types**

`crates/nigel-core/src/models.rs`:

```rust
pub struct Account {
    pub id: i64,
    pub name: String,
    pub account_type: String,
    pub class: AccountClass,
    pub institution: Option<String>,
    pub last_four: Option<String>,
}
```
with `use crate::db::AccountClass;` at the top.

`crates/nigel-core/src/categories.rs`:

```rust
pub struct CategoryRow {
    pub id: i64,
    pub name: String,
    pub category_type: String,
    pub class: AccountClass,
    pub tax_line: Option<String>,
    pub form_line: Option<String>,
}
```

- [ ] **Step 4: The accounts data layer**

In `crates/nigel-core/src/accounts.rs`, both readers select and parse the column — `list_accounts`:

```rust
pub fn list_accounts(conn: &Connection) -> Result<Vec<Account>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, account_type, class, institution, last_four FROM accounts ORDER BY name",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(|(id, name, account_type, class, institution, last_four)| {
            Ok(Account {
                id,
                name,
                account_type,
                class: AccountClass::from_db(&class)?,
                institution,
                last_four,
            })
        })
        .collect()
}
```

`get_account` takes the same `SELECT id, name, account_type, class, institution, last_four` and the same `AccountClass::from_db` on the way out, keeping its `QueryReturnedNoRows` → `NotFound` mapping.

`add_account` gains the parameter and writes the column:

```rust
/// Insert an account and return its id.
///
/// `class` absent means the account type decides. An explicit one is taken as
/// given: the derivation is a default, not a rule.
pub fn add_account(
    conn: &Connection,
    name: &str,
    account_type: &str,
    class: Option<AccountClass>,
    institution: Option<&str>,
    last_four: Option<&str>,
) -> Result<i64> {
    if name.trim().is_empty() {
        return Err(NigelError::Invalid("Name is required".into()));
    }
    if !ACCOUNT_TYPES.contains(&account_type) {
        return Err(NigelError::Invalid(format!(
            "Invalid account type: {account_type} (must be one of: {})",
            ACCOUNT_TYPES.join(", ")
        )));
    }
    let class = class.unwrap_or_else(|| crate::db::class_for_account_type(account_type));
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE name = ?1)",
        [name],
        |row| row.get(0),
    )?;
    if exists {
        return Err(NigelError::DuplicateName {
            kind: "Account",
            name: name.to_string(),
        });
    }
    conn.execute(
        "INSERT INTO accounts (name, account_type, class, institution, last_four) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![name, account_type, class.as_str(), institution, last_four],
    )?;
    Ok(conn.last_insert_rowid())
}
```

and the edit becomes a partial update with `rename_account` on top of it:

```rust
/// Change an account's name, its class, or both. `None` leaves a field alone.
pub fn update_account(
    conn: &Connection,
    id: i64,
    name: Option<&str>,
    class: Option<AccountClass>,
) -> Result<()> {
    if let Some(name) = name {
        if name.trim().is_empty() {
            return Err(NigelError::Invalid("Name is required".into()));
        }
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM accounts WHERE name = ?1 AND id != ?2)",
            rusqlite::params![name, id],
            |row| row.get(0),
        )?;
        if exists {
            return Err(NigelError::DuplicateName {
                kind: "Account",
                name: name.to_string(),
            });
        }
    }
    let updated = conn.execute(
        "UPDATE accounts SET name = COALESCE(?1, name), class = COALESCE(?2, class) WHERE id = ?3",
        rusqlite::params![name, class.map(|c| c.as_str()), id],
    )?;
    if updated == 0 {
        return Err(NigelError::NotFound(format!("Account not found: id {id}")));
    }
    Ok(())
}

pub fn rename_account(conn: &Connection, id: i64, new_name: &str) -> Result<()> {
    update_account(conn, id, Some(new_name), None)
}
```

Add `use crate::db::AccountClass;` at the top of the file.

- [ ] **Step 5: The categories data layer**

In `crates/nigel-core/src/categories.rs`, both readers select `class` and run it through `AccountClass::from_db` the same way, and the writers gain the field:

```rust
pub fn add_category(
    conn: &Connection,
    name: &str,
    category_type: &str,
    class: Option<AccountClass>,
    tax_line: Option<&str>,
    form_line: Option<&str>,
) -> Result<i64> {
    validate_fields(name, category_type)?;
    let class = class.unwrap_or_else(|| crate::db::class_for_category_type(category_type));
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM categories WHERE name = ?1 AND is_active = 1)",
        [name],
        |row| row.get(0),
    )?;
    if exists {
        return Err(NigelError::DuplicateName {
            kind: "Category",
            name: name.to_string(),
        });
    }
    conn.execute(
        "INSERT INTO categories (name, category_type, class, tax_line, form_line) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![name, category_type, class.as_str(), tax_line, form_line],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_category(
    conn: &Connection,
    id: i64,
    name: &str,
    category_type: &str,
    class: AccountClass,
    tax_line: Option<&str>,
    form_line: Option<&str>,
) -> Result<()> {
    validate_fields(name, category_type)?;
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM categories WHERE name = ?1 AND is_active = 1 AND id != ?2)",
        rusqlite::params![name, id],
        |row| row.get(0),
    )?;
    if exists {
        return Err(NigelError::DuplicateName {
            kind: "Category",
            name: name.to_string(),
        });
    }
    let updated = conn.execute(
        "UPDATE categories SET name = ?1, category_type = ?2, class = ?3, tax_line = ?4, form_line = ?5 WHERE id = ?6 AND is_active = 1",
        rusqlite::params![name, category_type, class.as_str(), tax_line, form_line, id],
    )?;
    if updated == 0 {
        return Err(NigelError::NotFound(format!("Category not found: id {id}")));
    }
    Ok(())
}
```

`update_category` takes a bare `AccountClass` rather than an `Option`: it is the whole-row writer that `rename_category` and the PATCH route both read the current row for, and an `Option` there would be a second way to say "keep it" beside the one those callers already use. `rename_category`'s existing fetch grows the column:

```rust
    let (cat_type, class, tax_line, form_line): (String, String, Option<String>, Option<String>) =
        conn.query_row(
            "SELECT category_type, class, tax_line, form_line FROM categories WHERE id = ?1 AND is_active = 1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                NigelError::NotFound(format!("Category not found: id {id}"))
            }
            other => NigelError::Db(other),
        })?;
    update_category(
        conn,
        id,
        new_name,
        &cat_type,
        AccountClass::from_db(&class)?,
        tax_line.as_deref(),
        form_line.as_deref(),
    )
```

- [ ] **Step 6: The categories route**

In `crates/nigel-core/src/server/routes/categories.rs`:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewCategory {
    name: String,
    category_type: String,
    /// Absent means the type decides. An unknown value is a `400` from the
    /// extractor — the closed set is the type, not a checker.
    class: Option<AccountClass>,
    tax_line: Option<String>,
    form_line: Option<String>,
}
```

with the `create` body passing `new.class` through, and:

```rust
pub struct CategoryPatch {
    name: Option<String>,
    category_type: Option<String>,
    class: Option<AccountClass>,
    #[serde(default, deserialize_with = "double_option")]
    tax_line: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    form_line: Option<Option<String>>,
}

impl CategoryPatch {
    fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.category_type.is_none()
            && self.class.is_none()
            && self.tax_line.is_none()
            && self.form_line.is_none()
    }
}
```

the `update` handler's merge gains one line and its message one word:

```rust
        return Err(ApiError::bad_request(
            "Nothing to update — provide at least one of `name`, `categoryType`, `class`, `taxLine`, or `formLine`.",
        ));
```

```rust
        let class = patch.class.unwrap_or(current.class);
```

and `update_category` is called with it. Add `use crate::db::AccountClass;`.

- [ ] **Step 7: The accounts route**

In `crates/nigel-core/src/server/routes/accounts.rs`, `NewAccount` gains `class: Option<AccountClass>` and passes it to `add_account`, and the rename handler becomes a partial update:

```rust
/// A partial update: name, class, or both. Institution and last four are set
/// when the account is created, which is all the data layer offers.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountPatch {
    name: Option<String>,
    class: Option<AccountClass>,
}

async fn update(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
    ApiJson(patch): ApiJson<AccountPatch>,
) -> ApiResult<Json<Account>> {
    if patch.name.is_none() && patch.class.is_none() {
        return Err(ApiError::bad_request(
            "Nothing to update — provide `name`, `class`, or both.",
        ));
    }
    let account = with_conn(&state, move |conn| {
        accounts::update_account(conn, id, patch.name.as_deref(), patch.class)?;
        accounts::get_account(conn, id)
    })
    .await?;
    Ok(Json(account))
}
```

with the route wired as `patch(update)`, `ApiError` added to `use super::super::error::{ApiError, ApiResult};`, and `use crate::db::AccountClass;` at the top of the file.

- [ ] **Step 8: Fix the call sites the signature changes break**

```bash
cargo build 2>&1 | rg '^error' -A 4
```
Expected: errors at `crates/nigel/src/cli/accounts.rs`, `crates/nigel/src/cli/categories.rs`, `crates/nigel/src/cli/account_manager.rs`, `crates/nigel/src/cli/category_manager.rs`, and the `mod tests` in `accounts.rs`/`categories.rs`. Pass `None` for the new `class` argument everywhere for now — Task 5 gives each of them a real value. `update_category`'s callers read the current row's class.

- [ ] **Step 9: Run the tests and watch them pass**

```bash
cargo test -p nigel-core -- --test-threads=1 class
cargo test -- --test-threads=1
```
Expected: PASS. `test_add_invalid_type_rejected` in `crates/nigel/src/cli/categories.rs` still passes — `"revenue"` remains an invalid *category type*, which is the whole point of keeping the two vocabularies apart.

---

### Task 5: The CLI and the TUI edit surfaces

**Files:**
- Modify: `crates/nigel/src/cli/mod.rs` (clap), `crates/nigel/src/main.rs` (dispatch)
- Modify: `crates/nigel/src/cli/accounts.rs`, `crates/nigel/src/cli/categories.rs`
- Modify: `crates/nigel/src/cli/account_manager.rs`, `crates/nigel/src/cli/category_manager.rs`
- Test: the `mod tests` in `cli/accounts.rs` and `cli/categories.rs`; new `mod tests` in both managers

**Interfaces:**
- Consumes: `accounts::add_account`/`update_account`, `categories::add_category`/`update_category`, `db::AccountClass`.
- Produces:
  - `nigel accounts add … --class <class>`, new `nigel accounts edit <id> [--name <name>] [--class <class>]`
  - `nigel categories add … --class <class>`, `nigel categories update … --class <class>`
  - A `Class` selector field in both TUI forms; the account manager's Rename screen becomes an Edit screen.
  - `cli::accounts::parse_class(value: &str) -> Result<AccountClass>` — the one place a CLI string becomes a class.

The TUI's `FieldKind::Selector` already cycles a `Vec<String>` with Left/Right and both managers render it identically, so the class field is a fifth entry in the existing list, not a new control. The words shown are the five class words, exactly as stored: no debit, no credit, no abbreviation (AC #6).

The account manager's `Rename` screen becomes `Edit` with two fields, Name and Class, because AC #5 requires class to be settable wherever an account is edited and rename is the only edit an account has. `account_type`, institution and last four stay creation-time.

- [ ] **Step 1: Write the failing tests**

Add to `crates/nigel/src/cli/accounts.rs`, inside `mod tests`:

```rust
    #[test]
    fn a_class_flag_is_parsed_or_names_the_five_words() {
        assert_eq!(parse_class("equity").unwrap(), AccountClass::Equity);
        let err = parse_class("contra-asset").unwrap_err();
        for word in ["asset", "liability", "equity", "revenue", "expense"] {
            assert!(err.to_string().contains(word), "got: {err}");
        }
    }

    #[test]
    fn an_account_takes_its_class_from_its_type_or_from_the_flag() {
        let (_dir, conn) = test_conn();
        add_account(&conn, "Globex Card", "credit_card", None, None, None).unwrap();
        add_account(
            &conn,
            "Globex Payroll",
            "payroll",
            Some(AccountClass::Liability),
            None,
            None,
        )
        .unwrap();

        let by_name = |name: &str| {
            list_accounts(&conn)
                .unwrap()
                .into_iter()
                .find(|a| a.name == name)
                .unwrap()
                .class
        };
        assert_eq!(by_name("Globex Card"), AccountClass::Liability);
        assert_eq!(by_name("Globex Payroll"), AccountClass::Liability);
    }

    #[test]
    fn editing_an_account_changes_only_what_it_names() {
        let (_dir, conn) = test_conn();
        let id = add_account(&conn, "Globex Checking", "checking", None, None, None).unwrap();

        update_account(&conn, id, None, Some(AccountClass::Liability)).unwrap();
        let account = get_account(&conn, id).unwrap();
        assert_eq!(account.name, "Globex Checking");
        assert_eq!(account.class, AccountClass::Liability);

        update_account(&conn, id, Some("Globex Operating"), None).unwrap();
        let account = get_account(&conn, id).unwrap();
        assert_eq!(account.name, "Globex Operating");
        assert_eq!(account.class, AccountClass::Liability);
    }
```

Add to `crates/nigel/src/cli/categories.rs`, inside `mod tests`:

```rust
    #[test]
    fn a_category_takes_its_class_from_its_type_or_from_the_flag() {
        let (_dir, conn) = test_conn();
        add_category(&conn, "Workshop Fees", "income", None, None, None).unwrap();
        let draws = add_category(
            &conn,
            "Partner Draw",
            "expense",
            Some(AccountClass::Equity),
            None,
            None,
        )
        .unwrap();

        let by_name = |name: &str| {
            list_categories(&conn)
                .unwrap()
                .into_iter()
                .find(|c| c.name == name)
                .unwrap()
                .class
        };
        assert_eq!(by_name("Workshop Fees"), AccountClass::Revenue);
        assert_eq!(by_name("Partner Draw"), AccountClass::Equity);

        // A rename keeps the class the operator chose.
        rename_category(&conn, draws, "Member Draw").unwrap();
        assert_eq!(by_name("Member Draw"), AccountClass::Equity);
    }
```

Add a new `mod tests` at the end of `crates/nigel/src/cli/category_manager.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use nigel_core::db::{get_connection, init_db, AccountClass};

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        (dir, conn)
    }

    fn form_of(mgr: &CategoryManager) -> &CategoryForm {
        match &mgr.screen {
            Screen::Add(form) | Screen::Edit(form) => form,
            _ => panic!("not on a form"),
        }
    }

    #[test]
    fn the_add_form_offers_every_class_and_defaults_to_the_type() {
        let (_d, conn) = test_conn();
        let mut mgr = CategoryManager::new(&conn, "Hello.");
        mgr.handle_key(KeyCode::Char('a'), &conn);

        let field = &form_of(&mgr).fields[CLASS_IDX];
        assert_eq!(field.label, "Class");
        assert_eq!(field.value, "expense");
        match &field.kind {
            FieldKind::Selector { options, .. } => {
                assert_eq!(options.len(), AccountClass::ALL.len());
                assert!(options.iter().all(|o| AccountClass::parse(o).is_some()));
                // AC #6: the five words, and nothing borrowed from a ledger.
                assert!(!options.iter().any(|o| o == "debit" || o == "credit"));
            }
            FieldKind::Text => panic!("not a selector"),
        }
    }

    #[test]
    fn an_edit_form_opens_on_the_categorys_own_class() {
        let (_d, conn) = test_conn();
        let mut mgr = CategoryManager::new(&conn, "Hello.");
        let draw = mgr
            .categories
            .iter()
            .position(|c| c.name == "Owner Draw / Distribution")
            .expect("the seeded distribution category");
        mgr.selection = draw;
        mgr.handle_key(KeyCode::Char('e'), &conn);

        assert_eq!(form_of(&mgr).fields[CLASS_IDX].value, "equity");
    }

    #[test]
    fn the_class_selector_cycles_and_saves_what_it_shows() {
        let (_d, conn) = test_conn();
        let mut mgr = CategoryManager::new(&conn, "Hello.");
        mgr.handle_key(KeyCode::Char('a'), &conn);
        // Name, then walk to the class field and pick the next class.
        for ch in "Member Draw".chars() {
            mgr.handle_key(KeyCode::Char(ch), &conn);
        }
        for _ in 0..CLASS_IDX {
            mgr.handle_key(KeyCode::Tab, &conn);
        }
        let before = form_of(&mgr).fields[CLASS_IDX].value.clone();
        mgr.handle_key(KeyCode::Right, &conn);
        let after = form_of(&mgr).fields[CLASS_IDX].value.clone();
        assert_ne!(before, after);

        mgr.handle_key(KeyCode::Enter, &conn);
        let saved = mgr
            .categories
            .iter()
            .find(|c| c.name == "Member Draw")
            .expect("saved");
        assert_eq!(saved.class.as_str(), after);
    }
}
```

Add a new `mod tests` at the end of `crates/nigel/src/cli/account_manager.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyCode;
    use nigel_core::db::{get_connection, init_db, AccountClass};

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        accounts::add_account(&conn, "Globex Card", "credit_card", None, None, None).unwrap();
        (dir, conn)
    }

    fn form_of(mgr: &AccountManager) -> &AccountForm {
        match &mgr.screen {
            Screen::Add(form) | Screen::Edit(form) => form,
            _ => panic!("not on a form"),
        }
    }

    #[test]
    fn the_add_form_offers_every_class() {
        let (_d, conn) = test_conn();
        let mut mgr = AccountManager::new(&conn, "Hello.");
        mgr.handle_key(KeyCode::Char('a'), &conn);

        match &form_of(&mgr).fields[CLASS_IDX].kind {
            FieldKind::Selector { options, .. } => {
                assert_eq!(options.len(), AccountClass::ALL.len());
                assert!(!options.iter().any(|o| o == "debit" || o == "credit"));
            }
            FieldKind::Text => panic!("not a selector"),
        }
    }

    #[test]
    fn the_edit_form_opens_on_the_accounts_class_and_saves_a_new_one() {
        let (_d, conn) = test_conn();
        let mut mgr = AccountManager::new(&conn, "Hello.");
        mgr.handle_key(KeyCode::Char('e'), &conn);
        assert_eq!(form_of(&mgr).fields[CLASS_IDX].value, "liability");

        mgr.handle_key(KeyCode::Tab, &conn);
        mgr.handle_key(KeyCode::Right, &conn);
        let chosen = form_of(&mgr).fields[CLASS_IDX].value.clone();
        mgr.handle_key(KeyCode::Enter, &conn);

        assert_eq!(mgr.accounts[0].class.as_str(), chosen);
        assert_eq!(mgr.accounts[0].name, "Globex Card", "the name is untouched");
    }
}
```

- [ ] **Step 2: Run them and watch them fail**

```bash
cargo test -p nigel -- --test-threads=1 class
```
Expected: FAIL — `cannot find value 'CLASS_IDX'`, `no variant 'Edit'`, `cannot find function 'parse_class'`.

- [ ] **Step 3: The CLI class parser and the two subcommands**

In `crates/nigel/src/cli/accounts.rs`, above `add`:

```rust
/// A `--class` flag as a class. The five words are the vocabulary; the error
/// names all of them rather than only rejecting the one that was typed.
pub fn parse_class(value: &str) -> Result<AccountClass> {
    AccountClass::parse(value).ok_or_else(|| {
        nigel_core::error::NigelError::Invalid(format!(
            "Invalid class: {value} (must be one of: {})",
            AccountClass::ALL
                .iter()
                .map(|c| c.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })
}
```

with `use nigel_core::db::AccountClass;` at the top, and:

```rust
pub fn add(
    name: &str,
    account_type: &str,
    class: Option<&str>,
    institution: Option<&str>,
    last_four: Option<&str>,
) -> Result<()> {
    let class = class.map(parse_class).transpose()?;
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    add_account(&conn, name, account_type, class, institution, last_four)?;
    println!("Added account: {name}");
    Ok(())
}

pub fn edit(id: i64, name: Option<&str>, class: Option<&str>) -> Result<()> {
    if name.is_none() && class.is_none() {
        return Err(nigel_core::error::NigelError::Invalid(
            "Nothing to change — pass --name, --class, or both".into(),
        ));
    }
    let class = class.map(parse_class).transpose()?;
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    update_account(&conn, id, name, class)?;
    println!("Updated account {id}");
    Ok(())
}
```

`list`'s hand-rolled query gains the column so `nigel accounts list` shows it:

```rust
    let mut stmt = conn.prepare(
        "SELECT id, name, account_type, class, institution, last_four FROM accounts",
    )?;
```
with `"Class"` in the header between `"Type"` and `"Institution"` and a `Cell::new(class)` in the row.

In `crates/nigel/src/cli/categories.rs`, `add` and `update` gain `class: Option<&str>` and `class: Option<&str>` respectively, parsed through `crate::cli::accounts::parse_class`. `update` reads the current row when no class is named, so an update that does not mention the class keeps it:

```rust
pub fn update(
    id: i64,
    name: &str,
    category_type: &str,
    class: Option<&str>,
    tax_line: Option<&str>,
    form_line: Option<&str>,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let class = match class {
        Some(value) => crate::cli::accounts::parse_class(value)?,
        None => get_category(&conn, id)?.class,
    };
    update_category(&conn, id, name, category_type, class, tax_line, form_line)?;
    println!("Updated category {id}: {name}");
    Ok(())
}
```

and `list` prints a `Class` column beside `Type`.

- [ ] **Step 4: The clap definitions and the dispatch**

In `crates/nigel/src/cli/mod.rs`, `AccountsCommands::Add` gains:

```rust
        /// Accounting class: asset, liability, equity, revenue, expense
        #[arg(long)]
        class: Option<String>,
```

`AccountsCommands` gains an `Edit`, and `Rename` stays as it is:

```rust
    /// Change an account's name or its accounting class.
    Edit {
        /// Account ID
        id: i64,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// Accounting class: asset, liability, equity, revenue, expense
        #[arg(long)]
        class: Option<String>,
    },
```

`CategoriesCommands::Add` and `::Update` each gain the same `class: Option<String>` argument. In `crates/nigel/src/main.rs`, each arm destructures and forwards `class.as_deref()`, and `AccountsCommands::Edit { id, name, class } => cli::accounts::edit(id, name.as_deref(), class.as_deref())`.

- [ ] **Step 5: The TUI category form**

In `crates/nigel/src/cli/category_manager.rs`, the index block becomes:

```rust
const NAME_IDX: usize = 0;
const TYPE_IDX: usize = 1;
const CLASS_IDX: usize = 2;
const TAX_LINE_IDX: usize = 3;
const FORM_LINE_IDX: usize = 4;
```

with a helper beside `CATEGORY_TYPES`:

```rust
fn class_options() -> Vec<String> {
    AccountClass::ALL
        .iter()
        .map(|c| c.as_str().to_string())
        .collect()
}

fn class_field(selected: AccountClass) -> FormField {
    let options = class_options();
    let index = options
        .iter()
        .position(|o| o == selected.as_str())
        .unwrap_or(0);
    FormField {
        label: "Class",
        value: selected.as_str().to_string(),
        kind: FieldKind::Selector {
            options,
            selected: index,
        },
    }
}
```

`new_add` inserts `class_field(AccountClass::Expense)` after the Type field — matching `CATEGORY_TYPES[0]`, so the two fields open agreeing. `new_edit` inserts `class_field(cat.class)`. The save path resolves it and passes it on:

```rust
                let class = match AccountClass::parse(&form.fields[CLASS_IDX].value) {
                    Some(class) => class,
                    None => {
                        self.set_status("Pick a class".into());
                        return CategoryAction::Continue;
                    }
                };
```
`FormMode::Add` calls `categories::add_category(conn, &name, &cat_type, Some(class), …)` and `FormMode::Edit` calls `categories::update_category(conn, cat.id, &name, &cat_type, class, …)`.

The list gains the column — header and row alike, with `Tax Line` narrowed to make room:

```rust
                    format!(
                        "{:<28} {:<10} {:<10} {:<16} {}",
                        "Name", "Type", "Class", "Tax Line", "Form Line"
                    ),
```

```rust
                    format!(
                        "{marker}{:<28} {:<10} {:<10} {:<16} {}",
                        truncate(&cat.name, 26),
                        cat.category_type,
                        cat.class.as_str(),
                        truncate(tax, 14),
                        form
                    ),
```

Add `use nigel_core::db::AccountClass;`.

- [ ] **Step 6: The TUI account form**

In `crates/nigel/src/cli/account_manager.rs`, `Screen::Rename` becomes `Screen::Edit`, `FormMode::Rename` becomes `FormMode::Edit`, and `new_rename` becomes:

```rust
    fn new_edit(account: &Account) -> Self {
        Self {
            fields: vec![
                FormField {
                    label: "Name",
                    value: account.name.clone(),
                    kind: FieldKind::Text,
                },
                class_field(account.class),
            ],
            focused: 0,
        }
    }
```

with `const CLASS_IDX: usize = 1;` for the edit form and the same `class_field` helper (its own copy — the two managers already keep their own `FieldKind` and neither imports from the other). `new_add` gains `class_field(AccountClass::Asset)` after the Type field, so its indices become Name 0, Type 1, Class 2, Institution 3, Last Four 4 — and the `INST_IDX`/`LAST_IDX` constants move with them. Note the add form's class field does **not** follow the type selector as it is cycled: the selector's default is the class the first account type maps to, and past that the operator's pick is the operator's.

The `Enter` arm's `FormMode::Add` passes `Some(class)` to `accounts::add_account`; `FormMode::Edit` calls:

```rust
                    accounts::update_account(conn, account.id, Some(&new_name), Some(class))
```

The list gains a `Class` column, and the footer hint becomes `" a=add  e=edit  d=delete  Esc=back  q=quit"` with `KeyCode::Char('e')` opening it — `r` stays bound to the same screen so the keystroke in anyone's fingers keeps working.

- [ ] **Step 7: Run the tests and watch them pass**

```bash
cargo test -p nigel -- --test-threads=1 class
cargo test -- --test-threads=1
cargo run -- accounts add "Cedar Checking" --type checking --class asset
cargo run -- accounts list
cargo run -- categories add "Member Draw" --type expense --class equity
cargo run -- categories list
```
Expected: the tests pass; `accounts list` and `categories list` each show a `Class` column with the value just set.

- [ ] **Step 8: Full verification**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
cargo test -p nigel-core -- --test-threads=1
./scripts/check-no-real-data.sh --staged; echo "exit=$?"
```
Expected: all green, `exit=0`.

---

### Task 6: The web screens, component-first

**Files:**
- Create: `web/packages/ui/src/components/account-class.ts`
- Modify: `web/packages/ui/src/components/wc-category-form.ts`, `wc-account-form.ts`, `index.ts`
- Modify: `web/packages/ui/src/components/wc-category-form.preview.ts`, `wc-account-form.preview.ts`
- Modify: `web/apps/app/src/api/types.ts`, `client.ts`, `desktop-client.ts`, `__mocks__/fake-api-client.ts`
- Modify: `web/apps/app/src/screens/categories.ts`, `categories-data.ts`, `accounts.ts`
- Test: `wc-category-form.test.ts`, `wc-account-form.test.ts`, `screens/categories.test.ts`, `accounts.test.ts`, `categories-data.test.ts`

**Interfaces:**
- Consumes: `class` on `Account` and `CategoryRow` from the API.
- Produces:
  - `ACCOUNT_CLASSES`, `AccountClassValue`, `accountClassLabel(value: string): string` from `@nigel/ui`.
  - `CategoryFormValue` and `AccountFormValue` each gain `class: string`.
  - `WcAccountFormMode` becomes `'create' | 'edit'`.
  - `ApiClient.renameAccount` becomes `updateAccount(id: number, input: AccountPatch): Promise<Account>`; `AccountPatch` becomes `{ name?: string; class?: string }`.

No new component: both forms already own a closed-set control, and a class is one more of those. `wc-account-form` uses `wa-select`, which is right for five options; `wc-category-form` uses `wa-radio-group` for its two-value type and gains a `wa-select` for the class beside it, because five radios in a row is a control that stops reading as a choice.

`class` is typed `string`, not a union — the same reasoning `RuleRow.matchType` is documented with: a row written by another tool cannot be assumed to be one of the five, and a narrow type would let a select quietly retype a category it merely displayed. Both selects render an extra out-of-vocabulary option the way `wc-rule-form` does.

- [ ] **Step 1: Write the failing tests**

Create nothing yet; add to `web/packages/ui/src/components/wc-category-form.test.ts`:

```ts
describe('the class control', () => {
  it('offers the five classes and nothing borrowed from a ledger', async () => {
    const el = await mount({ value: { ...EMPTY_CATEGORY_FORM } });
    const options = [
      ...(el.shadowRoot?.querySelectorAll('[data-class] wa-option') ?? []),
    ].map((option) => option.getAttribute('value'));

    expect(options).toEqual([
      'asset',
      'liability',
      'equity',
      'revenue',
      'expense',
    ]);
    expect(el.shadowRoot?.textContent).not.toMatch(/debit|credit/i);
  });

  it('keeps a class it does not know rather than retyping the row', async () => {
    const el = await mount({
      value: { ...EMPTY_CATEGORY_FORM, class: 'contra-asset' },
    });
    const options = [
      ...(el.shadowRoot?.querySelectorAll('[data-class] wa-option') ?? []),
    ].map((option) => option.getAttribute('value'));

    expect(options).toContain('contra-asset');
  });

  it('emits the whole value when the class changes', async () => {
    const el = await mount({
      value: { ...EMPTY_CATEGORY_FORM, name: 'Member Draw' },
    });
    const seen: CategoryFormValue[] = [];
    el.addEventListener('nc-category-form-change', (event) => {
      seen.push((event as CustomEvent<NcCategoryFormChangeDetail>).detail.value);
    });

    const select = el.shadowRoot?.querySelector<HTMLInputElement>('[data-class]');
    if (!select) throw new Error('no class select');
    select.value = 'equity';
    select.dispatchEvent(new Event('change'));

    expect(seen).toEqual([
      { ...EMPTY_CATEGORY_FORM, name: 'Member Draw', class: 'equity' },
    ]);
  });
});
```

Add to `web/apps/app/src/screens/categories-data.test.ts`:

```ts
describe('class round trips through the form', () => {
  it('carries the class into the form and back out on create', () => {
    const row: CategoryRow = {
      id: 9,
      name: 'Member Draw',
      categoryType: 'expense',
      class: 'equity',
      taxLine: null,
      formLine: null,
    };
    expect(toCategoryForm(row).class).toBe('equity');
    expect(newCategoryRequest(toCategoryForm(row)).class).toBe('equity');
  });

  it('patches the class alone and leaves an unchanged class out', () => {
    const current: CategoryRow = {
      id: 9,
      name: 'Member Draw',
      categoryType: 'expense',
      class: 'expense',
      taxLine: null,
      formLine: null,
    };
    const next = { ...toCategoryForm(current), class: 'equity' };
    expect(categoryPatch(current, next)).toEqual({ class: 'equity' });
    expect(categoryPatch(current, toCategoryForm(current))).toEqual({});
  });
});
```

Add to `web/apps/app/src/screens/categories.test.ts` (and update the two fixtures and the `cells` assertion):

```ts
  it('shows each category class in the list', async () => {
    const { el } = await mount();
    expect(table(el).rows.map((row) => row.cells[2])).toEqual([
      'Revenue',
      'Expense',
      'Equity',
    ]);
  });
```

Add to `web/apps/app/src/screens/accounts.test.ts`:

```ts
  it('saves a reclassification without touching the name', async () => {
    const fake = client();
    const { el } = await mount(fake);
    await rowAction(el, 'edit', 1);
    await pick(el, '[data-class]', 'liability');
    await save(el);

    expect(fake.calls).toContain('updateAccount:1:{"class":"liability"}');
  });
```

with a helper beside `type()`, because `wa-select` emits `change` rather than `input`:

```ts
async function pick(
  el: NigelAccountsScreen,
  hook: string,
  value: string,
): Promise<void> {
  const control = form(el).shadowRoot?.querySelector<HTMLInputElement>(hook);
  if (!control) throw new Error(`no ${hook} in the form`);
  control.value = value;
  control.dispatchEvent(new Event('change'));
  await settle(el);
}
```

- [ ] **Step 2: Run them and watch them fail**

```bash
cd web && npm test
```
Expected: FAIL — `no class select`, `Object literal may only specify known properties ('class' does not exist on type 'CategoryRow')`, `fake.calls` has `renameAccount:…`.

- [ ] **Step 3: The vocabulary module**

Create `web/packages/ui/src/components/account-class.ts`:

```ts
/**
 * The accounting classes every account and category carries, in the order the
 * CLI and the TUI offer them.
 *
 * Mirrors `AccountClass` in `crates/nigel-core/src/db.rs`, which is where the
 * set is defined and where the `CHECK` constraint enforces it. Kept here rather
 * than derived from the API because no endpoint publishes it — a select has to
 * name its options.
 *
 * These five words are the whole vocabulary a user sees. No debit, no credit:
 * classification is structure, and the labels stay the plain words.
 */
export const ACCOUNT_CLASSES = [
  'asset',
  'liability',
  'equity',
  'revenue',
  'expense',
] as const;

export type AccountClassValue = (typeof ACCOUNT_CLASSES)[number];

const ACCOUNT_CLASS_LABELS: Record<string, string> = {
  asset: 'Asset',
  liability: 'Liability',
  equity: 'Equity',
  revenue: 'Revenue',
  expense: 'Expense',
};

/**
 * The human name for a class. A value from outside the vocabulary falls back
 * to itself rather than to a guess, so a database written by some other tool
 * still reads honestly.
 */
export function accountClassLabel(value: string): string {
  return ACCOUNT_CLASS_LABELS[value] ?? value;
}
```

Export it from `web/packages/ui/src/components/index.ts`, beside the `account-type.js` block:

```ts
export {
  ACCOUNT_CLASSES,
  accountClassLabel,
  type AccountClassValue,
} from './account-class.js';
```

- [ ] **Step 4: The two forms**

In `web/packages/ui/src/components/wc-category-form.ts`, add the select imports and the value field:

```ts
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import {
  ACCOUNT_CLASSES,
  accountClassLabel,
  type AccountClassValue,
} from './account-class.js';
```

```ts
export interface CategoryFormValue {
  name: string;
  categoryType: string;
  class: string;
  taxLine: string;
  formLine: string;
}

export const EMPTY_CATEGORY_FORM: CategoryFormValue = {
  name: '',
  categoryType: 'expense',
  class: 'expense',
  taxLine: '',
  formLine: '',
};
```

and the control, directly below the `wa-radio-group`:

```ts
        <wa-select
          data-class
          label="Class"
          hint="Where this sits in the accounting structure."
          value=${this.value.class}
          ?disabled=${this.disabled}
          @change=${this.handleField('class')}
        >
          ${ACCOUNT_CLASSES.map(
            (value) =>
              html`<wa-option value=${value}>${accountClassLabel(value)}</wa-option>`,
          )}
          ${unknownClass
            ? html`<wa-option value=${this.value.class}
                >${this.value.class}</wa-option
              >`
            : nothing}
        </wa-select>
```

with, at the top of `render()`:

```ts
    const unknownClass = !ACCOUNT_CLASSES.includes(
      this.value.class as AccountClassValue,
    );
```

`wc-account-form.ts` takes the same field, the same `unknownClass` guard and the same `<wa-select data-class …>` — after the type select in `renderCreateFields`, and above the read-only block in edit mode:

```ts
export type WcAccountFormMode = 'create' | 'edit';
```

The mode switch in `render()` becomes:

```ts
        ${this.mode === 'create' ? this.renderCreateFields() : this.renderEditFields()}
```

with `renderFixed` unchanged — its wording is still true, because type, institution and last four are still creation-time — and a new method putting the editable class above it, so what can be changed and what cannot stay visually separated:

```ts
  private renderEditFields() {
    return html`${this.renderClassSelect()} ${this.renderFixed()}`;
  }
```

`renderClassSelect()` is the one `<wa-select data-class …>`, called from both branches so the create and edit forms cannot drift apart.

`EMPTY_ACCOUNT_FORM` gains `class: 'asset'`.

- [ ] **Step 5: The previews**

Add a class to the `filled` fixture in both preview files and one state each that shows the field carrying a class the form does not know:

```ts
    {
      name: 'a class from outside the vocabulary',
      render: () => html`
        <wc-category-form
          .value=${{ ...filled, class: 'contra-asset' }}
        ></wc-category-form>
      `,
    },
```

`describePreviewA11y` picks the new states up automatically — do not restate them in the test file. Both components already carry `describeControlsAdoption`, and both already adopt `controlsCss`; adding a `wa-select` import to `wc-category-form` does not change that, but `controls-adoption.test.ts` is what proves it.

- [ ] **Step 6: The api types and the client**

In `web/apps/app/src/api/types.ts`:

```ts
/** `GET /api/accounts` */
export interface Account {
  id: number;
  name: string;
  accountType: string;
  /**
   * Where the account sits in the accounting structure: asset, liability,
   * equity, revenue or expense.
   *
   * A plain string rather than a union, for `RuleRow.matchType`'s reason: every
   * write path validates against the five, but a row written by some other tool
   * cannot be assumed to be one of them, and typing it narrowly would make the
   * form's select quietly retype an account it merely displayed.
   */
  class: string;
  institution: string | null;
  lastFour: string | null;
}
```

`CategoryRow` gains the same field. The payload types become:

```ts
/** `POST /api/accounts` — `class` absent means the account type decides. */
export interface NewAccountRequest {
  name: string;
  accountType: string;
  class?: string;
  institution?: string | null;
  lastFour?: string | null;
}

/** `PATCH /api/accounts/:id` — name, class, or both. An empty body is a 400. */
export interface AccountPatch {
  name?: string;
  class?: string;
}

/** `POST /api/categories` — `class` absent means the category type decides. */
export interface NewCategoryRequest {
  name: string;
  categoryType: string;
  class?: string;
  taxLine?: string | null;
  formLine?: string | null;
}

export interface CategoryPatch {
  name?: string;
  categoryType?: string;
  class?: string;
  taxLine?: string | null;
  formLine?: string | null;
}
```

In `web/apps/app/src/api/client.ts`, the interface declaration and the implementation:

```ts
  /** Name, class, or both — `PATCH /api/accounts/:id` takes a partial. */
  updateAccount(id: number, input: AccountPatch): Promise<Account>;
```

```ts
  updateAccount(id: number, input: AccountPatch): Promise<Account> {
    return this.request<Account>('PATCH', `/accounts/${id}`, input);
  }
```

Rename the method in `web/apps/app/src/api/desktop-client.ts` and `web/apps/app/src/__mocks__/fake-api-client.ts` to match, and give the fake's `createAccount`/`createCategory` the derived defaults the server has so the fixtures stay honest:

```ts
  async updateAccount(id: number, input: AccountPatch): Promise<Account> {
    this.calls.push(`updateAccount:${id}:${JSON.stringify(input)}`);
    if (this.renameAccountError) throw this.renameAccountError;

    const account = this.accounts.find((candidate) => candidate.id === id);
    if (!account) throw new Error(`no account ${id} in the fixture`);
    if (input.name !== undefined) account.name = input.name;
    if (input.class !== undefined) account.class = input.class;
    return { ...account };
  }
```

- [ ] **Step 7: The two screens**

In `web/apps/app/src/screens/categories-data.ts`, each of the three functions gains its line:

```ts
export function toCategoryForm(row: CategoryRow): CategoryFormValue {
  return {
    name: row.name,
    categoryType: row.categoryType,
    class: row.class,
    taxLine: row.taxLine ?? '',
    formLine: row.formLine ?? '',
  };
}
```
`newCategoryRequest` adds `class: value.class`, and `categoryPatch` adds `if (next.class !== current.class) patch.class = next.class;`.

In `web/apps/app/src/screens/categories.ts`, the column table and the row mapping:

```ts
const COLUMNS: ManagerColumn[] = [
  { key: 'name', label: 'Name' },
  { key: 'categoryType', label: 'Type' },
  { key: 'class', label: 'Class' },
  { key: 'taxLine', label: 'Tax line' },
  { key: 'formLine', label: 'Form line', mono: true },
];
```

```ts
      cells: [
        category.name,
        category.categoryType === 'income' ? 'Income' : 'Expense',
        accountClassLabel(category.class),
        category.taxLine,
        category.formLine,
      ],
```
with `accountClassLabel` added to the `@nigel/ui` import.

In `web/apps/app/src/screens/accounts.ts`, the same shape — a `{ key: 'class', label: 'Class' }` column after `accountType`, `accountClassLabel(account.class)` in the cells, `class: account.class` in `toFormValue`, `class: editor.value.class` on `createAccount`, the `rename` action renamed to `edit` with the label `Edit`, and the save path:

```ts
      } else if (editor.id !== undefined) {
        const current = this.accounts.find((account) => account.id === editor.id);
        const patch: AccountPatch = {};
        const name = editor.value.name.trim();
        if (!current || current.name !== name) patch.name = name;
        if (!current || current.class !== editor.value.class) {
          patch.class = editor.value.class;
        }
        // An edit that changed nothing is a request that can only fail on itself.
        if (Object.keys(patch).length === 0) {
          this.closeEditor();
          return;
        }
        await this.client.updateAccount(editor.id, patch);
      }
```
with the dialog heading `creating ? 'Add account' : 'Edit account'`.

- [ ] **Step 8: Run the tests and watch them pass**

```bash
cd web && npm run typecheck && npm run lint && npm test
```
Expected: all green. The exact-JSON `fake.calls` assertions in `categories.test.ts` and `accounts.test.ts` must be updated to include `class` in the object-literal order the screens build — key order matters to those assertions.

- [ ] **Step 9: See it**

```bash
cd web && npm run preview     # http://localhost:9090 — wc-category-form, wc-account-form
```
and the full loop:
```bash
cargo run -- serve --no-open   # terminal 1, prints /auth?token=<hex>
cd web && npm run dev          # terminal 2
# browser: http://localhost:5173/auth?token=<hex> — Categories and Accounts
```
Expected: a `Class` column on both lists, a `Class` select in both dialogs, and no occurrence of "debit" or "credit" anywhere on either screen.

- [ ] **Step 10: Full verification**

```bash
cd web && npm run build
cd .. && cargo test -- --test-threads=1
```
Expected: green.

---

### Task 7: The documentation

**Files:**
- Modify: `docs/architecture.md`, `docs/api.md`, `docs/design-constraints.md`, `docs/commands.md`
- Test: `./scripts/check-no-real-data.sh`

**Interfaces:**
- Consumes: everything above.
- Produces: no code.

AC #8 asks for documentation updated *and* out-of-date information removed. Describe the current state only — no "added in v10", no "was formerly `category_type`", no migration-history table. `git log` and `backlog/decisions/` carry that.

- [ ] **Step 1: `docs/architecture.md`**

In the **Database** bullet, the tables line gains the columns and the seeding sentence gains the class:

> tables: accounts (with `class`), categories (with `form_line` for 1120-S mapping and `class`), transactions, …

and a sentence after the `get_profile()` clause:

> `AccountClass` is the five-value closed set — asset, liability, equity, revenue, expense — that both `accounts.class` and `categories.class` carry, with `as_str()`/`parse()` in `Profile`'s shape, `from_db()` for the fallible read, and a `CHECK` constraint on each column so the set holds against a hand-edited database. `class_for_account_type`/`class_for_category_type` are the derivations a create falls back on. `account_type` and `category_type` stay: they are the user-facing vocabulary the UI organizes by, and nothing branches on them for accounting meaning.

In the **Migrations** bullet, append to the version list:

> v10 adds `class` to `accounts` and `categories`, backfills every row — checking/payroll → asset, credit_card/line_of_credit → liability, income → revenue, expense → expense, then `Owner Draw / Distribution` → equity after the general rule, which is the order that makes it correct — and seeds an `Owner Contribution` equity category on business-profile databases when one is absent

In the **Category Manager** bullet, `type (income/expense selector)` becomes `type (income/expense selector), class (the five-class selector)`. Add the reports line:

> `reports::natural_balance(class, raw_sum)` is the one place the sign convention lives: the raw sum stated so more of what the class is reads positive, which the balance report carries as `naturalBalance` beside the register's own `balance`. Nothing re-derives a sign from an account type.

- [ ] **Step 2: `docs/api.md`**

In **List responses**, under `/api/accounts` and `/api/categories`, add:

> Both carry `class` — `asset`, `liability`, `equity`, `revenue` or `expense`.

The **Changing data** table rows become:

```
| `/api/accounts` | `POST` | `name`, `accountType`, `class?`, `institution?`, `lastFour?` | `Account` (`201`) |
| `/api/accounts/:id` | `PATCH` | `name?`, `class?` | `Account` |
| `/api/categories` | `POST` | `name`, `categoryType`, `class?`, `taxLine?`, `formLine?` | `CategoryRow` (`201`) |
| `/api/categories/:id` | `PATCH` | `name?`, `categoryType?`, `class?`, `taxLine?`, `formLine?` | `CategoryRow` |
```

Rewrite the **Accounts, categories, and rules** paragraph that says renaming is the only account edit:

> Accounts are hard-deleted and categories are soft-deleted, exactly as in the CLI and the TUI. `PATCH /api/accounts/:id` takes a name, a class, or both; institution, last four and `accountType` are set at creation, which is all the data layer offers. A patch with neither field is a `400`.
>
> `class` is the accounting class — `asset`, `liability`, `equity`, `revenue`, `expense` — and anything outside those five is a `400`. Omitting it on a create derives it: an account from its `accountType` (`credit_card` and `line_of_credit` are liabilities, everything else an asset) and a category from its `categoryType` (`income` → `revenue`, `expense` → `expense`). A client that has never heard of the field therefore keeps working unchanged.

In **Report responses**, note the balance report's second figure:

> The balance report's accounts each carry `class` and `naturalBalance` beside `balance`. `balance` is the register summed with the signs the transactions were imported with, which is what `total` adds up; `naturalBalance` is the same figure stated so that more of what the class is reads positive — a liability with money owed reports positive.

- [ ] **Step 3: `docs/design-constraints.md`**

Add one rule, in the file's existing voice:

> - **A class match never carries a catch-all arm.** `db::AccountClass` is a closed set of five, and every `match` over it names all five — in the reports, in the K-1's mapping, in `natural_balance`, in the TUI's row styling. The one permitted default is `AccountClass::parse` answering `None` for a string that is not a class, which its caller turns into an error rather than into a class. The reason is specific: owner distributions were reported as deductions for as long as they were, because a category whose meaning nothing handled fell into an `else` and was counted as spending. A sixth class must be a compile error at every site that decides what a class means, and a `_ =>` arm is what takes that guarantee away. `crates/nigel-core/src/reports/mod.rs`'s absorption test is the runtime half: a category on each class in turn, asserting the expense totals move only for `expense`.
>
> - **Classification is structure; the words on screen are not.** `account_type` and `category_type` stay exactly as they read — checking, credit card, income, expense — and `class` sits underneath them. A freelancer importing a bank CSV never sees "debit" or "credit"; the five class words are the entire accounting vocabulary any surface shows.

- [ ] **Step 4: `docs/commands.md`**

Add `--class` to the `nigel accounts add` and `nigel categories add`/`update` entries and document `nigel accounts edit`, in the file's existing table/section style:

> `nigel accounts edit <id> [--name <name>] [--class <class>]` — change an account's name, its accounting class, or both. `<class>` is one of `asset`, `liability`, `equity`, `revenue`, `expense`.

- [ ] **Step 5: Verify**

```bash
./scripts/check-no-real-data.sh; echo "exit=$?"
rg -n 'added in|was formerly|changed in version|migration v10 added' docs/
wc -l CLAUDE.md
```
Expected: `exit=0`, no provenance prose in `docs/`, `CLAUDE.md` untouched (no command, rule or pointer changed there — the new rule lives in `docs/design-constraints.md`, which CLAUDE.md already points at).

---

## Closing verification

Run everything, in CI's order, and confirm each before claiming the work is done:

```bash
./scripts/check-no-real-data.sh; echo "exit=$?"
cd web && npm run lint && npm run typecheck && npm test && npm run build && cd ..
cargo fmt --check
cargo clippy -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
cargo test --no-default-features --features serve -- --test-threads=1
cargo test -p nigel-core -- --test-threads=1
```

Then, and only then:

```bash
gh pr create --draft --base main --head feat/account-classification \
  --title "TASK-9.1: account classification" --body "…"
```

**It is a draft. It stays a draft until the operator reviews it** (AC #10).

### Acceptance criteria, and where each is discharged

| AC | Where |
|---|---|
| #1 One closed-set class vocabulary on accounts and categories | Task 1 — `AccountClass`, both columns, both `CHECK` constraints |
| #2 Migration backfills everything, distributions included, no manual work | Task 1 — migration v10 and its five tests |
| #3 Every report classifies from class; equity out of deductions everywhere | Task 3 — P&L, expense breakdown, tax summary, YTD net income, K-1 |
| #4 Liability sign convention the balance and Schedule L can rely on | Task 2 — `natural_balance` and its five-row table, `naturalBalance` on the balance report |
| #5 Class settable and visible in CLI, TUI, web | Tasks 4, 5, 6 |
| #6 No debit/credit vocabulary anywhere user-facing | Tasks 5 and 6, asserted in `the_add_form_offers_every_class_and_defaults_to_the_type` and the `wc-category-form` class-control test |
| #7 Tests including the silent-absorption guard | Task 3 — `no_class_but_expense_can_reach_the_expense_totals` |
| #8 Documentation created or updated, stale information removed | Task 7 |
| #9 All linting passes | Closing verification |
| #10 The PR is a draft until reviewed | Header and closing verification |
