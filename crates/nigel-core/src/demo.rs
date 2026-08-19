//! The demo books: eighteen months of invented transactions, rules, clients
//! and invoices for a fictional consultancy.
//!
//! It lives in the core crate rather than the CLI because the web's setup
//! route offers the same "show me the demo" exit the terminal's onboarding
//! does, and two seeders would drift.

use std::path::PathBuf;

use chrono::{Datelike, Local, NaiveDate};
use rusqlite::Connection;

use crate::categorizer::categorize_transactions;
use crate::db::{get_connection, init_db};
use crate::error::Result;
use crate::invoicing::clients::add_client;
use crate::invoicing::invoices::{create_invoice, mark_published, record_payment, NewLineItem};
use crate::settings::load_settings;

pub const ACCOUNT_NAME: &str = "BofA Checking";

struct DemoTxn {
    date: String,
    description: &'static str,
    amount: f64,
}

/// Recurring transactions generated every month.
struct RecurringTxn {
    day: u32,
    description: &'static str,
    amount: f64,
}

const RECURRING: &[RecurringTxn] = &[
    RecurringTxn {
        day: 1,
        description: "GUSTO PAYROLL",
        amount: -3200.00,
    },
    RecurringTxn {
        day: 5,
        description: "ADOBE CREATIVE CLOUD",
        amount: -54.99,
    },
    RecurringTxn {
        day: 5,
        description: "GITHUB INC",
        amount: -21.00,
    },
    RecurringTxn {
        day: 5,
        description: "SLACK TECHNOLOGIES",
        amount: -12.50,
    },
    RecurringTxn {
        day: 5,
        description: "GOOGLE WORKSPACE",
        amount: -14.40,
    },
    RecurringTxn {
        day: 8,
        description: "AMAZON WEB SERVICES",
        amount: -189.00,
    },
    RecurringTxn {
        day: 8,
        description: "FLYWHEEL HOSTING",
        amount: -89.00,
    },
];

/// Rotating one-off expenses — each month picks a subset based on index.
struct RotatingTxn {
    day: u32,
    description: &'static str,
    amount: f64,
}

const ROTATING: &[RotatingTxn] = &[
    RotatingTxn {
        day: 15,
        description: "CHECK 1042",
        amount: -2400.00,
    },
    RotatingTxn {
        day: 20,
        description: "VENMO PAYMENT",
        amount: -150.00,
    },
    RotatingTxn {
        day: 25,
        description: "COMCAST BUSINESS",
        amount: -129.99,
    },
    RotatingTxn {
        day: 28,
        description: "STAPLES OFFICE SUPPLY",
        amount: -67.23,
    },
    RotatingTxn {
        day: 7,
        description: "WEWORK MEMBERSHIP",
        amount: -450.00,
    },
    RotatingTxn {
        day: 12,
        description: "ZOOM VIDEO COMMUNICATIONS",
        amount: -14.99,
    },
    RotatingTxn {
        day: 19,
        description: "COSTCO WHOLESALE",
        amount: -29.33,
    },
    RotatingTxn {
        day: 25,
        description: "FEDEX SHIPPING",
        amount: -18.75,
    },
    RotatingTxn {
        day: 14,
        description: "DROPBOX BUSINESS",
        amount: -19.99,
    },
    RotatingTxn {
        day: 18,
        description: "TARGET STORE",
        amount: -43.67,
    },
];

/// Meal delivery vendors rotated across months.
const MEALS: &[(&str, &str)] = &[
    ("UBER EATS", "GRUBHUB DELIVERY"),
    ("GRUBHUB DELIVERY", "DOORDASH DELIVERY"),
    ("UBER EATS", "DOORDASH DELIVERY"),
];

/// Base income amounts for the two monthly Stripe transfers.
/// Tuned so that average monthly income is ~$9k, creating a realistic mix
/// of profitable and unprofitable months against ~$4.7k/month in expenses.
const INCOME_BASES: &[(f64, f64)] = &[
    (5500.0, 3800.0),
    (7500.0, 4500.0),
    (3200.0, 1800.0),
    (6000.0, 5000.0),
    (3800.0, 1500.0),
    (8000.0, 4200.0),
];

