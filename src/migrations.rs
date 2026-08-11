use rusqlite::Connection;

use crate::db::set_metadata;
use crate::error::Result;

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
            // `validate_date` pads on the way in; these are the rows written
            // before it did. Parsing is chrono's, not SQL's, so the rule is
            // exactly the one the data layer applies — and a value that does
            // not parse is left untouched rather than guessed at.
            for (table, column) in [
                ("invoices", "issue_date"),
                ("invoices", "due_date"),
                ("invoices", "published_at"),
                ("invoices", "voided_at"),
                ("invoice_payments", "paid_date"),
            ] {
                normalize_date_column(conn, table, column)?;
            }
            Ok(())
        },
    },
];

pub const LATEST_VERSION: u32 = MIGRATIONS[MIGRATIONS.len() - 1].version;

/// Rewrite every parseable value in a date column to zero-padded `YYYY-MM-DD`.
///
/// `table` and `column` are compile-time literals from v6's own list, never user
/// input, so the `format!` is not an injection seam.
fn normalize_date_column(conn: &Connection, table: &str, column: &str) -> Result<()> {
    let mut stmt = conn.prepare(&format!(
        "SELECT id, {column} FROM {table} WHERE {column} IS NOT NULL"
    ))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);

    for (id, value) in rows {
        let Ok(date) = chrono::NaiveDate::parse_from_str(&value, "%Y-%m-%d") else {
            continue;
        };
        let padded = date.format("%Y-%m-%d").to_string();
        if padded != value {
            conn.execute(
                &format!("UPDATE {table} SET {column} = ?1 WHERE id = ?2"),
                rusqlite::params![padded, id],
            )?;
        }
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
}

#[cfg(test)]
mod invoicing_migration_tests {
    use crate::db::{get_connection, init_db};
    use crate::migrations::run_migrations;

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
