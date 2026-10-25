use std::ffi::OsString;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;
use zeroize::{Zeroize, Zeroizing};

use crate::error::Result;
use crate::migrations;

/// The password lives for the process, and is replaced and cleared from more
/// than one thread; [`Zeroizing`] wipes every copy as it goes, so a replaced or
/// cleared password does not linger in freed heap.
static DB_PASSWORD: Mutex<Option<Zeroizing<String>>> = Mutex::new(None);

/// Set the global database password. Call before `get_connection()`.
pub fn set_db_password(password: Option<Zeroizing<String>>) {
    #[cfg(any(test, feature = "testutil"))]
    test_globals::hold_exclusive_on_test_thread();
    // unwrap: poisoned mutex means a thread panicked — unrecoverable
    *DB_PASSWORD.lock().unwrap() = password;
}

/// [`set_db_password`] for a test: takes the process-globals lock first.
#[cfg(any(test, feature = "testutil"))]
pub fn set_db_password_for_tests(password: Option<Zeroizing<String>>) {
    hold_process_globals_for_tests();
    set_db_password(password);
}

/// Read the current global database password.
pub fn get_db_password() -> Option<Zeroizing<String>> {
    #[cfg(any(test, feature = "testutil"))]
    test_globals::enter_shared();
    // unwrap: poisoned mutex means a thread panicked — unrecoverable
    DB_PASSWORD.lock().unwrap().clone()
}

/// Take the process-globals lock exclusively for the rest of the calling
/// test's thread. See [`test_globals`].
#[cfg(any(test, feature = "testutil"))]
pub fn hold_process_globals_for_tests() {
    test_globals::hold_exclusive();
}

/// Note that the calling test thread reads the process-global state, for the
/// rest of the thread. See [`test_globals`].
#[cfg(any(test, feature = "testutil"))]
pub fn read_process_globals_for_tests() {
    test_globals::enter_shared();
}

/// Isolation for the two pieces of process-global state tests touch: the
/// database password above and the config directory override in `settings`.
///
/// A test is either a reader, which only looks at the globals (every test that
/// opens a connection or loads settings), or a holder, which changes them. The
/// two are separated by one read-write lock taken per test thread and kept
/// until the thread exits:
///
/// - the first read of a global on a test thread takes the lock shared, so any
///   number of reading tests run together;
/// - a test that writes a global (`hold_process_globals_for_tests`,
///   `set_db_password`) takes it exclusively, waiting for every reading test
///   to finish and keeping later ones out until it is done, then puts the
///   password back to unset;
/// - a thread that holds the lock exclusively reads freely, and a thread that
///   is not a test thread (the workers a test's code spawns while the test
///   thread waits on them, whatever they are named) never takes the lock, so
///   a holder's own workers are never locked out of the state it set up.
///
/// A holder therefore never runs alongside any other test that reads the
/// globals, which is what lets a test put a password on the process without
/// breaking a plain-database test running next to it.
#[cfg(any(test, feature = "testutil"))]
mod test_globals {
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

    static LOCK: RwLock<()> = RwLock::new(());
    static EXCLUSIVE_HOLDERS: AtomicUsize = AtomicUsize::new(0);