/// Base meal amounts cycled per month.
const MEAL_AMOUNTS: &[(f64, f64)] = &[
    (-32.18, -28.45),
    (-41.50, -35.72),
    (-27.90, -33.10),
    (-38.25, -24.60),
    (-29.99, -31.40),
    (-44.15, -26.80),
];

/// Clamp a day to the last valid day of the given year/month.
fn clamp_day(year: i32, month: u32, day: u32) -> u32 {
    let first_of_next = NaiveDate::from_ymd_opt(year, month + 1, 1)
        .or_else(|| NaiveDate::from_ymd_opt(year + 1, 1, 1))
        // year+1, Jan 1 is always valid for any realistic year
        .expect("valid year for date arithmetic");
    let last_day = first_of_next
        .pred_opt()
        // predecessor of a valid date is always valid (won't underflow NaiveDate::MIN here)
        .expect("predecessor of first-of-month is valid")
        .day();
    day.min(last_day)
}

fn make_date(year: i32, month: u32, day: u32) -> String {
    let d = clamp_day(year, month, day);
    format!("{year:04}-{month:02}-{d:02}")
}

/// Build 18 months of demo transactions ending at the current month.
fn generate_transactions() -> Vec<DemoTxn> {
    let today = Local::now().date_naive();
    let mut txns = Vec::new();

    for i in 0..18u32 {
        // Count backwards: i=0 is 17 months ago, i=17 is current month
        let months_ago = 17 - i;
        let target = today - chrono::Months::new(months_ago);
        let year = target.year();
        let month = target.month();
        let idx = i as usize;

        // — Income: two Stripe transfers per month —
        let (base1, base2) = INCOME_BASES[idx % INCOME_BASES.len()];
        // Small deterministic variation: +/- up to ~3% based on month index
        let vary = 1.0 + ((idx % 7) as f64 - 3.0) * 0.01;
        txns.push(DemoTxn {
            date: make_date(year, month, 3),
            description: "STRIPE TRANSFER",
            amount: (base1 * vary * 100.0).round() / 100.0,
        });
        txns.push(DemoTxn {
            date: make_date(year, month, 17),
            description: "STRIPE TRANSFER",
            amount: (base2 * vary * 100.0).round() / 100.0,
        });

        // — Recurring subscriptions & hosting —
        for r in RECURRING {
            // AWS varies slightly each month
            let amt = if r.description == "AMAZON WEB SERVICES" {
                let base = r.amount;
                let delta = ((idx % 5) as f64 - 2.0) * 3.5;
                ((base + delta) * 100.0).round() / 100.0
            } else {
                r.amount
            };
            txns.push(DemoTxn {
                date: make_date(year, month, r.day),
                description: r.description,
                amount: amt,
            });
        }

        // — Meals: two per month, rotating vendors —
        let (meal1_desc, meal2_desc) = MEALS[idx % MEALS.len()];
        let (meal1_amt, meal2_amt) = MEAL_AMOUNTS[idx % MEAL_AMOUNTS.len()];
        txns.push(DemoTxn {
            date: make_date(year, month, 12),
            description: meal1_desc,
            amount: meal1_amt,
        });
        txns.push(DemoTxn {
            date: make_date(year, month, 22),
            description: meal2_desc,
            amount: meal2_amt,
        });

        // — Rotating extras: pick 3 per month from the pool —
        for j in 0..3usize {
            let pick = (idx * 3 + j) % ROTATING.len();
            let rot = &ROTATING[pick];
            txns.push(DemoTxn {
                date: make_date(year, month, rot.day),
                description: rot.description,
                amount: rot.amount,
            });
        }

        // — Interest payment on last day of month —
        let interest = 1.50 + (idx % 5) as f64 * 0.25;
        txns.push(DemoTxn {
            date: make_date(year, month, 31),
            description: "INTEREST PAYMENT",
            amount: (interest * 100.0).round() / 100.0,
        });
    }

    txns
}

struct DemoRule {
    pattern: &'static str,
    category: &'static str,
    vendor: &'static str,
}

