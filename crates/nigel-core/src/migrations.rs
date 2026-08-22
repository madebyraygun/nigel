use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension};

use crate::db::{set_metadata, AccountClass, Profile};
use crate::error::Result;
use crate::invoicing::invoices::{refresh_status, validate_date};

struct Migration {
    version: u32,
    description: &'static str,
    up: fn(&Connection) -> Result<()>,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "baseline — establish schema version tracking",
        up: |_conn| Ok(()),
    },
    Migration {
        version: 2,
        description: "add csv_profiles table for generic CSV column mappings",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE csv_profiles (
                    id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL UNIQUE,
                    date_col INTEGER NOT NULL,
                    desc_col INTEGER NOT NULL,
                    amount_col INTEGER NOT NULL,
                    date_format TEXT NOT NULL DEFAULT '%m/%d/%Y',
                    created_at TEXT DEFAULT (datetime('now'))
                )",
            )?;
            Ok(())
        },
    },
    Migration {
        version: 3,
        description: "backfill 1120-S form_line for stock chart-of-accounts categories",
        up: |conn| {
            conn.execute_batch(
                "UPDATE categories SET form_line = '1120S-1a'
                     WHERE form_line IS NULL AND tax_line = 'Gross receipts'
                       AND name IN ('Client Services', 'Hosting & Maintenance', 'Reimbursements');
                 UPDATE categories SET form_line = '1120S-5'
                     WHERE form_line IS NULL AND tax_line = 'Other income'
                       AND name = 'Other Income';
                 UPDATE categories SET form_line = '1120S-2'
                     WHERE form_line IS NULL
                       AND tax_line = 'Schedule C Part III / 1120-S Line 2'
                       AND name = 'Cost of Goods Sold';
                 UPDATE categories SET form_line = 'excluded'
                     WHERE form_line IS NULL AND tax_line = 'Not deductible'
                       AND name = 'Transfer';",
            )?;
            Ok(())
        },
    },
    Migration {
        version: 4,
        description: "add invoicing tables (clients, invoices, line items, payments)",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS clients (
                    id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    email TEXT,
                    billing_address TEXT,
                    notes TEXT,
                    created_at TEXT DEFAULT (datetime('now'))
                );
                CREATE TABLE IF NOT EXISTS invoices (
                    id INTEGER PRIMARY KEY,
                    number INTEGER NOT NULL UNIQUE,
                    client_id INTEGER NOT NULL,
                    issue_date TEXT NOT NULL,
                    due_date TEXT,
                    status TEXT NOT NULL DEFAULT 'draft',
                    currency TEXT NOT NULL DEFAULT 'USD',
                    subtotal REAL NOT NULL DEFAULT 0,
                    tax REAL NOT NULL DEFAULT 0,
                    total REAL NOT NULL DEFAULT 0,
                    notes TEXT,
                    terms TEXT,
                    token TEXT NOT NULL UNIQUE,
                    stripe_payment_link_id TEXT,
                    stripe_payment_link_url TEXT,
                    published_at TEXT,
                    created_at TEXT DEFAULT (datetime('now')),
                    FOREIGN KEY (client_id) REFERENCES clients(id)
                );
                CREATE TABLE IF NOT EXISTS invoice_line_items (
                    id INTEGER PRIMARY KEY,
                    invoice_id INTEGER NOT NULL,
                    description TEXT NOT NULL,
                    quantity REAL NOT NULL DEFAULT 1,
                    unit_amount REAL NOT NULL DEFAULT 0,
                    line_total REAL NOT NULL DEFAULT 0,
                    position INTEGER NOT NULL DEFAULT 0,
                    FOREIGN KEY (invoice_id) REFERENCES invoices(id)
                );
                CREATE TABLE IF NOT EXISTS invoice_payments (
                    id INTEGER PRIMARY KEY,
                    invoice_id INTEGER NOT NULL,
                    amount REAL NOT NULL,
                    paid_date TEXT NOT NULL,
                    method TEXT NOT NULL CHECK (method IN ('stripe','ach','direct_deposit','other')),
                    stripe_checkout_session_id TEXT UNIQUE,
                    recorded_at TEXT DEFAULT (datetime('now')),
                    FOREIGN KEY (invoice_id) REFERENCES invoices(id)
                );",
            )?;
            Ok(())
        },
    },
    Migration {
        version: 5,
        description: "add voided_at to invoices so void is a derived status like sent",
        up: |conn| {
            // SQLite has no ADD COLUMN IF NOT EXISTS; the probe is what makes a
            // replay of this migration as harmless as v4's IF NOT EXISTS tables.
            let has_column: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('invoices') WHERE name = 'voided_at'",
                [],
                |r| r.get(0),
            )?;
            if !has_column {
                conn.execute_batch("ALTER TABLE invoices ADD COLUMN voided_at TEXT")?;
            }
            conn.execute_batch(
                "UPDATE invoices SET voided_at = COALESCE(published_at, issue_date)
                     WHERE status = 'void' AND voided_at IS NULL",
            )?;
            Ok(())
        },
    },
    Migration {
        version: 6,
        description: "normalize stored invoice dates to zero-padded YYYY-MM-DD",
        up: |conn| {
            // A stored date is padded through `validate_date` itself, so the
            // migration and the writers cannot disagree about what a date is; a
            // value that rule rejects is left untouched rather than guessed at.
            let mut touched = BTreeSet::new();
            for column in ["issue_date", "due_date", "published_at", "voided_at"] {
                touched.extend(normalize_date_column(conn, "invoices", column)?);
            }
            for payment_id in normalize_date_column(conn, "invoice_payments", "paid_date")? {
                let invoice_id: Option<i64> = conn
                    .query_row(
                        "SELECT invoice_id FROM invoice_payments WHERE id = ?1",
                        [payment_id],
                        |r| r.get(0),
                    )
                    .optional()?;
                touched.extend(invoice_id);
            }
            re_derive_status(conn, &touched)
        },
    },
    Migration {
        version: 7,
        description: "add archived_at to clients so a finished client can leave the list",
        up: |conn| {
            // v5's probe, for v5's reason: SQLite has no ADD COLUMN IF NOT
            // EXISTS, and a replay must be harmless.
            let has_column: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('clients') WHERE name = 'archived_at'",
                [],
                |r| r.get(0),
            )?;
            if !has_column {
                conn.execute_batch("ALTER TABLE clients ADD COLUMN archived_at TEXT")?;
            }
            // No backfill: every existing client is active, which NULL says.
            Ok(())
        },
    },
    Migration {
        version: 8,
        description: "move client emails into client_contacts, one row per address",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS client_contacts (
                    id INTEGER PRIMARY KEY,
                    client_id INTEGER NOT NULL,
                    name TEXT,
                    email TEXT NOT NULL,
                    title TEXT,
                    is_billing INTEGER NOT NULL DEFAULT 0,
                    position INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT DEFAULT (datetime('now')),
                    FOREIGN KEY (client_id) REFERENCES clients(id) ON DELETE CASCADE
                );
                 -- At most one billing contact per client, enforced by the
                 -- database rather than by remembering to check.
                 CREATE UNIQUE INDEX IF NOT EXISTS idx_client_contacts_billing
                     ON client_contacts(client_id) WHERE is_billing = 1;
                 -- One address per client, case-insensitively: a cc that is
                 -- also the To is a duplicate delivery, not a second recipient.
                 CREATE UNIQUE INDEX IF NOT EXISTS idx_client_contacts_email
                     ON client_contacts(client_id, lower(email));",
            )?;

            // v5's probe again, and here it also makes the backfill replayable:
            // once the column is gone there is nothing left to read.
            let has_email: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('clients') WHERE name = 'email'",
                [],
                |r| r.get(0),
            )?;
            if has_email {
                conn.execute_batch(
                    "INSERT INTO client_contacts (client_id, email, is_billing, position)
                     SELECT id, TRIM(email), 1, 0
                       FROM clients
                      WHERE email IS NOT NULL AND TRIM(email) <> '';
                     ALTER TABLE clients DROP COLUMN email;",
                )?;
            }
            Ok(())
        },
    },
    Migration {
        version: 9,
        description: "seed payment_instructions from the paragraph the stock page used to print",
        up: |conn| seed_payment_instructions(conn, contact_address()),
    },
    Migration {
        version: 10,
        description: "classify accounts and categories as asset/liability/equity/revenue/expense",
        up: classify_accounts_and_categories,
    },
    Migration {
        version: 12,
        description: "add recurring invoice schedules, their items and their run history",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS invoice_schedules (
                    id INTEGER PRIMARY KEY,
                    client_id INTEGER NOT NULL,
                    cadence TEXT NOT NULL CHECK (cadence IN ('monthly','quarterly','yearly')),
                    anchor_day INTEGER NOT NULL CHECK (anchor_day BETWEEN 1 AND 31),
                    next_period TEXT NOT NULL,
                    net_days INTEGER,
                    currency TEXT NOT NULL DEFAULT 'USD',
                    notes TEXT,
                    terms TEXT,
                    autosend INTEGER NOT NULL DEFAULT 0,
                    paused INTEGER NOT NULL DEFAULT 0,
                    ended_at TEXT,
                    created_at TEXT DEFAULT (datetime('now')),
                    FOREIGN KEY (client_id) REFERENCES clients(id)
                );
                 CREATE TABLE IF NOT EXISTS invoice_schedule_items (
                    id INTEGER PRIMARY KEY,
                    schedule_id INTEGER NOT NULL,
                    description TEXT NOT NULL,
                    quantity REAL NOT NULL DEFAULT 1,
                    unit_amount REAL NOT NULL DEFAULT 0,
                    position INTEGER NOT NULL DEFAULT 0,
                    FOREIGN KEY (schedule_id) REFERENCES invoice_schedules(id) ON DELETE CASCADE
                );
                 CREATE TABLE IF NOT EXISTS invoice_schedule_runs (
                    id INTEGER PRIMARY KEY,
                    schedule_id INTEGER NOT NULL,
                    period TEXT NOT NULL,
                    invoice_id INTEGER NOT NULL,
                    generated_at TEXT NOT NULL,
                    FOREIGN KEY (schedule_id) REFERENCES invoice_schedules(id),
                    FOREIGN KEY (invoice_id) REFERENCES invoices(id)
                );
                 -- Provenance and idempotency in one row: a rerun for a period
                 -- already generated finds this and writes nothing.
                 CREATE UNIQUE INDEX IF NOT EXISTS idx_invoice_schedule_runs_period
                     ON invoice_schedule_runs(schedule_id, period);",
            )?;
            Ok(())
        },
    },
];