    enum Held {
        None,
        Shared(#[allow(dead_code)] RwLockReadGuard<'static, ()>),
        Exclusive(#[allow(dead_code)] Exclusive),
    }

    struct Exclusive(#[allow(dead_code)] RwLockWriteGuard<'static, ()>);

    impl Drop for Exclusive {
        fn drop(&mut self) {
            *super::DB_PASSWORD.lock().unwrap_or_else(|e| e.into_inner()) = None;
            EXCLUSIVE_HOLDERS.fetch_sub(1, Ordering::SeqCst);
        }
    }

    thread_local! {
        static HELD: RefCell<Held> = const { RefCell::new(Held::None) };
    }

    /// libtest names each test thread after the test's module path, and runs
    /// tests on the thread named `main` when it is told to use one thread.
    /// Every other thread, a tokio worker or one a library names for itself,
    /// belongs to the code under test and must never wait on the lock that the
    /// test waiting on it holds.
    fn is_test_thread() -> bool {
        std::thread::current()
            .name()
            .is_some_and(|name| name == "main" || name.contains("::"))
    }

    pub(super) fn enter_shared() {
        if !is_test_thread() {
            return;
        }
        let needs_lock = HELD.with(|held| matches!(*held.borrow(), Held::None));
        if needs_lock {
            let guard = LOCK.read().unwrap_or_else(|e| e.into_inner());
            HELD.with(|held| *held.borrow_mut() = Held::Shared(guard));
        }
    }

    /// A setter on a test thread takes the lock itself; one on a worker thread
    /// can only be acting for a test that already holds it.
    pub(super) fn hold_exclusive_on_test_thread() {
        if is_test_thread() {
            hold_exclusive();
        } else {
            #[cfg(test)]
            assert!(
                EXCLUSIVE_HOLDERS.load(Ordering::SeqCst) > 0,
                "the database password was set from a worker thread by a test that never took \
                 hold_process_globals_for_tests()"
            );
        }
    }

    pub(super) fn hold_exclusive() {
        let already = HELD.with(|held| {
            let mut held = held.borrow_mut();
            match *held {
                Held::Exclusive(_) => true,
                Held::Shared(_) => {
                    *held = Held::None;
                    false
                }
                Held::None => false,
            }
        });
        if !already {
            let guard = LOCK.write().unwrap_or_else(|e| e.into_inner());
            EXCLUSIVE_HOLDERS.fetch_add(1, Ordering::SeqCst);
            HELD.with(|held| *held.borrow_mut() = Held::Exclusive(Exclusive(guard)));
        }
    }
}

pub const SCHEMA: &str = "
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

CREATE TABLE IF NOT EXISTS imports (
    id INTEGER PRIMARY KEY,
    filename TEXT NOT NULL,
    account_id INTEGER,
    import_date TEXT DEFAULT (datetime('now')),
    record_count INTEGER,
    date_range_start TEXT,
    date_range_end TEXT,
    checksum TEXT,
    malformed_count INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
);

CREATE TABLE IF NOT EXISTS import_rejects (
    id INTEGER PRIMARY KEY,
    import_id INTEGER NOT NULL,
    row_number INTEGER NOT NULL,
    content TEXT NOT NULL,
    reason TEXT NOT NULL,
    FOREIGN KEY (import_id) REFERENCES imports(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_import_rejects_import ON import_rejects(import_id);

CREATE TABLE IF NOT EXISTS transactions (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL,
    date TEXT NOT NULL,
    description TEXT NOT NULL,
    amount REAL NOT NULL,
    category_id INTEGER,
    vendor TEXT,
    notes TEXT,
    is_flagged INTEGER DEFAULT 0,
    flag_reason TEXT,
    import_id INTEGER,
    created_at TEXT DEFAULT (datetime('now')),
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    FOREIGN KEY (category_id) REFERENCES categories(id),
    FOREIGN KEY (import_id) REFERENCES imports(id)
);

CREATE TABLE IF NOT EXISTS rules (
    id INTEGER PRIMARY KEY,
    pattern TEXT NOT NULL,
    match_type TEXT DEFAULT 'contains',
    vendor TEXT,
    category_id INTEGER NOT NULL,
    priority INTEGER DEFAULT 0,
    hit_count INTEGER DEFAULT 0,
    is_active INTEGER DEFAULT 1,
    created_at TEXT DEFAULT (datetime('now')),
    FOREIGN KEY (category_id) REFERENCES categories(id)
);

CREATE TABLE IF NOT EXISTS reconciliations (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL,
    month TEXT NOT NULL,
    statement_balance REAL,
    calculated_balance REAL,
    is_reconciled INTEGER DEFAULT 0,
    reconciled_at TEXT,
    notes TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
);

CREATE TABLE IF NOT EXISTS metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

/// Metadata key recording which chart-of-accounts template a database was
/// created with. Absent on databases created before profiles existed, which
/// all carried the business chart.
pub const PROFILE_KEY: &str = "profile";

/// Which kind of books a database keeps: the business chart of accounts
/// (Schedule C / 1120-S) or a personal one with no tax mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Profile {
    #[default]
    Business,
    Personal,
}

impl Profile {
    pub fn as_str(self) -> &'static str {
        match self {
            Profile::Business => "business",
            Profile::Personal => "personal",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "business" => Some(Profile::Business),
            "personal" => Some(Profile::Personal),
            _ => None,
        }
    }
}

/// The profile a database keeps books under. Missing or unrecognized metadata
/// reads as business — the only chart that existed before profiles did.
pub fn get_profile(conn: &Connection) -> Profile {
    get_metadata(conn, PROFILE_KEY)
        .and_then(|value| Profile::parse(&value))
        .unwrap_or_default()
}

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

type CategoryDef = (
    &'static str,
    Option<i64>,
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
    &'static str,
    AccountClass,
);

const BUSINESS_CATEGORIES: &[CategoryDef] = &[
    // Income
    (
        "Client Services",
        None,
        "income",
        Some("Gross receipts"),
        Some("1120S-1a"),
        "Project fees, retainer payments",
        AccountClass::Revenue,
    ),
    (
        "Hosting & Maintenance",
        None,
        "income",
        Some("Gross receipts"),
        Some("1120S-1a"),
        "Recurring client hosting/maintenance fees",
        AccountClass::Revenue,
    ),
    (
        "Reimbursements",
        None,
        "income",
        Some("Gross receipts"),
        Some("1120S-1a"),
        "Client reimbursements for expenses",
        AccountClass::Revenue,
    ),
    (
        "Interest Income",
        None,
        "income",
        Some("Other income"),
        Some("K-4"),
        "Bank interest",
        AccountClass::Revenue,
    ),
    (
        "Other Income",
        None,
        "income",
        Some("Other income"),
        Some("1120S-5"),
        "Anything else",
        AccountClass::Revenue,
    ),
    // Expenses
    (
        "Cost of Goods Sold",
        None,
        "expense",
        Some("Schedule C Part III / 1120-S Line 2"),
        Some("1120S-2"),
        "Materials, subcontractor costs directly tied to delivering client work",
        AccountClass::Expense,
    ),
    (
        "Advertising & Marketing",
        None,
        "expense",
        Some("Line 8"),
        Some("1120S-16"),
        "Ads, sponsorships, marketing tools",
        AccountClass::Expense,
    ),
    (
        "Car & Truck",
        None,
        "expense",
        Some("Line 9"),
        Some("1120S-19"),
        "Mileage, fuel, parking",
        AccountClass::Expense,
    ),
    (
        "Commissions & Fees",
        None,
        "expense",
        Some("Line 10"),
        Some("1120S-19"),
        "Subcontractor commissions, platform fees",
        AccountClass::Expense,
    ),
    (
        "Contract Labor",
        None,
        "expense",
        Some("Line 11"),
        Some("1120S-19"),
        "Freelancers, subcontractors (1099 work)",
        AccountClass::Expense,
    ),
    (
        "Insurance",
        None,
        "expense",
        Some("Line 15"),
        Some("1120S-19"),
        "Business insurance, E&O",
        AccountClass::Expense,
    ),
    (
        "Legal & Professional",
        None,
        "expense",
        Some("Line 17"),
        Some("1120S-19"),
        "Accountant, lawyer, professional services",
        AccountClass::Expense,
    ),
    (
        "Office Expense",
        None,
        "expense",
        Some("Line 18"),
        Some("1120S-19"),
        "Office supplies, minor equipment",
        AccountClass::Expense,
    ),
    (
        "Rent / Lease",
        None,
        "expense",
        Some("Line 20b"),
        Some("1120S-11"),
        "Office rent, coworking",
        AccountClass::Expense,
    ),
    (
        "Software & Subscriptions",
        None,
        "expense",
        Some("Line 18/27a"),
        Some("1120S-19"),
        "SaaS tools, domain renewals, cloud services",
        AccountClass::Expense,
    ),
    (
        "Hosting & Infrastructure",
        None,
        "expense",
        Some("Line 18/27a"),
        Some("1120S-19"),
        "AWS, server costs, CDN",
        AccountClass::Expense,
    ),
    (
        "Taxes & Licenses",
        None,
        "expense",
        Some("Line 23"),
        Some("1120S-12"),
        "Business licenses, state fees",
        AccountClass::Expense,
    ),
    (
        "Travel",
        None,
        "expense",
        Some("Line 24a"),
        Some("1120S-19"),
        "Flights, hotels, conference travel",
        AccountClass::Expense,
    ),
    (
        "Meals",
        None,
        "expense",
        Some("Line 24b"),
        Some("1120S-19"),
        "Business meals (50% deductible)",
        AccountClass::Expense,
    ),
    (
        "Utilities",
        None,
        "expense",
        Some("Line 25"),
        Some("1120S-19"),
        "Internet, phone (business portion)",
        AccountClass::Expense,
    ),
    (
        "Payroll — Wages",
        None,
        "expense",
        Some("Line 26"),
        Some("1120S-8"),
        "Employee salaries (from Gusto)",
        AccountClass::Expense,
    ),
    (
        "Payroll — Taxes",
        None,
        "expense",
        Some("Line 23"),
        Some("1120S-12"),
        "Employer payroll taxes (from Gusto)",
        AccountClass::Expense,
    ),
    (
        "Payroll — Benefits",
        None,
        "expense",
        Some("Line 14"),
        Some("1120S-18"),
        "Health insurance, retirement (from Gusto)",
        AccountClass::Expense,
    ),
    (
        "Bank & Merchant Fees",
        None,
        "expense",
        Some("Line 27a"),
        Some("1120S-19"),
        "Stripe fees, bank charges, wire fees",
        AccountClass::Expense,
    ),
    (
        "Education & Training",
        None,
        "expense",
        Some("Line 27a"),
        Some("1120S-19"),
        "Courses, books, conferences",
        AccountClass::Expense,
    ),
    (
        "Equipment",
        None,
        "expense",
        Some("Line 13"),
        Some("1120S-19"),
        "Hardware, major purchases",
        AccountClass::Expense,
    ),
    (
        "Home Office",
        None,
        "expense",
        Some("Line 30"),
        Some("1120S-19"),
        "Simplified method or actual expenses",
        AccountClass::Expense,
    ),
    (
        OWNER_CONTRIBUTION,
        None,
        "income",
        Some("Not taxable"),
        None,
        "Money the owner puts into the business",
        AccountClass::Equity,
    ),
    (
        "Owner Draw / Distribution",
        None,
        "expense",
        Some("Not deductible"),
        Some("K-16d"),
        "Owner payments, distributions",
        AccountClass::Equity,
    ),
    (
        "Transfer",
        None,
        "expense",
        Some("Not deductible"),
        Some("excluded"),
        "Transfers between own accounts",
        AccountClass::Expense,
    ),
    (
        "Uncategorized",
        None,
        "expense",
        Some("\u{2014}"),
        None,
        "Needs review",
        AccountClass::Expense,
    ),
];

/// The personal chart of accounts. No `tax_line`/`form_line` mapping — those
/// belong to the business worksheets — except `Transfer`, whose `excluded`
/// marker is what keeps moves between own accounts out of the money reports.
const PERSONAL_CATEGORIES: &[CategoryDef] = &[
    // Income
    (
        "Salary & Wages",
        None,
        "income",
        None,
        None,
        "Paychecks and take-home pay",
        AccountClass::Revenue,
    ),
    (
        "Interest Income",
        None,
        "income",
        None,
        None,
        "Bank interest",
        AccountClass::Revenue,
    ),
    (
        "Other Income",
        None,
        "income",
        None,
        None,
        "Refunds, rebates, anything else",
        AccountClass::Revenue,
    ),
    // Expenses
    (
        "Rent / Mortgage",
        None,
        "expense",
        None,
        None,
        "Housing payments",
        AccountClass::Expense,
    ),
    (
        "Groceries",
        None,
        "expense",
        None,
        None,
        "Food and household staples",
        AccountClass::Expense,
    ),
    (
        "Dining & Takeout",
        None,
        "expense",
        None,
        None,
        "Restaurants, cafes, delivery",
        AccountClass::Expense,
    ),
    (
        "Utilities",
        None,
        "expense",
        None,
        None,
        "Electric, gas, water, internet, phone",
        AccountClass::Expense,
    ),
    (
        "Transportation",
        None,
        "expense",
        None,
        None,
        "Fuel, transit fares, parking, rideshare",
        AccountClass::Expense,
    ),
    (
        "Auto & Vehicle",
        None,
        "expense",
        None,
        None,
        "Car payments, repairs, registration",
        AccountClass::Expense,
    ),
    (
        "Health & Medical",
        None,
        "expense",
        None,
        None,
        "Doctors, dental, pharmacy",
        AccountClass::Expense,
    ),
    (
        "Insurance",
        None,
        "expense",
        None,
        None,
        "Home, auto, health premiums",
        AccountClass::Expense,
    ),
    (
        "Subscriptions & Streaming",
        None,
        "expense",
        None,
        None,
        "Streaming, apps, memberships",
        AccountClass::Expense,
    ),
    (
        "Shopping",
        None,
        "expense",
        None,
        None,
        "Clothing, electronics, general purchases",
        AccountClass::Expense,
    ),
    (
        "Home & Garden",
        None,
        "expense",
        None,
        None,
        "Furnishings, repairs, maintenance",
        AccountClass::Expense,
    ),
    (
        "Travel",
        None,
        "expense",
        None,
        None,
        "Flights, hotels, holidays",
        AccountClass::Expense,
    ),
    (
        "Entertainment",
        None,
        "expense",
        None,
        None,
        "Events, hobbies, going out",
        AccountClass::Expense,
    ),
    (
        "Education",
        None,
        "expense",
        None,
        None,
        "Tuition, courses, books",
        AccountClass::Expense,
    ),
    (
        "Childcare & Family",
        None,
        "expense",
        None,
        None,
        "Childcare, school costs, allowances",
        AccountClass::Expense,
    ),
    (
        "Pets",
        None,
        "expense",
        None,
        None,
        "Food, vet, supplies",
        AccountClass::Expense,
    ),
    (
        "Personal Care",
        None,
        "expense",
        None,
        None,
        "Haircuts, gym, wellness",
        AccountClass::Expense,
    ),
    (
        "Gifts & Donations",
        None,
        "expense",
        None,
        None,
        "Presents and charitable giving",
        AccountClass::Expense,
    ),
    (
        "Bank & Merchant Fees",
        None,
        "expense",
        None,
        None,
        "Account fees, card charges",
        AccountClass::Expense,
    ),
    (
        "Taxes",
        None,
        "expense",
        None,
        None,
        "Income and property tax payments",
        AccountClass::Expense,
    ),
    (
        "Transfer",
        None,
        "expense",
        None,
        Some("excluded"),
        "Transfers between own accounts, credit card payments",
        AccountClass::Expense,
    ),
    (
        "Uncategorized",
        None,
        "expense",
        None,
        None,
        "Needs review",
        AccountClass::Expense,
    ),
];

pub fn get_connection(db_path: &Path) -> Result<Connection> {
    let password = get_db_password();
    open_connection(db_path, password.as_ref().map(|p| p.as_str()))
}

/// Open a connection with an explicit password (bypasses global state).
/// Used by backup, password management, and tests.
pub fn open_connection(db_path: &Path, password: Option<&str>) -> Result<Connection> {
    let is_new = !db_path.exists();
    let conn = Connection::open(db_path)?;
    if is_new {
        crate::settings::restrict_file_permissions(db_path)?;
    }
    if let Some(pw) = password {
        conn.pragma_update(None, "key", pw)?;
    }
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    Ok(conn)
}

/// Check whether a database file is encrypted (requires a password to open).
/// Returns false for nonexistent files (they will be created fresh).
/// Detection uses the SQLite magic header: plaintext databases always start
/// with "SQLite format 3\0". Anything else is encrypted or corrupt.
pub fn is_encrypted(db_path: &Path) -> Result<bool> {
    if !db_path.exists() {
        return Ok(false);
    }
    let mut buf = [0u8; 16];
    let mut file = std::fs::File::open(db_path)?;
    use std::io::Read;
    let n = file.read(&mut buf)?;
    if n < 16 {
        return Ok(false); // too small to be a valid DB
    }
    Ok(&buf != b"SQLite format 3\0")
}

/// Test whether the given password can open the database at `db_path`.
/// The file must already exist. Callers should first confirm the database
/// is encrypted via `is_encrypted()`. Does not modify global password state.
///
/// Returns `Ok(true)` on success, `Ok(false)` for a wrong password
/// (SQLCipher's "not a database" error). All other errors are propagated.
pub fn validate_password(db_path: &Path, password: &str) -> Result<bool> {
    match open_connection(db_path, Some(password)) {
        Ok(_) => Ok(true),
        Err(crate::error::NigelError::Db(rusqlite::Error::SqliteFailure(err, _)))
            if err.code == rusqlite::ErrorCode::NotADatabase =>
        {
            Ok(false)
        }
        Err(e) => Err(e),
    }
}

/// Environment variable consulted for the database password whenever the
/// database is encrypted, so `nigel` can unlock one with no terminal attached.
/// Set but empty, invalid UTF-8, or wrong are all errors rather than a silent
/// fall back to prompting.
const PASSWORD_ENV_VAR: &str = "NIGEL_DB_PASSWORD";

/// Validate a password supplied through the environment, where `raw` is the
/// variable's value or `None` when unset. `Ok(None)` means nothing was supplied
/// and the caller should prompt.
///
/// `raw` is a parameter rather than read from the process environment here
/// because cargo runs tests as parallel threads of one process, which share
/// one environment and would clobber each other's setting.
fn env_password(db_path: &Path, raw: Option<OsString>) -> Result<Option<String>> {
    let Some(raw) = raw else {
        return Ok(None);
    };

    // Every failure below is fatal. A caller running unattended cannot answer the
    // prompt, so falling through to one would hang the job instead of reporting.
    let Ok(mut pw) = raw.into_string() else {
        return Err(crate::error::NigelError::Other(format!(
            "{PASSWORD_ENV_VAR} is set but is not valid UTF-8."
        )));
    };

    if pw.is_empty() {
        return Err(crate::error::NigelError::Other(format!(
            "{PASSWORD_ENV_VAR} is set but empty — the command that supplies it likely failed."
        )));
    }

    if validate_password(db_path, &pw)? {
        return Ok(Some(pw));
    }
    pw.zeroize();
    Err(did_not_unlock(PASSWORD_ENV_VAR, db_path))
}

/// A wrong password and a damaged file are indistinguishable here: SQLCipher
/// reports both as "not a database", and `is_encrypted` reads anything without
/// the plaintext header as encrypted. Naming one cause would send an operator
/// rotating a password when the ledger is actually corrupt.
fn did_not_unlock(var: &str, db_path: &Path) -> crate::error::NigelError {
    crate::error::NigelError::Other(format!(
        "{var} did not unlock {}. The password may be wrong, or the database file may be damaged.",
        db_path.display()
    ))
}

/// Environment variable naming a file that holds the database password — a
/// systemd credential, say — so the password itself never sits in the
/// environment. The file's content is the password, one trailing newline
/// stripped.
const PASSWORD_FILE_ENV_VAR: &str = "NIGEL_DB_PASSWORD_FILE";

/// The most a password file may hold. The file is one line of text; anything
/// larger is the wrong file, and reading it whole would pull an arbitrary
/// amount into memory.
const PASSWORD_FILE_MAX_BYTES: u64 = 4096;

/// Validate a password read from the file `path` names. Every failure is fatal,
/// for the reason [`env_password`]'s are, and none of them quotes the file.
fn file_password(db_path: &Path, path: &std::ffi::OsStr) -> Result<Option<String>> {
    use crate::error::NigelError;
    if path.is_empty() {
        return Err(NigelError::Other(format!(
            "{PASSWORD_FILE_ENV_VAR} is set but empty — it should name the file holding the password."
        )));
    }
    let path = Path::new(path);
    let could_not_read = |e: std::io::Error| {
        NigelError::Other(format!(
            "{PASSWORD_FILE_ENV_VAR} names {}, which could not be read: {e}.",
            path.display()
        ))
    };
    let mut bytes = Zeroizing::new(Vec::new());
    {
        use std::io::Read;
        let file = std::fs::File::open(path).map_err(could_not_read)?;
        file.take(PASSWORD_FILE_MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(could_not_read)?;
    }
    if bytes.len() as u64 > PASSWORD_FILE_MAX_BYTES {
        return Err(NigelError::Other(format!(
            "{PASSWORD_FILE_ENV_VAR} names {}, which is larger than {PASSWORD_FILE_MAX_BYTES} bytes. It should hold only the password.",
            path.display()
        )));
    }
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Err(NigelError::Other(format!(
            "{PASSWORD_FILE_ENV_VAR} names {}, which is not valid UTF-8.",
            path.display()
        )));
    };
    let text = match text.strip_suffix('\n') {
        Some(rest) => rest.strip_suffix('\r').unwrap_or(rest),
        None => text,
    };
    if text.is_empty() {
        return Err(NigelError::Other(format!(
            "{PASSWORD_FILE_ENV_VAR} names {}, which is empty.",
            path.display()
        )));
    }
    let mut pw = text.to_string();
    if validate_password(db_path, &pw)? {
        return Ok(Some(pw));
    }
    pw.zeroize();
    Err(did_not_unlock(PASSWORD_FILE_ENV_VAR, db_path))
}

/// The password from whichever of the two variables is set. Both set is an
/// error rather than a precedence rule: two sources for one secret is a
/// misconfiguration, and silently preferring one hides which is stale.
pub(crate) fn env_password_from(
    db_path: &Path,
    raw: Option<OsString>,
    file: Option<OsString>,
) -> Result<Option<String>> {
    match (raw, file) {
        (Some(_), Some(_)) => Err(crate::error::NigelError::Other(format!(
            "{PASSWORD_ENV_VAR} and {PASSWORD_FILE_ENV_VAR} are both set. Set one of them."
        ))),
        (None, Some(path)) => file_password(db_path, &path),
        (raw, None) => env_password(db_path, raw),
    }
}

/// Resolve a password for `db_path` from `NIGEL_DB_PASSWORD_FILE` or
/// `NIGEL_DB_PASSWORD`, if either is set. `Ok(None)` means neither is and the
/// caller should ask the user some other way — this function never touches
/// stdin.
pub fn env_password_if_set(db_path: &Path) -> Result<Option<String>> {
    env_password_from(
        db_path,
        std::env::var_os(PASSWORD_ENV_VAR),
        std::env::var_os(PASSWORD_FILE_ENV_VAR),
    )
}

pub fn init_db(conn: &Connection) -> Result<()> {
    init_db_with_profile(conn, Profile::Business)
}

/// `init_db`, but seeding the chart of accounts for the given profile.
///
/// The profile only matters on a database whose categories table is empty —
/// seeding and the profile stamp happen together, so re-running against an
/// existing database changes neither its chart nor its recorded profile.
pub fn init_db_with_profile(conn: &Connection, profile: Profile) -> Result<()> {
    conn.execute_batch(SCHEMA)?;

    let count: i64 = conn.query_row("SELECT count(*) FROM categories", [], |row| row.get(0))?;
    if count == 0 {
        let template = match profile {
            Profile::Business => BUSINESS_CATEGORIES,
            Profile::Personal => PERSONAL_CATEGORIES,
        };
        // One transaction: a failure partway must not leave a non-empty
        // categories table with no profile stamp, which would read as a
        // half-seeded business database forever.
        let tx = conn.unchecked_transaction()?;
        for cat in template {
            tx.execute(
                "INSERT INTO categories (name, parent_id, category_type, tax_line, form_line, description, class) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![cat.0, cat.1, cat.2, cat.3, cat.4, cat.5, cat.6.as_str()],
            )?;
        }
        set_metadata(&tx, PROFILE_KEY, profile.as_str())?;
        tx.commit()?;
    }

    migrations::run_migrations(conn)?;
    Ok(())
}

pub fn get_metadata(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM metadata WHERE key = ?1", [key], |row| {
        row.get(0)
    })
    .ok()
}

pub fn set_metadata(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO metadata (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

// Tests mutate the global DB_PASSWORD mutex and must run with --test-threads=1
// to avoid interference between tests. See also: cli::password::tests, cli::backup::tests.
#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        init_db(&conn).unwrap();
        (dir, conn)
    }

    #[test]
    fn test_init_db_creates_tables() {
        let (_dir, conn) = test_db();
        let tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        for expected in &[
            "accounts",
            "categories",
            "transactions",
            "rules",
            "imports",
            "reconciliations",
            "metadata",
        ] {
            assert!(
                tables.contains(&expected.to_string()),
                "missing table: {expected}"
            );
        }
    }

    #[test]
    fn test_init_db_is_idempotent() {
        let (_dir, conn) = test_db();
        init_db(&conn).unwrap();
    }

    #[test]
    fn test_init_db_seeds_categories() {
        let (_dir, conn) = test_db();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM categories", [], |r| r.get(0))
            .unwrap();
        assert!(count >= 29, "expected at least 29 categories, got {count}");
    }

    #[test]
    fn test_income_and_expense_categories() {
        let (_dir, conn) = test_db();
        let income: i64 = conn
            .query_row(
                "SELECT count(*) FROM categories WHERE category_type = 'income'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let expense: i64 = conn
            .query_row(
                "SELECT count(*) FROM categories WHERE category_type = 'expense'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(income >= 5, "expected >= 5 income categories, got {income}");
        assert!(
            expense >= 20,
            "expected >= 20 expense categories, got {expense}"
        );
    }

    #[test]
    fn test_init_db_stamps_business_profile() {
        let (_dir, conn) = test_db();
        assert_eq!(
            get_metadata(&conn, PROFILE_KEY).as_deref(),
            Some("business")
        );
        assert_eq!(get_profile(&conn), Profile::Business);
    }

    #[test]
    fn test_personal_profile_seeds_personal_chart() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("personal.db")).unwrap();
        init_db_with_profile(&conn, Profile::Personal).unwrap();

        assert_eq!(get_profile(&conn), Profile::Personal);
        let groceries: i64 = conn
            .query_row(
                "SELECT count(*) FROM categories WHERE name = 'Groceries'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(groceries, 1);
        let business_only: i64 = conn
            .query_row(
                "SELECT count(*) FROM categories WHERE name = 'Client Services'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(business_only, 0);
        // The excluded Transfer marker is what keeps transfers out of the
        // money reports, so the personal chart must carry it too.
        let transfer_form_line: Option<String> = conn
            .query_row(
                "SELECT form_line FROM categories WHERE name = 'Transfer'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(transfer_form_line.as_deref(), Some("excluded"));
        // No other personal category maps to a tax form.
        let mapped: i64 = conn
            .query_row(
                "SELECT count(*) FROM categories \
                 WHERE name <> 'Transfer' AND (tax_line IS NOT NULL OR form_line IS NOT NULL)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(mapped, 0);
    }

    #[test]
    fn test_reinit_does_not_change_profile_or_chart() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("personal.db")).unwrap();
        init_db_with_profile(&conn, Profile::Personal).unwrap();
        // The dispatch pre-flight calls plain init_db (business) on every
        // launch; it must not restamp or reseed an existing database.
        init_db(&conn).unwrap();
        assert_eq!(get_profile(&conn), Profile::Personal);
        let business_only: i64 = conn
            .query_row(
                "SELECT count(*) FROM categories WHERE name = 'Client Services'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(business_only, 0);
    }

    #[test]
    fn test_missing_profile_metadata_reads_as_business() {
        let (_dir, conn) = test_db();
        conn.execute("DELETE FROM metadata WHERE key = ?1", [PROFILE_KEY])
            .unwrap();
        assert_eq!(get_profile(&conn), Profile::Business);
        // Unrecognized values read as business too, rather than erroring.
        set_metadata(&conn, PROFILE_KEY, "cryptocurrency").unwrap();
        assert_eq!(get_profile(&conn), Profile::Business);
    }

    #[test]
    fn test_encrypted_db_cannot_open_without_password() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("encrypted.db");

        // Create encrypted DB
        set_db_password_for_tests(Some(zeroize::Zeroizing::new("secret".into())));
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        // Try opening without password — should fail
        set_db_password_for_tests(None);
        let result = get_connection(&db_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypted_db_opens_with_correct_password() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("encrypted.db");

        set_db_password_for_tests(Some(zeroize::Zeroizing::new("secret".into())));
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        // Reopen with same password
        set_db_password_for_tests(Some(zeroize::Zeroizing::new("secret".into())));
        let conn = get_connection(&db_path).unwrap();
        let result: i64 = conn
            .query_row("SELECT count(*) FROM categories", [], |r| r.get(0))
            .unwrap();
        assert!(result > 0);
    }

    #[test]
    fn test_unencrypted_db_works_without_password() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("plain.db");

        set_db_password_for_tests(None);
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        set_db_password_for_tests(None);
        let conn = get_connection(&db_path).unwrap();
        let result: i64 = conn
            .query_row("SELECT count(*) FROM categories", [], |r| r.get(0))
            .unwrap();
        assert!(result > 0);
    }

    #[test]
    fn test_is_encrypted_returns_false_for_plain_db() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("plain.db");
        set_db_password_for_tests(None);
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);
        assert!(!is_encrypted(&db_path).unwrap());
    }

    #[test]
    fn test_is_encrypted_returns_true_for_encrypted_db() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("encrypted.db");
        set_db_password_for_tests(Some(zeroize::Zeroizing::new("secret".into())));
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);
        set_db_password_for_tests(None);
        assert!(is_encrypted(&db_path).unwrap());
    }

    #[test]
    fn test_is_encrypted_returns_false_for_nonexistent_db() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("nope.db");
        assert!(!is_encrypted(&db_path).unwrap());
    }

    #[test]
    fn test_categories_have_form_line() {
        let (_dir, conn) = test_db();
        let form_line: Option<String> = conn
            .query_row(
                "SELECT form_line FROM categories WHERE name = 'Payroll — Wages'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(form_line.as_deref(), Some("1120S-8"));
    }

    #[test]
    fn test_validate_password_correct() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("encrypted.db");
        set_db_password_for_tests(Some(zeroize::Zeroizing::new("secret".into())));
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);
        set_db_password_for_tests(None);
        assert!(validate_password(&db_path, "secret").unwrap());
    }

    #[test]
    fn test_validate_password_wrong() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("encrypted.db");
        set_db_password_for_tests(Some(zeroize::Zeroizing::new("secret".into())));
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);
        set_db_password_for_tests(None);
        assert!(!validate_password(&db_path, "wrong").unwrap());
    }

    #[test]
    fn test_validate_password_nonexistent_file() {
        // validate_password on a missing file creates it via open_connection.
        // Callers must check existence first (dashboard.rs does db_path.exists()).
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("does_not_exist.db");
        let result = validate_password(&db_path, "anything");
        assert!(result.unwrap());
    }

    #[test]
    fn test_validate_password_non_database_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("garbage.db");
        std::fs::write(&path, b"this is not a database at all").unwrap();
        assert!(!validate_password(&path, "test").unwrap());
    }

    /// Build an encrypted database at a temp path locked with "secret".
    ///
    /// Uses `open_connection` with an explicit password rather than the global
    /// `DB_PASSWORD`: cargo runs these as parallel threads of one process, and a
    /// test that set the global would hand its password to every other test
    /// building a fixture through `get_connection`.
    fn encrypted_db() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("encrypted.db");
        let conn = open_connection(&db_path, Some("secret")).unwrap();
        init_db(&conn).unwrap();
        drop(conn);
        (dir, db_path)
    }

    #[test]
    fn test_env_password_unset_defers_to_prompt() {
        let (_dir, db_path) = encrypted_db();
        assert_eq!(env_password(&db_path, None).unwrap(), None);
    }

    #[test]
    fn test_env_password_correct_unlocks() {
        let (_dir, db_path) = encrypted_db();
        let resolved = env_password(&db_path, Some("secret".into())).unwrap();
        assert_eq!(resolved.as_deref(), Some("secret"));
    }

    #[test]
    fn test_env_password_wrong_errors_without_prompting() {
        let (_dir, db_path) = encrypted_db();
        let err = env_password(&db_path, Some("wrong".into())).unwrap_err();
        assert!(err.to_string().contains(PASSWORD_ENV_VAR));
    }

    #[test]
    fn test_env_password_empty_is_fatal_not_treated_as_unset() {
        let (_dir, db_path) = encrypted_db();
        let err = env_password(&db_path, Some(OsString::new()))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("empty"),
            "message should name the cause: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_env_password_invalid_utf8_is_fatal_not_treated_as_unset() {
        use std::os::unix::ffi::OsStringExt;
        let (_dir, db_path) = encrypted_db();
        let raw = OsString::from_vec(b"secret\xff".to_vec());
        let err = env_password(&db_path, Some(raw)).unwrap_err().to_string();
        assert!(
            err.contains("UTF-8"),
            "message should name the cause: {err}"
        );
    }

    #[test]
    fn test_env_password_error_does_not_leak_password() {
        let (_dir, db_path) = encrypted_db();
        let err = env_password(&db_path, Some("hunter2".into()))
            .unwrap_err()
            .to_string();
        assert!(!err.contains("hunter2"));
    }

    #[test]
    fn test_env_password_failure_names_both_possible_causes() {
        let (_dir, db_path) = encrypted_db();
        let err = env_password(&db_path, Some("wrong".into()))
            .unwrap_err()
            .to_string();
        assert!(err.contains("password may be wrong"), "{err}");
        assert!(err.contains("may be damaged"), "{err}");
        assert!(err.contains(&db_path.display().to_string()), "{err}");
    }

    fn password_file(dir: &Path, contents: &[u8]) -> OsString {
        let path = dir.join(format!("password-{}", hex::encode(contents)));
        std::fs::write(&path, contents).unwrap();
        path.into_os_string()
    }

    #[test]
    fn a_password_file_unlocks_with_one_trailing_newline_stripped() {
        let (dir, db_path) = encrypted_db();
        for contents in [&b"secret"[..], b"secret\n", b"secret\r\n"] {
            let file = password_file(dir.path(), contents);
            let resolved = env_password_from(&db_path, None, Some(file)).unwrap();
            assert_eq!(resolved.as_deref(), Some("secret"), "{contents:?}");
        }
    }

    #[test]
    fn only_one_newline_is_stripped() {
        let (dir, db_path) = encrypted_db();
        let file = password_file(dir.path(), b"secret\n\n");
        let err = env_password_from(&db_path, None, Some(file))
            .unwrap_err()
            .to_string();
        assert!(err.contains(PASSWORD_FILE_ENV_VAR), "{err}");
    }

    #[test]
    fn both_variables_set_is_an_error_naming_both() {
        let (dir, db_path) = encrypted_db();
        let file = password_file(dir.path(), b"secret");
        let err = env_password_from(&db_path, Some("secret".into()), Some(file))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains(PASSWORD_ENV_VAR) && err.contains(PASSWORD_FILE_ENV_VAR),
            "{err}"
        );
    }

    #[test]
    fn a_missing_empty_or_wrong_password_file_is_fatal_and_quotes_nothing() {
        let (dir, db_path) = encrypted_db();
        let missing = dir.path().join("absent").into_os_string();
        let cases = [
            (OsString::new(), "empty"),
            (missing, "could not be read"),
            (password_file(dir.path(), b"\n"), "is empty"),
            (password_file(dir.path(), b"hunter2\n"), "may be damaged"),
        ];
        for (file, expected) in cases {
            let err = env_password_from(&db_path, None, Some(file))
                .unwrap_err()
                .to_string();
            assert!(err.contains(PASSWORD_FILE_ENV_VAR), "{err}");
            assert!(err.contains(expected), "{expected}: {err}");
            assert!(!err.contains("hunter2"), "{err}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_password_file_that_is_not_utf8_is_fatal() {
        let (dir, db_path) = encrypted_db();
        let file = password_file(dir.path(), b"secret\xff");
        let err = env_password_from(&db_path, None, Some(file))
            .unwrap_err()
            .to_string();
        assert!(err.contains("UTF-8"), "{err}");
    }

    #[test]
    fn a_password_file_larger_than_the_cap_is_refused_without_quoting_it() {
        let (dir, db_path) = encrypted_db();
        let big = dir.path().join("big-password");
        std::fs::write(&big, vec![b'a'; PASSWORD_FILE_MAX_BYTES as usize + 1]).unwrap();
        let err = env_password_from(&db_path, None, Some(big.into_os_string()))
            .unwrap_err()
            .to_string();
        assert!(err.contains(PASSWORD_FILE_ENV_VAR), "{err}");
        assert!(err.contains("larger than 4096 bytes"), "{err}");
        assert!(!err.contains("aaaa"), "{err}");

        let at_cap = dir.path().join("at-cap-password");
        std::fs::write(&at_cap, vec![b'a'; PASSWORD_FILE_MAX_BYTES as usize]).unwrap();
        let err = env_password_from(&db_path, None, Some(at_cap.into_os_string()))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("may be damaged"),
            "a file at the cap is read: {err}"
        );
    }

    #[test]
    fn neither_variable_set_defers_to_the_prompt() {
        let (_dir, db_path) = encrypted_db();
        assert_eq!(env_password_from(&db_path, None, None).unwrap(), None);
    }

    #[test]
    fn test_init_db_sets_schema_version() {
        let (_dir, conn) = test_db();
        let version = crate::migrations::get_schema_version(&conn).unwrap();
        assert_eq!(version, crate::migrations::LATEST_VERSION);
    }

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
}