const RULES: &[DemoRule] = &[
    DemoRule {
        pattern: "STRIPE TRANSFER",
        category: "Client Services",
        vendor: "Stripe",
    },
    DemoRule {
        pattern: "GUSTO",
        category: "Contract Labor",
        vendor: "Gusto",
    },
    DemoRule {
        pattern: "ADOBE",
        category: "Software & Subscriptions",
        vendor: "Adobe",
    },
    DemoRule {
        pattern: "GITHUB",
        category: "Software & Subscriptions",
        vendor: "GitHub",
    },
    DemoRule {
        pattern: "SLACK",
        category: "Software & Subscriptions",
        vendor: "Slack",
    },
    DemoRule {
        pattern: "GOOGLE WORKSPACE",
        category: "Software & Subscriptions",
        vendor: "Google",
    },
    DemoRule {
        pattern: "AMAZON WEB SERVICES",
        category: "Hosting & Infrastructure",
        vendor: "AWS",
    },
    DemoRule {
        pattern: "FLYWHEEL",
        category: "Hosting & Infrastructure",
        vendor: "Flywheel",
    },
    DemoRule {
        pattern: "UBER EATS",
        category: "Meals",
        vendor: "Uber Eats",
    },
    DemoRule {
        pattern: "GRUBHUB",
        category: "Meals",
        vendor: "Grubhub",
    },
];

/// The demo's invoicing clients: name, email, billing address.
const CLIENTS: &[(&str, &str, &str)] = &[
    (
        "Cedar Systems",
        "ap@cedarsystems.test",
        "88 Cedar Way, Bend OR 97701",
    ),
    (
        "Harbor & Vale",
        "billing@harborvale.test",
        "1200 Harbor St, Portland OR 97209",
    ),
    (
        "Juniper Labs",
        "accounts@juniperlabs.test",
        "45 Juniper Rd, Eugene OR 97401",
    ),
];

struct DemoItem {
    description: &'static str,
    quantity: f64,
    unit_amount: f64,
}

struct DemoInvoice {
    /// Index into [`CLIENTS`].
    client: usize,
    issued_days_ago: i64,
    /// Offset from today, so a positive number is a due date still to come.
    due_in_days: i64,
    items: &'static [DemoItem],
    /// Whether `send` has been run on it. Every status but `draft` needs this,
    /// because `refresh_status` derives `sent` from `published_at`.
    published: bool,
    /// One payment: how much, and how many days ago.
    payment: Option<(f64, i64)>,
}

/// Four invoices covering the statuses a demo has any use for — paid, partly
/// paid, sent and unpaid, and a draft still being written. Every date is
/// relative to today, the way the transactions are, so a demo database is
/// never stale. All are Net 30.
const INVOICES: &[DemoInvoice] = &[
    DemoInvoice {
        client: 0,
        issued_days_ago: 75,
        due_in_days: -45,
        items: &[DemoItem {
            description: "Brand identity system",
            quantity: 1.0,
            unit_amount: 4800.00,
        }],
        published: true,
        payment: Some((4800.00, 50)),
    },
    DemoInvoice {
        client: 1,
        issued_days_ago: 25,
        due_in_days: 5,
        items: &[DemoItem {
            description: "Website redesign \u{2014} phase 1",
            quantity: 1.0,
            unit_amount: 6500.00,
        }],
        published: true,
        payment: Some((3250.00, 10)),
    },
    DemoInvoice {
        client: 2,
        issued_days_ago: 12,
        due_in_days: 18,
        items: &[
            DemoItem {
                description: "Discovery workshop",
                quantity: 2.0,
                unit_amount: 1500.00,
            },
            DemoItem {
                description: "Technical audit",
                quantity: 1.0,
                unit_amount: 2200.00,
            },
        ],
        published: true,
        payment: None,
    },
    DemoInvoice {
        client: 0,
        issued_days_ago: 2,
        due_in_days: 28,
        items: &[
            DemoItem {
                description: "Q3 retainer",
                quantity: 1.0,
                unit_amount: 3200.00,
            },
            DemoItem {
                description: "Additional design hours",
                quantity: 6.0,
                unit_amount: 165.00,
            },
        ],
        published: false,
        payment: None,
    },
];

const DEMO_TERMS: &str = "Net 30";

fn offset_day(today: NaiveDate, days: i64) -> String {
    (today + chrono::Duration::days(days))
        .format("%Y-%m-%d")
        .to_string()
}