pub const LATEST_VERSION: u32 = MIGRATIONS[MIGRATIONS.len() - 1].version;

/// The sentence the stock invoice page hardcoded before payment instructions
/// became the operator's own text, with the address `{{CONTACT}}` used to
/// interpolate written in.
///
/// The invoice number is gone from it: the old paragraph said "reference invoice
/// #1248" per document, and a setting is one value for every invoice. "the
/// invoice number" says the same thing to a person reading a bill.
fn legacy_payment_instructions(contact: &str) -> String {
    format!(
        "Direct deposit: to pay by bank transfer, reference the invoice number. \
         Contact {contact} for account details."
    )
}

/// The address the old page printed, from wherever this installation keeps it.
fn contact_address() -> Option<String> {
    let cfg = crate::settings::invoicing_config();
    cfg.contact_email
        .or(cfg.from_email)
        .map(|address| address.trim().to_string())
        .filter(|address| !address.is_empty())
}

/// Give an installation that has been invoicing the payment instructions its
/// documents used to carry.
///
/// Deleting the hardcoded paragraph from the stock page is a silent regression
/// for everyone who was relying on it: nothing seeds the new key, so both
/// documents would simply stop saying how to pay. This puts the old sentence
/// where the operator can now edit or delete it.
///
/// Three conditions, all of them about not inventing anything:
///
/// - **The key is unset.** A value already there is the operator's, including a
///   deliberately empty one, and a migration never overwrites one of those. This
///   is also what makes a replay a no-op.
/// - **There is a contact address.** The old sentence's only variable part was
///   `{{CONTACT}}`; with nothing to put there it printed a broken line, and
///   seeding that would be worse than seeding nothing.
/// - **This database has invoiced.** A brand-new database runs every migration
///   too, and a fresh install never printed the old paragraph — so it starts
///   with no payment instructions, which is the whole point of making them
///   configurable.
fn seed_payment_instructions(conn: &Connection, contact: Option<String>) -> Result<()> {
    if crate::db::get_metadata(conn, "payment_instructions").is_some() {
        return Ok(());
    }
    let Some(contact) = contact else {
        return Ok(());
    };
    let has_invoiced: bool =
        conn.query_row("SELECT COUNT(*) > 0 FROM invoices", [], |r| r.get(0))?;
    if !has_invoiced {
        return Ok(());
    }
    set_metadata(
        conn,
        "payment_instructions",
        &legacy_payment_instructions(&contact),
    )
}

/// The `CHECK` every class column carries, written once so the two tables and
/// the fresh-install schema cannot drift apart.
const CLASS_CHECK: &str = "CHECK (class IN ('asset', 'liability', 'equity', 'revenue', 'expense'))";

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
/// defect this migration exists to end. `Owner Contribution` is named there too
/// because a fresh install seeds it and then runs every migration, and the
/// general `income` rule would otherwise take its `equity` back off it.
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
         UPDATE categories SET class = 'equity'
             WHERE name IN ('Owner Draw / Distribution', 'Owner Contribution');",
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

/// Rewrite every value a date column holds that `validate_date` accepts to its
/// zero-padded form, answering the ids of the rows that moved.
///
/// `table` and `column` are compile-time literals from v6's own list, never user
/// input, so the `format!` is not an injection seam.
fn normalize_date_column(conn: &Connection, table: &str, column: &str) -> Result<Vec<i64>> {
    let mut select = conn.prepare(&format!(
        "SELECT id, {column} FROM {table} WHERE {column} IS NOT NULL"
    ))?;
    let rows = select
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(select);

    let mut update = conn.prepare(&format!("UPDATE {table} SET {column} = ?1 WHERE id = ?2"))?;
    let mut changed = Vec::new();
    for (id, value) in rows {
        let Ok(padded) = validate_date(&value, column) else {
            continue;
        };
        if padded != value {
            update.execute(rusqlite::params![padded, id])?;
            changed.push(id);
        }
    }
    Ok(changed)
}