/// Clients and invoices for the invoicing screens, which `review`'s seed
/// leaves empty. Returns the counts for the summary.
///
/// No invoice here gets a payment link: `set_payment_link` is never called, so
/// `sync_all_report` — whose query is `WHERE stripe_payment_link_id IS NOT
/// NULL` — skips every one of them and the launch sync stays silent on a demo
/// database. Statuses are derived rather than written: `mark_published` and
/// `record_payment` each call `refresh_status` themselves.
fn insert_demo_invoicing(conn: &Connection) -> Result<(usize, usize)> {
    let today = Local::now().date_naive();
    let client_ids = CLIENTS
        .iter()
        .map(|(name, email, address)| add_client(conn, name, Some(email), Some(address), None))
        .collect::<Result<Vec<i64>>>()?;

    for invoice in INVOICES {
        let items: Vec<NewLineItem> = invoice
            .items
            .iter()
            .map(|item| NewLineItem {
                description: item.description.to_string(),
                quantity: item.quantity,
                unit_amount: item.unit_amount,
            })
            .collect();
        let issued = offset_day(today, -invoice.issued_days_ago);
        let id = create_invoice(
            conn,
            client_ids[invoice.client],
            &issued,
            Some(&offset_day(today, invoice.due_in_days)),
            "USD",
            &items,
            None,
            Some(DEMO_TERMS),
        )?;
        if invoice.published {
            mark_published(conn, id, &issued)?;
        }
        if let Some((amount, days_ago)) = invoice.payment {
            record_payment(conn, id, amount, &offset_day(today, -days_ago), "ach", None)?;
        }
    }

    Ok((CLIENTS.len(), INVOICES.len()))
}

fn insert_demo_data(conn: &Connection) -> Result<usize> {
    let txns = generate_transactions();
    let txn_count = txns.len();

    // Create account
    conn.execute(
        "INSERT INTO accounts (name, account_type, institution) VALUES (?1, 'checking', 'Bank of America')",
        [ACCOUNT_NAME],
    )?;
    let account_id = conn.last_insert_rowid();

    // Insert transactions — all flagged initially
    for txn in &txns {
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) \
             VALUES (?1, ?2, ?3, ?4, 1, 'No matching rule')",
            rusqlite::params![account_id, txn.date, txn.description, txn.amount],
        )?;
    }

    // Insert rules
    for rule in RULES {
        let cat_id: i64 = conn.query_row(
            "SELECT id FROM categories WHERE name = ?1",
            [rule.category],
            |r| r.get(0),
        )?;
        conn.execute(
            "INSERT INTO rules (pattern, match_type, vendor, category_id, priority, is_active) \
             VALUES (?1, 'contains', ?2, ?3, 0, 1)",
            rusqlite::params![rule.pattern, rule.vendor, cat_id],
        )?;
    }

    Ok(txn_count)
}

/// What a loaded demo database holds, read back rather than tallied, so a run
/// that only filled in a missing half still reports what is actually there.
pub struct DemoSummary {
    pub transactions: i64,
    pub rules: i64,
    pub categorized: i64,
    pub flagged: i64,
    pub clients: i64,
    pub invoices: i64,
}

pub fn demo_summary(conn: &Connection) -> Result<DemoSummary> {
    let count = |sql: &str| -> Result<i64> { Ok(conn.query_row(sql, [], |r| r.get(0))?) };
    Ok(DemoSummary {
        transactions: count("SELECT COUNT(*) FROM transactions")?,
        rules: count("SELECT COUNT(*) FROM rules")?,
        categorized: count("SELECT COUNT(*) FROM transactions WHERE category_id IS NOT NULL")?,
        flagged: count("SELECT COUNT(*) FROM transactions WHERE is_flagged = 1")?,
        clients: count("SELECT COUNT(*) FROM clients")?,
        invoices: count("SELECT COUNT(*) FROM invoices")?,
    })
}