/// Re-derive the status of every invoice whose stored dates moved.
///
/// Padding a due date changes what `is_overdue`'s string comparison answers, so
/// a status derived from the unpadded value no longer follows from the row it
/// sits in. Void invoices are skipped: their status comes from `voided_at`, and
/// nothing about a date rewrite may take an invoice out of that terminal state.
/// The wall clock is the reference day: migrations run inside a command, which is
/// the same context every other `refresh_status` caller derives from.
fn re_derive_status(conn: &Connection, invoice_ids: &BTreeSet<i64>) -> Result<()> {
    let mut live = Vec::new();
    {
        let mut stmt = conn.prepare("SELECT voided_at IS NULL FROM invoices WHERE id = ?1")?;
        for id in invoice_ids {
            if stmt.query_row([id], |r| r.get::<_, bool>(0)).optional()? == Some(true) {
                live.push(*id);
            }
        }
    }
    let today = crate::clock::today();
    for id in live {
        refresh_status(conn, id, &today)?;
    }
    Ok(())
}

/// Returns the current schema version, or 0 if no version has been set.
/// Propagates actual DB errors instead of silently defaulting to 0.
pub fn get_schema_version(conn: &Connection) -> Result<u32> {
    match conn.query_row(
        "SELECT value FROM metadata WHERE key = 'schema_version'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        Ok(v) => v
            .parse::<u32>()
            .map_err(|_| crate::error::NigelError::Other(format!("invalid schema_version: {v}"))),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
        Err(e) => Err(e.into()),
    }
}

pub fn run_migrations(conn: &Connection) -> Result<()> {
    apply_migrations(conn, MIGRATIONS)
}

fn apply_migrations(conn: &Connection, migrations: &[Migration]) -> Result<()> {
    let current = get_schema_version(conn)?;
    for migration in migrations {
        if migration.version > current {
            eprintln!(
                "Applying migration v{}: {}",
                migration.version, migration.description
            );
            let sp_name = format!("migration_v{}", migration.version);
            conn.execute_batch(&format!("SAVEPOINT {sp_name}"))?;
            match (|| -> Result<()> {
                (migration.up)(conn)?;
                set_metadata(conn, "schema_version", &migration.version.to_string())?;
                Ok(())
            })() {
                Ok(()) => conn.execute_batch(&format!("RELEASE {sp_name}"))?,
                Err(e) => {
                    conn.execute_batch(&format!("ROLLBACK TO {sp_name}; RELEASE {sp_name}"))?;
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};

    fn test_db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        init_db(&conn).unwrap();
        (dir, conn)
    }

    #[test]
    fn test_fresh_install_at_latest_version() {
        let (_dir, conn) = test_db();
        let version = get_schema_version(&conn).unwrap();
        assert_eq!(version, LATEST_VERSION);
    }

    #[test]
    fn migration_versions_are_contiguous_from_one() {
        // `apply_migrations` runs what is numbered above the database's stamp, so
        // a duplicate number is skipped and a gap swallows whatever later fills
        // it — both silently, and both are what two branches numbering their
        // migrations in parallel produce. Contiguity is the invariant that turns
        // either into a failing test instead of a database missing a migration.
        for (index, migration) in MIGRATIONS.iter().enumerate() {
            let expected = index as u32 + 1;
            assert_eq!(
                migration.version, expected,
                "migration {:?} is v{} at index {index}, where v{expected} belongs",
                migration.description, migration.version
            );
        }
    }

    #[test]
    fn test_v0_upgrade() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        // Create schema without running migrations (simulates 0.1.x)
        conn.execute_batch(crate::db::SCHEMA).unwrap();
        assert_eq!(get_schema_version(&conn).unwrap(), 0);

        run_migrations(&conn).unwrap();
        assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
    }

    #[test]
    fn test_idempotent_rerun() {
        let (_dir, conn) = test_db();
        let v1 = get_schema_version(&conn).unwrap();
        run_migrations(&conn).unwrap();
        let v2 = get_schema_version(&conn).unwrap();
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_csv_profiles_table_exists_after_migration() {
        let (_dir, conn) = test_db();
        let exists: bool = conn
            .query_row(
                "SELECT count(*) > 0 FROM sqlite_master WHERE type='table' AND name='csv_profiles'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(exists, "csv_profiles table should exist after init_db");
    }

    #[test]
    fn test_failed_migration_rolls_back() {
        let (_dir, conn) = test_db();
        assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);

        let bad_migrations = &[Migration {
            version: LATEST_VERSION + 1,
            description: "failing migration",
            up: |conn| {
                conn.execute_batch("CREATE TABLE _test_rollback (id INTEGER)")?;
                Err(crate::error::NigelError::Other(
                    "intentional failure".into(),
                ))
            },
        }];

        let result = apply_migrations(&conn, bad_migrations);
        assert!(result.is_err());
        // Version unchanged
        assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
        // Table creation rolled back
        let table_exists: bool = conn
            .query_row(
                "SELECT count(*) > 0 FROM sqlite_master WHERE type='table' AND name='_test_rollback'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!table_exists);
    }

    #[test]
    fn v6_pads_stored_invoice_dates_and_leaves_unparseable_ones_alone() {
        let (_dir, conn) = test_db();
        conn.execute_batch(
            "INSERT INTO clients (id, name) VALUES (1, 'Acme');
             INSERT INTO invoices (id, number, client_id, issue_date, due_date, published_at,
                                   voided_at, status, currency, total, token)
                 VALUES (1, 1248, 1, '2026-8-7', '2026-9-1', '2026-8-8', NULL, 'sent', 'USD', 100, 't1'),
                        (2, 1249, 1, '2026-08-07', NULL, NULL, '2026-8-9', 'void', 'USD', 100, 't2'),
                        (3, 1250, 1, 'March',     NULL, NULL, NULL,       'draft','USD', 100, 't3');
             INSERT INTO invoice_payments (invoice_id, amount, paid_date, method)
                 VALUES (1, 50, '2026-8-10', 'ach');
             UPDATE metadata SET value = '5' WHERE key = 'schema_version';",
        )
        .unwrap();

        run_migrations(&conn).unwrap();

        let date = |sql: &str| {
            conn.query_row(sql, [], |r| r.get::<_, Option<String>>(0))
                .unwrap()
        };
        assert_eq!(
            date("SELECT issue_date   FROM invoices WHERE id = 1").as_deref(),
            Some("2026-08-07")
        );
        assert_eq!(
            date("SELECT due_date     FROM invoices WHERE id = 1").as_deref(),
            Some("2026-09-01")
        );
        assert_eq!(
            date("SELECT published_at FROM invoices WHERE id = 1").as_deref(),
            Some("2026-08-08")
        );
        assert_eq!(
            date("SELECT voided_at    FROM invoices WHERE id = 2").as_deref(),
            Some("2026-08-09")
        );
        assert_eq!(
            date("SELECT paid_date    FROM invoice_payments WHERE invoice_id = 1").as_deref(),
            Some("2026-08-10")
        );
        assert_eq!(
            date("SELECT issue_date FROM invoices WHERE id = 3").as_deref(),
            Some("March"),
            "a migration that rewrites what it cannot parse is guessing"
        );
        assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
    }

    #[test]
    fn clients_gain_an_archived_at_column() {
        let (_dir, conn) = test_db();
        run_migrations(&conn).unwrap();
        let has: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('clients') WHERE name = 'archived_at'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(has);
        assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
    }

    #[test]
    fn v6_is_idempotent_on_already_padded_dates() {
        let (_dir, conn) = test_db();
        conn.execute_batch(
            "INSERT INTO clients (id, name) VALUES (1, 'Acme');
             INSERT INTO invoices (id, number, client_id, issue_date, status, currency, total, token)
                 VALUES (1, 1248, 1, '2026-08-07', 'draft', 'USD', 100, 't1');
             UPDATE metadata SET value = '5' WHERE key = 'schema_version';",
        )
        .unwrap();

        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap();
        let issue: String = conn
            .query_row("SELECT issue_date FROM invoices WHERE id = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(issue, "2026-08-07");
    }

    /// Padding a due date changes what `is_overdue`'s string comparison answers, so
    /// a status derived from the unpadded value is stale the moment v6 runs. v6
    /// re-derives it rather than leaving the row disagreeing with its own dates.
    #[test]
    fn v6_re_derives_a_status_the_unpadded_due_date_had_wrong() {
        let (_dir, conn) = test_db();
        conn.execute_batch(
            "INSERT INTO clients (id, name) VALUES (1, 'Acme');
             INSERT INTO invoices (id, number, client_id, issue_date, due_date, published_at,
                                   status, currency, total, token)
                 VALUES (1, 1248, 1, '2020-01-05', '2020-1-5', '2020-01-05', 'sent', 'USD', 100, 't1');
             UPDATE metadata SET value = '5' WHERE key = 'schema_version';",
        )
        .unwrap();

        run_migrations(&conn).unwrap();

        let row = |col: &str| -> String {
            conn.query_row(
                &format!("SELECT {col} FROM invoices WHERE id = 1"),
                [],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(row("due_date"), "2020-01-05");
        assert_eq!(row("status"), "overdue");
    }

    /// A void invoice's status is derived from `voided_at`, not from its dates, and
    /// v6 must not walk it back to something else.
    #[test]
    fn v6_leaves_a_void_invoice_void() {
        let (_dir, conn) = test_db();
        conn.execute_batch(
            "INSERT INTO clients (id, name) VALUES (1, 'Acme');
             INSERT INTO invoices (id, number, client_id, issue_date, due_date, published_at,
                                   voided_at, status, currency, total, token)
                 VALUES (1, 1248, 1, '2020-01-05', '2020-1-5', '2020-01-05', '2020-2-1',
                         'void', 'USD', 100, 't1');
             UPDATE metadata SET value = '5' WHERE key = 'schema_version';",
        )
        .unwrap();

        run_migrations(&conn).unwrap();

        let status: String = conn
            .query_row("SELECT status FROM invoices WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "void");
    }

    /// A payment date that moves does not change what the invoice is owed, but the
    /// row still gets re-derived, so a status stale for any reason settles.
    #[test]
    fn v6_re_derives_from_a_changed_payment_date_too() {
        let (_dir, conn) = test_db();
        conn.execute_batch(
            "INSERT INTO clients (id, name) VALUES (1, 'Acme');
             INSERT INTO invoices (id, number, client_id, issue_date, published_at,
                                   status, currency, total, token)
                 VALUES (1, 1248, 1, '2020-01-05', '2020-01-05', 'sent', 'USD', 100, 't1');
             INSERT INTO invoice_payments (invoice_id, amount, paid_date, method)
                 VALUES (1, 100, '2020-1-9', 'ach');
             UPDATE metadata SET value = '5' WHERE key = 'schema_version';",
        )
        .unwrap();

        run_migrations(&conn).unwrap();

        let paid: String = conn
            .query_row("SELECT paid_date FROM invoice_payments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(paid, "2020-01-09");
        let status: String = conn
            .query_row("SELECT status FROM invoices WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "paid");
    }

    #[test]
    fn every_existing_client_starts_active() {
        let (_dir, conn) = test_db();
        // Rewind to the schema as it stood before the archive column, so the
        // migration runs against a database that really predates it.
        conn.execute_batch(
            "ALTER TABLE clients DROP COLUMN archived_at;
             UPDATE metadata SET value = '6' WHERE key = 'schema_version';",
        )
        .unwrap();
        conn.execute("INSERT INTO clients (name) VALUES ('Acme Co')", [])
            .unwrap();

        run_migrations(&conn).unwrap();

        let archived: Option<String> = conn
            .query_row("SELECT archived_at FROM clients", [], |r| r.get(0))
            .unwrap();
        assert_eq!(archived, None);
    }

    /// `DROP COLUMN` needs SQLite 3.35+, and the migration must not be where we
    /// find out otherwise.
    #[test]
    fn the_bundled_sqlite_supports_drop_column() {
        let (_dir, conn) = test_db();
        let v: String = conn
            .query_row("SELECT sqlite_version()", [], |r| r.get(0))
            .unwrap();
        let parts: Vec<u32> = v.split('.').filter_map(|p| p.parse().ok()).collect();
        assert!(
            parts[0] > 3 || (parts[0] == 3 && parts[1] >= 35),
            "sqlite {v} cannot DROP COLUMN"
        );
    }

    /// A database as it stood before contacts: `clients.email` back, the
    /// contacts table gone, the version rewound.
    fn rewind_to_pre_contacts(conn: &Connection) {
        conn.execute_batch(
            "DROP TABLE IF EXISTS client_contacts;
             ALTER TABLE clients ADD COLUMN email TEXT;
             UPDATE metadata SET value = '7' WHERE key = 'schema_version';",
        )
        .unwrap();
    }

    #[test]
    fn a_single_email_becomes_one_billing_contact() {
        let (_dir, conn) = test_db();
        rewind_to_pre_contacts(&conn);
        conn.execute_batch(
            "INSERT INTO clients (name, email) VALUES ('Acme Co', '  ap@acme.test  ');
             INSERT INTO clients (name, email) VALUES ('Globex', NULL);",
        )
        .unwrap();

        run_migrations(&conn).unwrap();

        let (email, billing, position): (String, i64, i64) = conn
            .query_row(
                "SELECT email, is_billing, position FROM client_contacts
                  WHERE client_id = (SELECT id FROM clients WHERE name = 'Acme Co')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            email, "ap@acme.test",
            "the address is trimmed, not re-entered"
        );
        assert_eq!(billing, 1);
        assert_eq!(position, 0);

        let globex: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM client_contacts
                  WHERE client_id = (SELECT id FROM clients WHERE name = 'Globex')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(globex, 0);
    }

    #[test]
    fn a_blank_email_does_not_become_a_contact() {
        let (_dir, conn) = test_db();
        rewind_to_pre_contacts(&conn);
        conn.execute(
            "INSERT INTO clients (name, email) VALUES ('Blank', '   ')",
            [],
        )
        .unwrap();

        run_migrations(&conn).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM client_contacts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "a whitespace address is not an address");
    }

    #[test]
    fn the_clients_table_no_longer_carries_an_email_column() {
        let (_dir, conn) = test_db();
        let has: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('clients') WHERE name = 'email'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!has, "a second source of truth for the billing address");
    }

    #[test]
    fn two_billing_contacts_for_one_client_are_refused_by_the_index() {
        let (_dir, conn) = test_db();
        conn.execute("INSERT INTO clients (name) VALUES ('Acme Co')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO client_contacts (client_id, email, is_billing) VALUES (1, 'a@x.test', 1)",
            [],
        )
        .unwrap();

        assert!(conn
            .execute(
                "INSERT INTO client_contacts (client_id, email, is_billing) \
                 VALUES (1, 'b@x.test', 1)",
                [],
            )
            .is_err());
    }

    #[test]
    fn the_same_address_twice_for_one_client_is_refused_case_insensitively() {
        let (_dir, conn) = test_db();
        conn.execute("INSERT INTO clients (name) VALUES ('Acme Co')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO client_contacts (client_id, email) VALUES (1, 'ap@acme.test')",
            [],
        )
        .unwrap();

        assert!(conn
            .execute(
                "INSERT INTO client_contacts (client_id, email) VALUES (1, 'AP@acme.test')",
                [],
            )
            .is_err());
    }

    #[test]
    fn deleting_a_client_takes_its_contacts_with_it() {
        let (_dir, conn) = test_db();
        conn.execute("INSERT INTO clients (name) VALUES ('Acme Co')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO client_contacts (client_id, email) VALUES (1, 'ap@acme.test')",
            [],
        )
        .unwrap();

        conn.execute("DELETE FROM clients WHERE id = 1", [])
            .unwrap();

        let left: i64 = conn
            .query_row("SELECT COUNT(*) FROM client_contacts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            left, 0,
            "the FK cascade is live — db.rs sets foreign_keys=ON"
        );
    }

    #[test]
    fn the_contacts_migration_is_replayable() {
        let (_dir, conn) = test_db();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap();
        assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
    }

    #[test]
    fn the_archive_migration_is_replayable() {
        let (_dir, conn) = test_db();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap();
        assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
    }

    #[test]
    fn v12_creates_the_three_schedule_tables_and_the_period_uniqueness() {
        let (_dir, conn) = test_db();
        assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);

        for table in [
            "invoice_schedules",
            "invoice_schedule_items",
            "invoice_schedule_runs",
        ] {
            let exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(exists, "{table} is missing");
        }

        conn.execute_batch(
            "INSERT INTO clients (name) VALUES ('Acme Co');
             INSERT INTO invoice_schedules
                 (client_id, cadence, anchor_day, next_period, currency)
             VALUES (1, 'monthly', 1, '2026-01-01', 'USD');
             INSERT INTO invoices (number, client_id, issue_date, token)
             VALUES (1248, 1, '2026-01-01', 'tok-a');
             INSERT INTO invoice_schedule_runs (schedule_id, period, invoice_id, generated_at)
             VALUES (1, '2026-01-01', 1, '2026-01-01');",
        )
        .unwrap();

        let second = conn.execute(
            "INSERT INTO invoice_schedule_runs (schedule_id, period, invoice_id, generated_at)
             VALUES (1, '2026-01-01', 1, '2026-02-01')",
            [],
        );
        assert!(
            second.is_err(),
            "a second row for the same period must be refused"
        );
    }

    #[test]
    fn v12_is_replayable() {
        let (_dir, conn) = test_db();
        set_metadata(&conn, "schema_version", "10").unwrap();
        run_migrations(&conn).unwrap();
        assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
    }
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
        assert_eq!(
            class_of(&conn, "categories", "Owner Contribution"),
            "equity"
        );
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
}

#[cfg(test)]
mod invoicing_migration_tests {
    use super::*;
    use crate::db::{get_connection, init_db};

    #[test]
    fn invoicing_tables_exist_after_migration() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        for table in [
            "clients",
            "invoices",
            "invoice_line_items",
            "invoice_payments",
        ] {
            let n: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "missing table {table}");
        }
    }

    #[test]
    fn invoices_carry_a_voided_at_column() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();

        let mut stmt = conn.prepare("PRAGMA table_info(invoices)").unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert!(cols.iter().any(|c| c == "voided_at"), "got: {cols:?}");
    }

    #[test]
    fn a_hand_set_void_status_is_backfilled_with_a_voided_at() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        conn.execute_batch(crate::db::SCHEMA).unwrap();
        let up_to_v4 = &crate::migrations::MIGRATIONS[..4];
        crate::migrations::apply_migrations(&conn, up_to_v4).unwrap();

        conn.execute("INSERT INTO clients (name) VALUES ('Acme')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO invoices (number, client_id, issue_date, status, token)
             VALUES (1248, 1, '2026-08-04', 'void', 'tok')",
            [],
        )
        .unwrap();

        run_migrations(&conn).unwrap();

        let voided_at: Option<String> = conn
            .query_row(
                "SELECT voided_at FROM invoices WHERE number = 1248",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(voided_at.as_deref(), Some("2026-08-04"));
    }

    /// A database that has been invoicing under the old stock page, upgrading.
    fn invoiced_db_at_v8() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        conn.execute_batch(crate::db::SCHEMA).unwrap();
        let up_to_v8 = &MIGRATIONS[..8];
        apply_migrations(&conn, up_to_v8).unwrap();
        conn.execute("INSERT INTO clients (name) VALUES ('Acme')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO invoices (number, client_id, issue_date, status, token)
             VALUES (1248, 1, '2026-08-04', 'sent', 'tok')",
            [],
        )
        .unwrap();
        (dir, conn)
    }

    fn stored_instructions(conn: &Connection) -> Option<String> {
        crate::db::get_metadata(conn, "payment_instructions")
    }

    /// The regression this migration exists for: an operator upgrades, the
    /// stock page stops hardcoding a bank-transfer paragraph, and without this
    /// their next invoice would go out with no way to pay it by transfer and
    /// nothing to say so.
    #[test]
    fn v9_gives_an_upgraded_database_the_instructions_its_documents_carried() {
        let (_dir, conn) = invoiced_db_at_v8();
        seed_payment_instructions(&conn, Some("billing@example.com".into())).unwrap();

        let stored = stored_instructions(&conn).expect("seeded");
        assert!(stored.contains("bank transfer"), "got: {stored}");
        assert!(stored.contains("billing@example.com"), "got: {stored}");
        assert!(
            !stored.contains("1248") && !stored.contains("{{"),
            "one value for every invoice, with nothing left to expand: {stored}"
        );
    }

    #[test]
    fn v9_never_overwrites_instructions_the_operator_already_set() {
        let (_dir, conn) = invoiced_db_at_v8();
        set_metadata(&conn, "payment_instructions", "Cheques only, please").unwrap();
        seed_payment_instructions(&conn, Some("billing@example.com".into())).unwrap();
        assert_eq!(
            stored_instructions(&conn).as_deref(),
            Some("Cheques only, please")
        );
    }

    /// Deliberately cleared is a decision, and a replay must not undo it.
    #[test]
    fn v9_leaves_a_deliberately_empty_value_empty_and_is_idempotent() {
        let (_dir, conn) = invoiced_db_at_v8();
        set_metadata(&conn, "payment_instructions", "").unwrap();
        seed_payment_instructions(&conn, Some("billing@example.com".into())).unwrap();
        assert_eq!(stored_instructions(&conn).as_deref(), Some(""));

        let (_dir, conn) = invoiced_db_at_v8();
        seed_payment_instructions(&conn, Some("billing@example.com".into())).unwrap();
        let once = stored_instructions(&conn);
        seed_payment_instructions(&conn, Some("someone.else@example.com".into())).unwrap();
        assert_eq!(stored_instructions(&conn), once, "a replay changed it");
    }

    /// The old sentence's only variable part was the address. With nothing to
    /// put there it printed a broken line, and seeding that is worse than
    /// seeding nothing.
    #[test]
    fn v9_seeds_nothing_when_the_installation_has_no_contact_address() {
        let (_dir, conn) = invoiced_db_at_v8();
        seed_payment_instructions(&conn, None).unwrap();
        assert_eq!(stored_instructions(&conn), None);
    }

    /// A fresh `nigel init` runs every migration too. It never printed the old
    /// paragraph, so it starts with no payment instructions — which is what
    /// making them configurable was for.
    #[test]
    fn v9_seeds_nothing_on_a_database_that_has_never_invoiced() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        seed_payment_instructions(&conn, Some("billing@example.com".into())).unwrap();
        assert_eq!(stored_instructions(&conn), None);
        assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
    }

    /// End to end through `run_migrations`, reading the address from the
    /// settings file the way the migration itself does.
    #[test]
    fn v9_reads_the_address_from_the_installations_own_settings() {
        let _config = crate::settings::TempConfigDir::new();
        let mut settings = crate::settings::load_settings();
        settings.from_email = Some("accounts@example.com".into());
        crate::settings::save_settings(&settings).unwrap();

        let (_dir, conn) = invoiced_db_at_v8();
        run_migrations(&conn).unwrap();

        let stored = stored_instructions(&conn).expect("seeded");
        assert!(stored.contains("accounts@example.com"), "got: {stored}");
        assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
    }
}

#[cfg(test)]
mod k1_backfill_tests {
    use crate::db::{get_connection, init_db};

    #[test]
    fn backfills_stock_categories_only_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();

        // Simulate a pre-migration database: blank the seeded mappings,
        // add a custom category sharing a stock tax_line, and a category
        // with an existing explicit mapping.
        conn.execute("UPDATE categories SET form_line = NULL", [])
            .unwrap();
        conn.execute(
            "INSERT INTO categories (name, category_type, tax_line, form_line) \
             VALUES ('My Consulting', 'income', 'Gross receipts', NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE categories SET form_line = 'K-16d' WHERE name = 'Owner Draw / Distribution'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE metadata SET value = '2' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();

        super::run_migrations(&conn).unwrap();

        let fl = |name: &str| -> Option<String> {
            conn.query_row(
                "SELECT form_line FROM categories WHERE name = ?1",
                [name],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(fl("Client Services").as_deref(), Some("1120S-1a"));
        assert_eq!(fl("Hosting & Maintenance").as_deref(), Some("1120S-1a"));
        assert_eq!(fl("Reimbursements").as_deref(), Some("1120S-1a"));
        assert_eq!(fl("Other Income").as_deref(), Some("1120S-5"));
        assert_eq!(fl("Cost of Goods Sold").as_deref(), Some("1120S-2"));
        assert_eq!(fl("Transfer").as_deref(), Some("excluded"));
        assert_eq!(fl("Uncategorized"), None); // deliberately left unmapped
        assert_eq!(fl("My Consulting"), None); // custom name untouched
        assert_eq!(fl("Owner Draw / Distribution").as_deref(), Some("K-16d")); // not overwritten
    }
}