/// Seed whatever is missing, answering whether anything was written.
///
/// The two halves cannot share a transaction — `create_invoice` opens its own,
/// and SQLite has no nested `BEGIN` — so they are two commits, and a failure
/// between them would be permanent if the account row alone were the guard:
/// every later run would report the data already loaded and never seed the
/// invoicing half. Each half is therefore guarded on what it writes.
pub fn seed_demo(conn: &Connection) -> Result<bool> {
    let exists = |sql: &str| -> Result<bool> { Ok(conn.query_row(sql, [], |r| r.get(0))?) };
    let has_ledger: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE name = ?1)",
        [ACCOUNT_NAME],
        |r| r.get(0),
    )?;
    let has_invoicing = exists("SELECT EXISTS(SELECT 1 FROM invoices)")?;
    if has_ledger && has_invoicing {
        return Ok(false);
    }

    if !has_ledger {
        insert_demo_data(conn)?;
    }
    if !has_invoicing {
        insert_demo_invoicing(conn)?;
    }
    crate::db::set_metadata(conn, "company_name", "Acme Consulting LLC")?;
    categorize_transactions(conn)?;
    Ok(true)
}

/// Build `<data_dir>/demo/` with its own seeded database and repoint
/// `settings.data_dir` at it, so the demo never touches the user's own books.
///
/// The demo is business books — its rules name business categories — so the
/// directory is initialized on the business chart whatever profile the caller
/// keeps next door.
pub fn setup_demo_dir() -> Result<PathBuf> {
    let mut settings = load_settings();
    let demo_dir = PathBuf::from(&settings.data_dir).join("demo");
    for dir in [demo_dir.clone(), demo_dir.join("exports")] {
        std::fs::create_dir_all(&dir)?;
        crate::settings::restrict_dir_permissions(&dir)?;
    }

    let db_path = demo_dir.join("nigel.db");
    let conn = get_connection(&db_path)?;
    init_db(&conn)?;
    seed_demo(&conn)?;
    drop(conn);

    settings.data_dir = demo_dir.to_string_lossy().to_string();
    crate::settings::save_settings(&settings)?;

    Ok(db_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> (tempfile::TempDir, Connection) {
        crate::db::set_db_password(None);
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = crate::db::open_connection(&dir.path().join("nigel.db"), None).expect("open");
        crate::db::init_db(&conn).expect("init");
        (dir, conn)
    }

    #[test]
    fn seeding_writes_the_fixture_company() {
        let (_dir, conn) = test_db();

        assert!(seed_demo(&conn).expect("seed"), "first seed wrote nothing");

        assert_eq!(
            crate::db::get_metadata(&conn, "company_name").as_deref(),
            Some("Acme Consulting LLC")
        );
    }

    #[test]
    fn seeding_twice_is_a_no_op() {
        let (_dir, conn) = test_db();
        seed_demo(&conn).expect("first seed");

        assert!(!seed_demo(&conn).expect("second seed"), "seeded twice");

        let summary = demo_summary(&conn).expect("summary");
        let again = demo_summary(&conn).expect("summary");
        assert_eq!(summary.transactions, again.transactions);
    }

    #[test]
    fn setup_demo_dir_builds_its_own_books_and_repoints_settings() {
        crate::db::set_db_password(None);
        let _config = crate::settings::TempConfigDir::new();
        let base = tempfile::tempdir().expect("tempdir");
        let mut settings = crate::settings::load_settings();
        settings.data_dir = base.path().to_string_lossy().to_string();
        crate::settings::save_settings(&settings).expect("save");

        let db_path = setup_demo_dir().expect("demo dir");

        assert_eq!(db_path, base.path().join("demo").join("nigel.db"));
        assert!(db_path.exists(), "no demo database");
        assert_eq!(
            crate::settings::load_settings().data_dir,
            base.path().join("demo").to_string_lossy().to_string(),
            "settings.json was not repointed"
        );
        let conn = crate::db::get_connection(&db_path).expect("open");
        assert_eq!(
            crate::db::get_metadata(&conn, "company_name").as_deref(),
            Some("Acme Consulting LLC")
        );
    }

    #[test]
    fn test_generate_transactions_count() {
        let txns = generate_transactions();
        // 18 months × 15 txns per month (2 income + 7 recurring + 2 meals + 3 rotating + 1 interest)
        assert_eq!(txns.len(), 18 * 15);
    }

    #[test]
    fn test_generate_transactions_span_current_year() {
        let txns = generate_transactions();
        let current_year = Local::now().date_naive().year();
        let year_prefix = format!("{current_year}-");
        let in_current_year = txns
            .iter()
            .filter(|t| t.date.starts_with(&year_prefix))
            .count();
        assert!(
            in_current_year > 0,
            "should have transactions in the current year"
        );
    }

    #[test]
    fn test_generate_transactions_span_18_months() {
        let txns = generate_transactions();
        let dates: Vec<NaiveDate> = txns
            .iter()
            .map(|t| NaiveDate::parse_from_str(&t.date, "%Y-%m-%d").unwrap())
            .collect();
        let min_date = dates.iter().min().unwrap();
        let max_date = dates.iter().max().unwrap();
        let span_months = (max_date.year() - min_date.year()) * 12 + max_date.month() as i32
            - min_date.month() as i32;
        assert!(
            span_months >= 17,
            "transactions should span at least 17 months, got {span_months}"
        );
    }

    #[test]
    fn test_demo_creates_data() {
        let (_dir, conn) = test_db();
        let txn_count = insert_demo_data(&conn).unwrap();
        crate::db::set_metadata(&conn, "company_name", "Acme Consulting LLC").unwrap();
        let result = categorize_transactions(&conn).unwrap();

        let acct_count: i64 = conn
            .query_row("SELECT count(*) FROM accounts", [], |r| r.get(0))
            .unwrap();
        let db_txn_count: i64 = conn
            .query_row("SELECT count(*) FROM transactions", [], |r| r.get(0))
            .unwrap();
        let rule_count: i64 = conn
            .query_row("SELECT count(*) FROM rules", [], |r| r.get(0))
            .unwrap();

        assert_eq!(acct_count, 1);
        assert_eq!(db_txn_count, txn_count as i64);
        assert_eq!(rule_count, RULES.len() as i64);
        assert!(
            result.categorized > 0,
            "should categorize some transactions"
        );
        assert!(result.still_flagged > 0, "should leave some flagged");

        let company = crate::db::get_metadata(&conn, "company_name");
        assert_eq!(company.as_deref(), Some("Acme Consulting LLC"));
    }

    #[test]
    fn test_demo_ytd_income_nonzero() {
        let (_dir, conn) = test_db();
        insert_demo_data(&conn).unwrap();
        categorize_transactions(&conn).unwrap();

        let current_year = Local::now().date_naive().year();
        let ytd_income: f64 = conn
            .query_row(
                "SELECT COALESCE(SUM(amount), 0) FROM transactions WHERE amount > 0 AND date LIKE ?1",
                [format!("{current_year}%")],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            ytd_income > 0.0,
            "YTD income should be non-zero, got {ytd_income}"
        );
    }

    #[test]
    fn test_demo_idempotent() {
        let (_dir, conn) = test_db();
        insert_demo_data(&conn).unwrap();
        categorize_transactions(&conn).unwrap();

        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM accounts WHERE name = ?1)",
                [ACCOUNT_NAME],
                |r| r.get(0),
            )
            .unwrap();
        assert!(exists, "account should exist after first insert");

        let txn_count_before: i64 = conn
            .query_row("SELECT count(*) FROM transactions", [], |r| r.get(0))
            .unwrap();

        // Simulate what run() does: check guard, skip if exists
        if !exists {
            insert_demo_data(&conn).unwrap();
        }

        let txn_count_after: i64 = conn
            .query_row("SELECT count(*) FROM transactions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            txn_count_before, txn_count_after,
            "no duplicates on second run"
        );
    }

    #[test]
    fn test_total_balance_under_120k() {
        let txns = generate_transactions();
        let total: f64 = txns.iter().map(|t| t.amount).sum();
        assert!(
            total < 120_000.0,
            "total balance should be under $120k, got ${total:.2}"
        );
        assert!(
            total > 0.0,
            "total balance should be positive, got ${total:.2}"
        );
    }

    #[test]
    fn test_some_months_are_unprofitable() {
        let txns = generate_transactions();
        let today = Local::now().date_naive();

        // Group transactions by month and check net
        let mut unprofitable_months = 0;
        for i in 0..18u32 {
            let months_ago = 17 - i;
            let target = today - chrono::Months::new(months_ago);
            let ym = format!("{:04}-{:02}", target.year(), target.month());

            let month_net: f64 = txns
                .iter()
                .filter(|t| t.date.starts_with(&ym))
                .map(|t| t.amount)
                .sum();

            if month_net < 0.0 {
                unprofitable_months += 1;
            }
        }

        assert!(
            unprofitable_months >= 2,
            "at least 2 months should be unprofitable, got {unprofitable_months}"
        );
    }

    /// The invoicing screens open empty on a demo database without this, which
    /// is what 68.7 is fixing; the statuses are what make the list worth
    /// looking at.
    #[test]
    fn demo_seeds_clients_and_invoices_covering_the_statuses() {
        let (_dir, conn) = test_db();
        let (clients, invoices) = insert_demo_invoicing(&conn).unwrap();
        assert_eq!(clients, 3);
        assert_eq!(invoices, 4);

        let mut statuses: Vec<String> = conn
            .prepare("SELECT status FROM invoices ORDER BY number")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        statuses.sort();
        assert_eq!(statuses, ["draft", "paid", "partial", "sent"]);

        let payments: i64 = conn
            .query_row("SELECT COUNT(*) FROM invoice_payments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(payments, 2);
    }

    /// A seeded invoice with a payment link would be pulled at every launch by
    /// `sync_all_report`, against a Stripe account that has never heard of it.
    #[test]
    fn demo_invoices_carry_no_payment_link_so_the_launch_sync_ignores_them() {
        let (_dir, conn) = test_db();
        insert_demo_invoicing(&conn).unwrap();

        let syncable: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM invoices
                 WHERE stripe_payment_link_id IS NOT NULL AND status IN ('sent','partial','overdue')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(syncable, 0);
    }

    /// The two halves are two commits, so a failure between them is a real
    /// state. Keying the guard on the account alone would make it permanent.
    #[test]
    fn a_seed_that_stopped_after_the_transactions_is_finished_by_the_next_run() {
        let (_dir, conn) = test_db();
        insert_demo_data(&conn).unwrap();
        let transactions = demo_summary(&conn).unwrap().transactions;

        assert!(
            seed_demo(&conn).unwrap(),
            "the missing invoicing half was not seeded"
        );
        let after = demo_summary(&conn).unwrap();
        assert_eq!(after.clients, 3);
        assert_eq!(after.invoices, 4);
        assert_eq!(
            after.transactions, transactions,
            "the ledger half was seeded twice"
        );

        assert!(!seed_demo(&conn).unwrap(), "a complete seed is left alone");
        let again = demo_summary(&conn).unwrap();
        assert_eq!(again.invoices, 4);
        assert_eq!(again.transactions, transactions);
    }

    #[test]
    fn seed_demo_fills_an_empty_database_and_then_stops() {
        let (_dir, conn) = test_db();
        assert!(seed_demo(&conn).unwrap());
        let summary = demo_summary(&conn).unwrap();
        assert_eq!(summary.transactions, 18 * 15);
        assert_eq!(summary.rules, RULES.len() as i64);
        assert_eq!(summary.invoices, 4);
        assert!(summary.categorized > 0);
        assert!(summary.flagged > 0);
        assert_eq!(
            crate::db::get_metadata(&conn, "company_name").as_deref(),
            Some("Acme Consulting LLC")
        );

        assert!(!seed_demo(&conn).unwrap());
    }

    #[test]
    fn demo_invoice_dates_are_valid_and_sit_around_today() {
        let (_dir, conn) = test_db();
        insert_demo_invoicing(&conn).unwrap();
        let today = Local::now().date_naive();

        let mut stmt = conn
            .prepare("SELECT issue_date, due_date FROM invoices")
            .unwrap();
        let rows: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(rows.len(), 4);
        for (issue, due) in rows {
            let issue = NaiveDate::parse_from_str(&issue, "%Y-%m-%d").unwrap();
            let due = NaiveDate::parse_from_str(&due, "%Y-%m-%d").unwrap();
            assert!(issue <= today, "issued in the future: {issue}");
            assert_eq!(
                (due - issue).num_days(),
                30,
                "every demo invoice is Net 30: {issue} -> {due}"
            );
        }
    }

    #[test]
    fn test_dates_are_valid() {
        let txns = generate_transactions();
        for txn in &txns {
            let parsed = NaiveDate::parse_from_str(&txn.date, "%Y-%m-%d");
            assert!(parsed.is_ok(), "invalid date: {}", txn.date);
        }
    }
}
