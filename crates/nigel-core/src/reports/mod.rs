pub mod text;

use chrono::Datelike;
use rusqlite::Connection;
use serde::Serialize;

use crate::db::AccountClass;
use crate::error::{NigelError, Result};

/// What a build without the `pdf` feature says when asked for a PDF. Shared
/// with the HTTP export endpoints so the CLI and the API explain the same
/// missing feature the same way.
pub const PDF_DISABLED_MESSAGE: &str =
    "PDF export requires the 'pdf' feature — build with `cargo build --features pdf`";

/// The default basename of an exported report: the report's slug and the day it
/// was exported, with the extension left to the caller. Used for the CLI's
/// output paths and for the filename the HTTP download suggests.
pub fn export_file_stem(name: &str) -> String {
    let date = chrono::Local::now().format("%Y-%m-%d");
    format!("{name}-{date}")
}

/// Parse a `YYYY-MM` month string into its year and month parts. Anything
/// else — absent, malformed, or a month that fails to parse as a number —
/// answers `(None, None)` rather than erroring, matching the lenient
/// `--month` handling every report and the register browser share.
pub fn parse_month_opt(month: &Option<String>) -> (Option<i32>, Option<u32>) {
    if let Some(m) = month {
        let parts: Vec<&str> = m.split('-').collect();
        if parts.len() == 2 {
            let year = parts[0].parse().ok();
            let month = parts[1].parse().ok();
            return (year, month);
        }
    }
    (None, None)
}

/// The period label for a register: "2026-03", "FY 2026", or "All dates" when
/// no date filter was given. An unfiltered register shows every transaction,
/// so its label must say so — `reports::date_range_label` instead labels a
/// missing year as the current FY, matching the other report views'
/// current-year default. Built from the parsed values `get_register` is
/// actually asked with, so a `--month` that failed to parse (and therefore
/// filtered nothing) is labelled "All dates", never echoed as a period.
pub fn register_range_label(year: Option<i32>, month: Option<u32>) -> String {
    match (year, month) {
        (Some(y), Some(m)) => format!("{y}-{m:02}"),
        (Some(y), None) => format!("FY {y}"),
        (None, _) => "All dates".to_string(),
    }
}

/// Subtitle for a register report: the period followed by any active non-date filters.
pub fn register_subtitle(range: &str, filters: &RegisterFilters) -> String {
    let labels = filters.labels();
    if labels.is_empty() {
        range.to_string()
    } else {
        format!("{range} — {}", labels.join(", "))
    }
}

// ---------------------------------------------------------------------------
// Report identity and date granularity
// ---------------------------------------------------------------------------

/// What date navigation granularities a report supports.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DateGranularity {
    /// Supports both month and year navigation (P&L, Expenses, Cash Flow)
    MonthAndYear,
    /// Supports only year navigation (Tax, K-1)
    YearOnly,
    /// No date navigation (Flagged, Balance)
    None,
}

/// The set of reports Nigel can produce, independent of how they are requested.
#[derive(Clone, Copy, PartialEq)]
pub enum ReportKind {
    Pnl,
    Expenses,
    Tax,
    Cashflow,
    Register,
    Flagged,
    Balance,
    Aging,
    K1,
    /// Bulk export of every report; not a report in its own right.
    All,
}

impl ReportKind {
    /// Stable slug used for CLI subcommand names and export filenames.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pnl => "pnl",
            Self::Expenses => "expenses",
            Self::Tax => "tax",
            Self::Cashflow => "cashflow",
            Self::Register => "register",
            Self::Flagged => "flagged",
            Self::Balance => "balance",
            Self::Aging => "aging",
            Self::K1 => "k1-prep",
            Self::All => "all",
        }
    }

    pub fn granularity(&self) -> DateGranularity {
        match self {
            Self::Pnl | Self::Expenses | Self::Cashflow | Self::Register => {
                DateGranularity::MonthAndYear
            }
            Self::Tax | Self::K1 | Self::All => DateGranularity::YearOnly,
            Self::Flagged | Self::Balance | Self::Aging => DateGranularity::None,
        }
    }
}

/// The period label printed under a report's title: the month itself when one
/// was asked for, otherwise the fiscal year, otherwise the current one.
///
/// `month` is the raw `YYYY-MM` string rather than a parsed month so the label
/// reads the way the caller asked for it, and `year` is the effective year —
/// an explicit year already resolved against the one inside `month`.
pub fn date_range_label(month: Option<&str>, year: Option<i32>) -> String {
    if let Some(month) = month {
        return month.to_string();
    }
    let year = year.unwrap_or_else(|| Datelike::year(&chrono::Local::now()));
    format!("FY {year}")
}

/// SQL predicate keeping money-movement categories (`form_line = 'excluded'`,
/// e.g. the stock `Transfer` category) out of income-statement math. A transfer
/// between own accounts is not income or spending, so P&L, the expense
/// breakdown, and cash flow skip those rows; the register and per-account
/// balances keep them, because per account the cash really moved. Expects the
/// categories table aliased as `c`; NULL-safe so uncategorized rows pass.
const EXCLUDE_TRANSFERS: &str = "COALESCE(c.form_line, '') <> 'excluded'";

fn to_sql_params(params: &[String]) -> Vec<&dyn rusqlite::types::ToSql> {
    params
        .iter()
        .map(|p| p as &dyn rusqlite::types::ToSql)
        .collect()
}

// ---------------------------------------------------------------------------
// Date filter helper
// ---------------------------------------------------------------------------

pub(crate) fn date_filter(
    year: Option<i32>,
    month: Option<u32>,
    from_date: Option<&str>,
    to_date: Option<&str>,
) -> Result<(String, Vec<String>)> {
    if let (Some(from), Some(to)) = (from_date, to_date) {
        return Ok((
            "t.date BETWEEN ?1 AND ?2".to_string(),
            vec![from.to_string(), to.to_string()],
        ));
    }
    if from_date.is_some() {
        return Err(crate::error::NigelError::Other(
            "--from requires --to (both date boundaries must be specified)".to_string(),
        ));
    }
    if to_date.is_some() {
        return Err(crate::error::NigelError::Other(
            "--to requires --from (both date boundaries must be specified)".to_string(),
        ));
    }
    if let (Some(y), Some(m)) = (year, month) {
        let prefix = format!("{y:04}-{m:02}");
        return Ok(("t.date LIKE ?1".to_string(), vec![format!("{prefix}%")]));
    }
    if let Some(y) = year {
        return Ok(("t.date LIKE ?1".to_string(), vec![format!("{y}%")]));
    }
    // Default: all transactions (no date filter)
    Ok(("1=1".to_string(), vec![]))
}

// ---------------------------------------------------------------------------
// P&L
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PnlItem {
    pub name: String,
    pub total: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PnlReport {
    pub income: Vec<PnlItem>,
    pub expenses: Vec<PnlItem>,
    pub total_income: f64,
    pub total_expenses: f64,
    pub net: f64,
}

pub fn get_pnl(
    conn: &Connection,
    year: Option<i32>,
    month: Option<u32>,
    from_date: Option<&str>,
    to_date: Option<&str>,
) -> Result<PnlReport> {
    let (clause, params) = date_filter(year, month, from_date, to_date)?;

    let income =
        query_category_totals(conn, &clause, &params, AccountClass::Revenue, "total DESC")?;
    let expenses =
        query_category_totals(conn, &clause, &params, AccountClass::Expense, "total ASC")?;

    let total_income: f64 = income.iter().map(|i| i.total).sum();
    let total_expenses: f64 = expenses.iter().map(|i| i.total).sum();

    Ok(PnlReport {
        income,
        expenses,
        total_income,
        total_expenses,
        net: total_income + total_expenses,
    })
}

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
    let mut stmt = conn.prepare(&sql)?;
    let param_values = to_sql_params(params);
    let rows = stmt.query_map(param_values.as_slice(), |row| {
        Ok(PnlItem {
            name: row.get(0)?,
            total: row.get(1)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

// ---------------------------------------------------------------------------
// Expense Breakdown
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseItem {
    pub name: String,
    pub total: f64,
    pub count: i64,
    pub pct: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VendorItem {
    pub vendor: String,
    pub total: f64,
    pub count: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseBreakdown {
    pub categories: Vec<ExpenseItem>,
    pub total: f64,
    pub top_vendors: Vec<VendorItem>,
}

pub fn get_expense_breakdown(
    conn: &Connection,
    year: Option<i32>,
    month: Option<u32>,
) -> Result<ExpenseBreakdown> {
    // Custom date ranges (--from/--to) not supported here; expense breakdown
    // is scoped by year/month only, matching the CLI subcommand interface.
    let (clause, params) = date_filter(year, month, None, None)?;
    let class = AccountClass::Expense.as_str();

    let sql = format!(
        "SELECT c.name, SUM(t.amount) as total, COUNT(*) as count \
         FROM transactions t JOIN categories c ON t.category_id = c.id \
         WHERE {clause} AND c.class = '{class}' AND {EXCLUDE_TRANSFERS} \
         GROUP BY c.name ORDER BY total ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let param_values = to_sql_params(&params);
    let raw: Vec<(String, f64, i64)> = stmt
        .query_map(param_values.as_slice(), |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let total: f64 = raw.iter().map(|(_, t, _)| t).sum();
    let categories = raw
        .iter()
        .map(|(name, t, c)| ExpenseItem {
            name: name.clone(),
            total: *t,
            count: *c,
            pct: if total != 0.0 { t / total * 100.0 } else { 0.0 },
        })
        .collect();

    let vendor_sql = format!(
        "SELECT t.vendor, SUM(t.amount) as total, COUNT(*) as count \
         FROM transactions t JOIN categories c ON t.category_id = c.id \
         WHERE {clause} AND c.class = '{class}' AND t.vendor IS NOT NULL \
           AND {EXCLUDE_TRANSFERS} \
         GROUP BY t.vendor ORDER BY total ASC LIMIT 10"
    );
    let mut vstmt = conn.prepare(&vendor_sql)?;
    let top_vendors: Vec<VendorItem> = vstmt
        .query_map(param_values.as_slice(), |row| {
            Ok(VendorItem {
                vendor: row.get(0)?,
                total: row.get(1)?,
                count: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(ExpenseBreakdown {
        categories,
        total,
        top_vendors,
    })
}

// ---------------------------------------------------------------------------
// Tax Summary
// ---------------------------------------------------------------------------

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxSummary {
    pub line_items: Vec<TaxItem>,
}

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
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
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

// ---------------------------------------------------------------------------
// Cash Flow
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CashflowMonth {
    pub month: String,
    pub inflows: f64,
    pub outflows: f64,
    pub net: f64,
    pub running_balance: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CashflowReport {
    pub months: Vec<CashflowMonth>,
}

pub fn get_cashflow(
    conn: &Connection,
    year: Option<i32>,
    month: Option<u32>,
) -> Result<CashflowReport> {
    let (clause, params) = date_filter(year, month, None, None)?;

    // Transfers are excluded from both the flows and the running balance: when
    // both legs are imported they cancel anyway, and when one is not, counting
    // the lone leg would report a move between own accounts as spending.
    let sql = format!(
        "SELECT substr(t.date, 1, 7) as month, \
         SUM(CASE WHEN t.amount > 0 THEN t.amount ELSE 0 END) as inflows, \
         SUM(CASE WHEN t.amount < 0 THEN t.amount ELSE 0 END) as outflows \
         FROM transactions t LEFT JOIN categories c ON t.category_id = c.id \
         WHERE {clause} AND {EXCLUDE_TRANSFERS} \
         GROUP BY substr(t.date, 1, 7) ORDER BY month"
    );
    let mut stmt = conn.prepare(&sql)?;
    let param_values = to_sql_params(&params);
    let raw: Vec<(String, f64, f64)> = stmt
        .query_map(param_values.as_slice(), |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // When filtered to a single month, seed the running balance with the
    // cumulative total from prior months in that year so users see the
    // correct year-to-date cash position, not just that month's net.
    let prior_balance = if let (Some(y), Some(m)) = (year, month) {
        if m > 1 {
            let end = format!("{y:04}-{m:02}");
            conn.query_row(
                &format!(
                    "SELECT COALESCE(SUM(t.amount), 0) \
                     FROM transactions t LEFT JOIN categories c ON t.category_id = c.id \
                     WHERE t.date >= ?1 AND t.date < ?2 AND {EXCLUDE_TRANSFERS}"
                ),
                rusqlite::params![format!("{y:04}-01"), end],
                |row| row.get::<_, f64>(0),
            )?
        } else {
            0.0
        }
    } else {
        0.0
    };

    let mut months = Vec::new();
    let mut running = prior_balance;
    for (m, inflows, outflows) in raw {
        running += inflows + outflows;
        months.push(CashflowMonth {
            month: m,
            inflows,
            outflows,
            net: inflows + outflows,
            running_balance: running,
        });
    }

    Ok(CashflowReport { months })
}

// ---------------------------------------------------------------------------
// Register (all transactions)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRow {
    pub id: i64,
    pub date: String,
    pub description: String,
    pub amount: f64,
    pub category: Option<String>,
    pub category_id: Option<i64>,
    pub vendor: Option<String>,
    pub account_name: String,
    pub is_flagged: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterReport {
    pub rows: Vec<RegisterRow>,
    pub total: f64,
}

/// Which categories a register selection covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CategorySelection {
    /// `name` is the display name of the category row `id` refers to. The two
    /// halves come out of one row in `RegisterFilters::resolve`, and the variant
    /// is `non_exhaustive` so that stays the only way to get one: the id drives
    /// the SQL and the name goes in the report header, so a pair that disagrees
    /// prints one category's name over another category's rows.
    #[non_exhaustive]
    Named {
        id: i64,
        name: String,
    },
    Uncategorized,
}

impl CategorySelection {
    /// A named selection assembled without a database, for tests in crates that
    /// depend on this one. Not compiled into a normal build: production callers
    /// go through `RegisterFilters::resolve`, which is what makes the id and the
    /// name describe the same row.
    #[cfg(any(test, feature = "testutil"))]
    pub fn named_for_test(id: i64, name: impl Into<String>) -> Self {
        Self::Named {
            id,
            name: name.into(),
        }
    }
}

/// Non-date filters applied to a register selection. Carries the display names
/// so report headers can describe the selection. The category was validated at
/// construction; the account deliberately was not — an unknown account is an
/// empty register, matching what `--account` has always done.
///
/// The fields are private so the validated category cannot be swapped for an
/// unvalidated one after the fact. Both ways to build one carry a category the
/// database agreed to: `resolve` reads it out, and `for_account` sets none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegisterFilters {
    account: Option<String>,
    category: Option<CategorySelection>,
}

impl RegisterFilters {
    /// Filter on an account and nothing else.
    ///
    /// What the HTTP register routes want: they refuse category filters by name
    /// in `ReportParams::parse`, so the category half is always empty and there
    /// is nothing to validate against the database.
    pub fn for_account(account: Option<String>) -> Self {
        Self {
            account,
            category: None,
        }
    }

    /// Add an already-resolved category selection.
    ///
    /// Test-only, like `CategorySelection::named_for_test`: no production caller
    /// wants one. The CLI resolves its filters from user input, and the HTTP
    /// routes refuse category filters by name, so `resolve` and `for_account`
    /// cover every real path.
    #[cfg(any(test, feature = "testutil"))]
    pub fn with_category(mut self, category: CategorySelection) -> Self {
        self.category = Some(category);
        self
    }

    /// Resolve raw CLI arguments, validating the category name against the
    /// database and resolving it to an id; the account passes through
    /// unvalidated. Only active categories match: an inactive one has zero
    /// transactions by construction (`delete_blocker` refuses otherwise), and
    /// its name may since have been reused by a live category — binding to the
    /// dead row would silently answer an empty register for a name with data.
    pub fn resolve(
        conn: &Connection,
        account: Option<String>,
        category: Option<String>,
        uncategorized: bool,
    ) -> Result<Self> {
        let category = match (category, uncategorized) {
            (Some(_), true) => {
                // Clap refuses this combination at the CLI; a programmatic
                // caller gets the same answer rather than a silent preference.
                return Err(crate::error::NigelError::Invalid(
                    "--category and --uncategorized are mutually exclusive".into(),
                ));
            }
            (Some(name), false) => {
                let id = match conn.query_row(
                    "SELECT id FROM categories WHERE name = ?1 AND is_active = 1",
                    [&name],
                    |row| row.get(0),
                ) {
                    Ok(id) => id,
                    Err(rusqlite::Error::QueryReturnedNoRows) => {
                        return Err(crate::error::NigelError::UnknownCategory(name))
                    }
                    Err(e) => return Err(e.into()),
                };
                Some(CategorySelection::Named { id, name })
            }
            (None, true) => Some(CategorySelection::Uncategorized),
            (None, false) => None,
        };
        Ok(Self { account, category })
    }

    /// Human-readable filter labels for report headers: `["account: BofA Checking"]`
    pub fn labels(&self) -> Vec<String> {
        let mut labels = Vec::new();
        if let Some(ref account) = self.account {
            labels.push(format!("account: {account}"));
        }
        match self.category {
            Some(CategorySelection::Named { ref name, .. }) => {
                labels.push(format!("category: {name}"))
            }
            Some(CategorySelection::Uncategorized) => labels.push("uncategorized".to_string()),
            None => {}
        }
        labels
    }
}

/// Filename-safe fragments describing a register selection, used to name default
/// exports. Takes the raw arguments so callers can name the file without first
/// opening the database to validate them.
///
/// A filter whose name slugs to nothing (all punctuation, a non-Latin script)
/// still contributes a `filtered` fragment: dropping it would give a filtered
/// export the unfiltered register's default filename, silently overwriting a
/// same-day unfiltered export.
pub fn register_slug_parts(
    account: Option<&str>,
    category: Option<&str>,
    uncategorized: bool,
) -> Vec<String> {
    let slug_or_placeholder = |name: &str| {
        let slug = crate::fmt::slugify(name);
        if slug.is_empty() {
            "filtered".to_string()
        } else {
            slug
        }
    };
    let mut parts = Vec::new();
    if let Some(account) = account {
        parts.push(slug_or_placeholder(account));
    }
    if let Some(category) = category {
        parts.push(slug_or_placeholder(category));
    }
    if uncategorized {
        parts.push("uncategorized".to_string());
    }
    parts
}

pub fn get_register(
    conn: &Connection,
    year: Option<i32>,
    month: Option<u32>,
    from_date: Option<&str>,
    to_date: Option<&str>,
    filters: &RegisterFilters,
) -> Result<RegisterReport> {
    let (clause, mut params) = date_filter(year, month, from_date, to_date)?;

    let account_clause = if let Some(ref acc) = filters.account {
        params.push(acc.clone());
        format!(" AND a.name = ?{}", params.len())
    } else {
        String::new()
    };

    let category_clause = match filters.category {
        Some(CategorySelection::Named { id, .. }) => {
            params.push(id.to_string());
            format!(" AND t.category_id = ?{}", params.len())
        }
        Some(CategorySelection::Uncategorized) => " AND t.category_id IS NULL".to_string(),
        None => String::new(),
    };

    let sql = format!(
        "SELECT t.id, t.date, t.description, t.amount, c.name, t.category_id, t.vendor, a.name, t.is_flagged \
         FROM transactions t \
         JOIN accounts a ON t.account_id = a.id \
         LEFT JOIN categories c ON t.category_id = c.id \
         WHERE {clause}{account_clause}{category_clause} \
         ORDER BY t.date, t.id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let param_values = to_sql_params(&params);
    let rows: Vec<RegisterRow> = stmt
        .query_map(param_values.as_slice(), |row| {
            Ok(RegisterRow {
                id: row.get(0)?,
                date: row.get(1)?,
                description: row.get(2)?,
                amount: row.get(3)?,
                category: row.get(4)?,
                category_id: row.get(5)?,
                vendor: row.get(6)?,
                account_name: row.get(7)?,
                is_flagged: row.get(8)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let total: f64 = rows.iter().map(|r| r.amount).sum();
    Ok(RegisterReport { rows, total })
}

/// One register row by id — the shape an edited transaction answers with, so
/// the row a client sends back is the row it already knows how to render.
pub fn get_register_row(conn: &Connection, id: i64) -> Result<RegisterRow> {
    conn.query_row(
        "SELECT t.id, t.date, t.description, t.amount, c.name, t.category_id, t.vendor, a.name, t.is_flagged \
         FROM transactions t \
         JOIN accounts a ON t.account_id = a.id \
         LEFT JOIN categories c ON t.category_id = c.id \
         WHERE t.id = ?1",
        [id],
        |row| {
            Ok(RegisterRow {
                id: row.get(0)?,
                date: row.get(1)?,
                description: row.get(2)?,
                amount: row.get(3)?,
                category: row.get(4)?,
                category_id: row.get(5)?,
                vendor: row.get(6)?,
                account_name: row.get(7)?,
                is_flagged: row.get(8)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            NigelError::NotFound(format!("No transaction found with ID {id}"))
        }
        other => NigelError::Db(other),
    })
}

// ---------------------------------------------------------------------------
// Flagged
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlaggedTransaction {
    pub id: i64,
    pub date: String,
    pub description: String,
    pub amount: f64,
    pub account_name: String,
}

pub fn get_flagged(conn: &Connection) -> Result<Vec<FlaggedTransaction>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.date, t.description, t.amount, a.name as account_name \
         FROM transactions t JOIN accounts a ON t.account_id = a.id \
         WHERE t.is_flagged = 1 ORDER BY t.date",
    )?;
    let rows: Vec<FlaggedTransaction> = stmt
        .query_map([], |row| {
            Ok(FlaggedTransaction {
                id: row.get(0)?,
                date: row.get(1)?,
                description: row.get(2)?,
                amount: row.get(3)?,
                account_name: row.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Balance
// ---------------------------------------------------------------------------

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceReport {
    pub accounts: Vec<AccountBalance>,
    pub total: f64,
    pub ytd_net_income: f64,
}

pub fn get_balance(conn: &Connection) -> Result<BalanceReport> {
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

    let total: f64 = accounts.iter().map(|a| a.balance).sum();

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

    Ok(BalanceReport {
        accounts,
        total,
        ytd_net_income,
    })
}

// ---------------------------------------------------------------------------
// K-1 Prep Report
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct K1LineItem {
    pub form_line: String,
    pub category_name: String,
    pub total: f64,
}

#[allow(dead_code)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct K1OtherDeduction {
    pub category_name: String,
    pub total: f64,
    pub deductible: f64,
}

#[allow(dead_code)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct K1Validation {
    pub uncategorized_count: i64,
    pub officer_comp: f64,
    pub distributions: f64,
    pub comp_dist_ratio: Option<f64>,
}

#[allow(dead_code)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct K1PrepReport {
    pub gross_receipts: f64,
    pub cogs: f64,
    pub gross_profit: f64,
    pub other_income: f64,
    pub total_deductions: f64,
    pub ordinary_business_income: f64,
    pub deduction_lines: Vec<K1LineItem>,
    pub schedule_k_items: Vec<K1LineItem>,
    pub other_deductions: Vec<K1OtherDeduction>,
    pub other_deductions_total: f64,
    pub auto_mapped: Vec<String>,
    pub unmapped: Vec<K1LineItem>,
    pub validation: K1Validation,
}

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

pub fn get_k1_prep(conn: &Connection, year: Option<i32>) -> Result<K1PrepReport> {
    let (clause, params) = date_filter(year, None, None, None)?;

    // Query all categorized transactions grouped by category
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

    let mut gross_receipts = 0.0f64;
    let mut cogs = 0.0f64;
    let mut other_income = 0.0f64;
    let mut total_deductions = 0.0f64;
    let mut deduction_lines = Vec::new();
    let mut schedule_k_items = Vec::new();
    let mut other_deductions = Vec::new();
    let mut other_deductions_total = 0.0f64;
    let mut officer_comp = 0.0f64;
    let mut distributions = 0.0f64;
    let mut auto_mapped = Vec::new();
    let mut unmapped = Vec::new();

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
                gross_receipts += total;
                auto_mapped.push(name.clone());
                continue;
            }
            K1Mapping::Unmapped => {
                unmapped.push(K1LineItem {
                    form_line: "—".to_string(),
                    category_name: name.clone(),
                    total: total.abs(),
                });
                continue;
            }
            K1Mapping::Explicit(fl) => fl,
        };
        match line.as_str() {
            "1120S-1a" => gross_receipts += total,
            "1120S-2" => cogs += total.abs(),
            "1120S-5" => other_income += total,
            fl if fl.starts_with("K-") => {
                schedule_k_items.push(K1LineItem {
                    form_line: line.clone(),
                    category_name: name.clone(),
                    total: *total,
                });
            }
            fl if fl.starts_with("1120S-") => {
                let abs_total = total.abs();

                if fl == "1120S-7" || fl == "1120S-8" {
                    officer_comp += abs_total;
                }

                deduction_lines.push(K1LineItem {
                    form_line: line.clone(),
                    category_name: name.clone(),
                    total: abs_total,
                });

                let deductible = if fl == "1120S-19" {
                    let is_meals = name.to_lowercase().contains("meal");
                    let d = if is_meals { abs_total * 0.5 } else { abs_total };
                    other_deductions_total += d;
                    other_deductions.push(K1OtherDeduction {
                        category_name: name.clone(),
                        total: abs_total,
                        deductible: d,
                    });
                    d
                } else {
                    abs_total
                };
                total_deductions += deductible;
            }
            _ => unmapped.push(K1LineItem {
                form_line: line.clone(),
                category_name: name.clone(),
                total: total.abs(),
            }),
        }
    }

    let gross_profit = gross_receipts - cogs;
    let ordinary_business_income = gross_profit + other_income - total_deductions;

    // Validation: count uncategorized transactions
    let uncategorized_sql = format!(
        "SELECT COUNT(*) FROM transactions t WHERE {clause} AND t.category_id IS NULL",
        clause = clause
    );
    let mut ustmt = conn.prepare(&uncategorized_sql)?;
    let uncategorized_count: i64 = ustmt.query_row(param_values.as_slice(), |row| row.get(0))?;

    let comp_dist_ratio = if distributions > 0.0 {
        Some(officer_comp / distributions)
    } else {
        None
    };

    Ok(K1PrepReport {
        gross_receipts,
        cogs,
        gross_profit,
        other_income,
        total_deductions,
        ordinary_business_income,
        deduction_lines,
        schedule_k_items,
        other_deductions,
        other_deductions_total,
        auto_mapped,
        unmapped,
        validation: K1Validation {
            uncategorized_count,
            officer_comp,
            distributions,
            comp_dist_ratio,
        },
    })
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

    fn account_filter(name: &str) -> RegisterFilters {
        RegisterFilters {
            account: Some(name.to_string()),
            category: None,
        }
    }

    #[test]
    fn a_month_labels_itself_and_outranks_the_year() {
        assert_eq!(date_range_label(Some("2025-03"), None), "2025-03");
        assert_eq!(date_range_label(Some("2025-03"), Some(2024)), "2025-03");
    }

    #[test]
    fn a_year_alone_labels_a_fiscal_year() {
        assert_eq!(date_range_label(None, Some(2025)), "FY 2025");
    }

    #[test]
    fn an_unfiltered_report_is_labelled_with_the_current_year() {
        let this_year = Datelike::year(&chrono::Local::now());
        assert_eq!(date_range_label(None, None), format!("FY {this_year}"));
    }

    #[test]
    fn get_register_row_matches_the_full_register() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')",
            [],
        )
        .unwrap();
        let account = conn.last_insert_rowid();
        let category: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id, vendor, is_flagged) \
             VALUES (?1, '2025-01-15', 'ADOBE', -50.0, ?2, 'Adobe', 1)",
            rusqlite::params![account, category],
        )
        .unwrap();
        let id = conn.last_insert_rowid();

        let row = get_register_row(&conn, id).unwrap();
        let from_report =
            get_register(&conn, None, None, None, None, &RegisterFilters::default()).unwrap();
        let expected = &from_report.rows[0];
        assert_eq!(row.id, expected.id);
        assert_eq!(row.category, expected.category);
        assert_eq!(row.vendor, expected.vendor);
        assert_eq!(row.account_name, expected.account_name);
        assert_eq!(row.is_flagged, expected.is_flagged);

        let err = get_register_row(&conn, 4242).unwrap_err();
        assert!(matches!(err, NigelError::NotFound(_)), "got: {err}");
    }

    /// Seed one account with income, an expense, and both legs of a transfer
    /// categorized under the stock `Transfer` category (`form_line = 'excluded'`).
    fn db_with_transfer() -> (tempfile::TempDir, Connection, i64) {
        let (dir, conn) = test_db();
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Checking', 'checking')",
            [],
        )
        .unwrap();
        let account = conn.last_insert_rowid();
        let income: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Client Services'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let software: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let transfer: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Transfer'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let rows: [(&str, &str, f64, i64); 4] = [
            ("2025-01-10", "CLIENT PAYMENT", 5000.0, income),
            ("2025-02-05", "ADOBE", -50.0, software),
            ("2025-01-20", "TRANSFER TO SAVINGS", -2000.0, transfer),
            ("2025-02-14", "TRANSFER FROM SAVINGS", 500.0, transfer),
        ];
        for (date, desc, amount, category) in rows {
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, category_id, vendor) \
                 VALUES (?1, ?2, ?3, ?4, ?5, 'Bank')",
                rusqlite::params![account, date, desc, amount, category],
            )
            .unwrap();
        }
        (dir, conn, account)
    }

    #[test]
    fn pnl_excludes_transfer_categories() {
        let (_dir, conn, _) = db_with_transfer();
        let report = get_pnl(&conn, Some(2025), None, None, None).unwrap();
        assert_eq!(report.total_income, 5000.0);
        assert_eq!(report.total_expenses, -50.0);
        assert_eq!(report.net, 4950.0);
        assert!(
            !report.expenses.iter().any(|item| item.name == "Transfer"),
            "Transfer must not appear as an expense line"
        );
        assert!(!report.income.iter().any(|item| item.name == "Transfer"));
    }

    #[test]
    fn expense_breakdown_excludes_transfer_categories() {
        let (_dir, conn, _) = db_with_transfer();
        let report = get_expense_breakdown(&conn, Some(2025), None).unwrap();
        assert_eq!(report.total, -50.0);
        assert!(!report.categories.iter().any(|item| item.name == "Transfer"));
        // The vendor rollup shares the exclusion: 'Bank' would otherwise carry
        // the transfer leg's -2000 into the top-vendors table.
        let bank: f64 = report
            .top_vendors
            .iter()
            .filter(|v| v.vendor == "Bank")
            .map(|v| v.total)
            .sum();
        assert_eq!(bank, -50.0);
    }

    #[test]
    fn cashflow_excludes_transfer_categories() {
        let (_dir, conn, _) = db_with_transfer();
        let report = get_cashflow(&conn, Some(2025), None).unwrap();
        let jan = report.months.iter().find(|m| m.month == "2025-01").unwrap();
        assert_eq!(jan.inflows, 5000.0);
        assert_eq!(jan.outflows, 0.0, "the transfer leg is not an outflow");
        let feb = report.months.iter().find(|m| m.month == "2025-02").unwrap();
        assert_eq!(feb.inflows, 0.0, "the returning leg is not an inflow");
        assert_eq!(feb.outflows, -50.0);
        assert_eq!(feb.running_balance, 4950.0);
    }

    #[test]
    fn cashflow_prior_balance_excludes_transfer_categories() {
        let (_dir, conn, _) = db_with_transfer();
        // A single-month view seeds its running balance from the year's prior
        // months, which must apply the same exclusion as the months themselves.
        let report = get_cashflow(&conn, Some(2025), Some(2)).unwrap();
        let feb = report.months.iter().find(|m| m.month == "2025-02").unwrap();
        assert_eq!(feb.running_balance, 4950.0);
    }

    #[test]
    fn balance_report_keeps_transfers_but_ytd_net_income_skips_them() {
        let (_dir, conn, account) = db_with_transfer();
        // Current-year rows so ytd_net_income (which is always year-to-date of
        // the running year) has something to add up.
        let year = chrono::Local::now().year();
        let transfer: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Transfer'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let income: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Client Services'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, ?2, 'RETAINER', 1000.0, ?3)",
            rusqlite::params![account, format!("{year}-01-05"), income],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, ?2, 'TRANSFER OUT', -300.0, ?3)",
            rusqlite::params![account, format!("{year}-01-06"), transfer],
        )
        .unwrap();

        let report = get_balance(&conn).unwrap();
        // Per account the cash really moved, so the account balance includes
        // every transfer leg even though the income statements skip them.
        let checking = report
            .accounts
            .iter()
            .find(|a| a.name == "Checking")
            .unwrap();
        assert_eq!(
            checking.balance,
            5000.0 - 50.0 - 2000.0 + 500.0 + 1000.0 - 300.0
        );
        // The fixture's dated rows only join the YTD figure when the suite
        // happens to run in their calendar year.
        let expected_ytd = if year == 2025 {
            1000.0 + 4950.0
        } else {
            1000.0
        };
        assert_eq!(report.ytd_net_income, expected_ytd);
    }

    #[test]
    fn report_kind_slugs_and_granularity() {
        use DateGranularity::*;
        use ReportKind::*;

        let expected = [
            (Pnl, "pnl", MonthAndYear),
            (Expenses, "expenses", MonthAndYear),
            (Tax, "tax", YearOnly),
            (Cashflow, "cashflow", MonthAndYear),
            (Register, "register", MonthAndYear),
            (Flagged, "flagged", None),
            (Balance, "balance", None),
            (Aging, "aging", None),
            (K1, "k1-prep", YearOnly),
            (All, "all", YearOnly),
        ];

        for (kind, slug, granularity) in expected {
            assert_eq!(kind.as_str(), slug);
            assert_eq!(kind.granularity(), granularity, "granularity for {slug}");
        }
    }

    #[test]
    fn date_granularity_serializes_camel_case() {
        // 31.5 wraps every report as { granularity, report } and the SPA
        // switches on these exact strings.
        assert_eq!(
            serde_json::to_value(DateGranularity::MonthAndYear).unwrap(),
            serde_json::json!("monthAndYear")
        );
        assert_eq!(
            serde_json::to_value(DateGranularity::YearOnly).unwrap(),
            serde_json::json!("yearOnly")
        );
        assert_eq!(
            serde_json::to_value(DateGranularity::None).unwrap(),
            serde_json::json!("none")
        );
    }

    #[test]
    fn pnl_report_serializes_camel_case() {
        let report = PnlReport {
            income: vec![PnlItem {
                name: "Client Services".to_string(),
                total: 5000.0,
            }],
            expenses: vec![PnlItem {
                name: "Software".to_string(),
                total: -250.0,
            }],
            total_income: 5000.0,
            total_expenses: -250.0,
            net: 4750.0,
        };

        let value = serde_json::to_value(&report).unwrap();
        let obj = value.as_object().unwrap();

        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["expenses", "income", "net", "totalExpenses", "totalIncome"]
        );
        assert!(!obj.contains_key("total_income"));
        assert!(!obj.contains_key("total_expenses"));

        assert_eq!(value["totalIncome"], 5000.0);
        assert_eq!(value["income"][0]["name"], "Client Services");
        assert_eq!(value["expenses"][0]["total"], -250.0);
    }

    #[test]
    fn k1_prep_report_serializes_camel_case() {
        let report = K1PrepReport {
            gross_receipts: 100_000.0,
            cogs: 10_000.0,
            gross_profit: 90_000.0,
            other_income: 500.0,
            total_deductions: 40_000.0,
            ordinary_business_income: 50_500.0,
            deduction_lines: vec![K1LineItem {
                form_line: "1120S-7".to_string(),
                category_name: "Officer Compensation".to_string(),
                total: -30_000.0,
            }],
            schedule_k_items: vec![K1LineItem {
                form_line: "K-16d".to_string(),
                category_name: "Distributions".to_string(),
                total: -15_000.0,
            }],
            other_deductions: vec![K1OtherDeduction {
                category_name: "Meals".to_string(),
                total: -1_000.0,
                deductible: -500.0,
            }],
            other_deductions_total: -500.0,
            auto_mapped: vec!["Consulting".to_string()],
            unmapped: vec![K1LineItem {
                form_line: String::new(),
                category_name: "Misc".to_string(),
                total: -25.0,
            }],
            validation: K1Validation {
                uncategorized_count: 3,
                officer_comp: -30_000.0,
                distributions: -15_000.0,
                comp_dist_ratio: Some(2.0),
            },
        };

        let value = serde_json::to_value(&report).unwrap();

        assert_eq!(value["grossReceipts"], 100_000.0);
        assert_eq!(value["grossProfit"], 90_000.0);
        assert_eq!(value["otherIncome"], 500.0);
        assert_eq!(value["totalDeductions"], 40_000.0);
        assert_eq!(value["ordinaryBusinessIncome"], 50_500.0);
        assert_eq!(value["otherDeductionsTotal"], -500.0);
        assert_eq!(value["deductionLines"][0]["formLine"], "1120S-7");
        assert_eq!(
            value["deductionLines"][0]["categoryName"],
            "Officer Compensation"
        );
        assert_eq!(value["scheduleKItems"][0]["formLine"], "K-16d");
        assert_eq!(value["otherDeductions"][0]["deductible"], -500.0);
        assert_eq!(value["autoMapped"][0], "Consulting");
        assert_eq!(value["unmapped"][0]["categoryName"], "Misc");
        assert_eq!(value["validation"]["uncategorizedCount"], 3);
        assert_eq!(value["validation"]["officerComp"], -30_000.0);
        assert_eq!(value["validation"]["compDistRatio"], 2.0);

        assert!(value.as_object().unwrap().keys().all(|k| !k.contains('_')));

        let unset_ratio = K1Validation {
            uncategorized_count: 0,
            officer_comp: 0.0,
            distributions: 0.0,
            comp_dist_ratio: None,
        };
        let value = serde_json::to_value(unset_ratio).unwrap();
        assert!(value["compDistRatio"].is_null());
    }

    fn seed_transactions(conn: &Connection) {
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')",
            [],
        )
        .unwrap();
        let acct = conn.last_insert_rowid();
        let income_cat: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Client Services'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let expense_cat: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-15', 'Client payment', 1000.0, ?2)",
            rusqlite::params![acct, income_cat],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-20', 'Adobe CC', -50.0, ?2)",
            rusqlite::params![acct, expense_cat],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-02-10', 'GitHub', -10.0, ?2)",
            rusqlite::params![acct, expense_cat],
        )
        .unwrap();
    }

    #[test]
    fn test_pnl_ytd() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report = get_pnl(&conn, Some(2025), None, None, None).unwrap();
        // seed_transactions: 1×1000.0 income, 2 expenses (−50.0 + −10.0 = −60.0)
        assert_eq!(report.total_income, 1000.0);
        assert_eq!(report.total_expenses, -60.0);
        assert_eq!(report.net, 940.0);
    }

    #[test]
    fn test_pnl_by_month() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report = get_pnl(&conn, Some(2025), Some(1), None, None).unwrap();
        // seed_transactions Jan only: 1×1000.0 income, 1×−50.0 expense (GitHub −10.0 is Feb)
        assert_eq!(report.total_income, 1000.0);
        assert_eq!(report.total_expenses, -50.0);
        assert_eq!(report.net, 950.0);
    }

    #[test]
    fn test_expense_breakdown() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let breakdown = get_expense_breakdown(&conn, Some(2025), None).unwrap();
        // seed_transactions: 2 expenses in "Software & Subscriptions" (−50.0 + −10.0)
        assert_eq!(breakdown.categories.len(), 1);
        assert_eq!(breakdown.categories[0].name, "Software & Subscriptions");
        assert_eq!(breakdown.categories[0].count, 2);
        assert_eq!(breakdown.total, -60.0);
    }

    #[test]
    fn test_register_returns_all_transactions() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report = get_register(
            &conn,
            Some(2025),
            None,
            None,
            None,
            &RegisterFilters::default(),
        )
        .unwrap();
        assert_eq!(report.rows.len(), 3);
        // First two are categorized, all should appear
        assert!(report.rows.iter().all(|r| r.category.is_some()));
    }

    #[test]
    fn test_register_default_returns_all_years() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn); // 2025 transactions
                                  // Add a transaction in a different year
        let acct: i64 = conn
            .query_row("SELECT id FROM accounts WHERE name = 'Test'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let cat: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Client Services'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2024-06-15', 'Old payment', 500.0, ?2)",
            rusqlite::params![acct, cat],
        )
        .unwrap();
        // No date filters — should return all 4 transactions across both years
        let report =
            get_register(&conn, None, None, None, None, &RegisterFilters::default()).unwrap();
        assert_eq!(report.rows.len(), 4);
        assert_eq!(report.rows[0].date, "2024-06-15"); // oldest first
    }

    #[test]
    fn test_register_shows_uncategorized() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')",
            [],
        )
        .unwrap();
        let acct = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) \
             VALUES (?1, '2025-01-15', 'UNKNOWN VENDOR', -99.0, 1, 'No matching rule')",
            rusqlite::params![acct],
        )
        .unwrap();
        let report = get_register(
            &conn,
            Some(2025),
            None,
            None,
            None,
            &RegisterFilters::default(),
        )
        .unwrap();
        assert_eq!(report.rows.len(), 1);
        assert!(report.rows[0].category.is_none());
    }

    #[test]
    fn test_register_account_filter() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report =
            get_register(&conn, Some(2025), None, None, None, &account_filter("Test")).unwrap();
        assert_eq!(report.rows.len(), 3);
        let report = get_register(
            &conn,
            Some(2025),
            None,
            None,
            None,
            &account_filter("Nonexistent"),
        )
        .unwrap();
        assert_eq!(report.rows.len(), 0);
    }

    #[test]
    fn test_register_category_filter() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let filters =
            RegisterFilters::resolve(&conn, None, Some("Software & Subscriptions".into()), false)
                .unwrap();
        let report = get_register(&conn, Some(2025), None, None, None, &filters).unwrap();
        assert_eq!(report.rows.len(), 2);
        assert!(report
            .rows
            .iter()
            .all(|r| r.category.as_deref() == Some("Software & Subscriptions")));
    }

    #[test]
    fn test_register_category_filter_composes_with_account_and_dates() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let filters = RegisterFilters::resolve(
            &conn,
            Some("Test".into()),
            Some("Software & Subscriptions".into()),
            false,
        )
        .unwrap();
        let report = get_register(&conn, Some(2025), None, None, None, &filters).unwrap();
        assert_eq!(report.rows.len(), 2);

        // Same category on a different account selects nothing.
        let filters = RegisterFilters::resolve(
            &conn,
            Some("Nonexistent".into()),
            Some("Software & Subscriptions".into()),
            false,
        )
        .unwrap();
        let report = get_register(&conn, Some(2025), None, None, None, &filters).unwrap();
        assert_eq!(report.rows.len(), 0);

        // Same category in a year with no transactions selects nothing.
        let filters =
            RegisterFilters::resolve(&conn, None, Some("Software & Subscriptions".into()), false)
                .unwrap();
        let report = get_register(&conn, Some(2019), None, None, None, &filters).unwrap();
        assert_eq!(report.rows.len(), 0);
    }

    #[test]
    fn test_register_uncategorized_filter() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let acct: i64 = conn
            .query_row("SELECT id FROM accounts WHERE name = 'Test'", [], |r| {
                r.get(0)
            })
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount) \
             VALUES (?1, '2025-04-01', 'UNKNOWN VENDOR', -12.0)",
            rusqlite::params![acct],
        )
        .unwrap();

        let filters = RegisterFilters::resolve(&conn, None, None, true).unwrap();
        let report = get_register(&conn, Some(2025), None, None, None, &filters).unwrap();
        assert_eq!(report.rows.len(), 1);
        assert_eq!(report.rows[0].description, "UNKNOWN VENDOR");
        assert!(report.rows[0].category.is_none());
    }

    #[test]
    fn test_register_filters_reject_unknown_category() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let err = RegisterFilters::resolve(&conn, None, Some("Nope".into()), false).unwrap_err();
        assert!(
            matches!(err, crate::error::NigelError::UnknownCategory(ref name) if name == "Nope"),
            "expected UnknownCategory, got {err:?}"
        );
    }

    #[test]
    fn test_register_filters_propagate_real_db_errors() {
        let (_dir, conn) = test_db();
        conn.execute("DROP TABLE categories", []).unwrap();
        let err =
            RegisterFilters::resolve(&conn, None, Some("Anything".into()), false).unwrap_err();
        assert!(
            !matches!(err, crate::error::NigelError::UnknownCategory(_)),
            "a missing table must not be reported as an unknown category name"
        );
    }

    #[test]
    fn test_register_filter_labels_and_slugs() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let filters = RegisterFilters::resolve(
            &conn,
            Some("BofA Checking".into()),
            Some("Software & Subscriptions".into()),
            false,
        )
        .unwrap();
        assert_eq!(
            filters.labels(),
            vec![
                "account: BofA Checking".to_string(),
                "category: Software & Subscriptions".to_string()
            ]
        );

        let uncategorized = RegisterFilters::resolve(&conn, None, None, true).unwrap();
        assert_eq!(uncategorized.labels(), vec!["uncategorized".to_string()]);
        assert!(RegisterFilters::default().labels().is_empty());

        assert_eq!(
            register_slug_parts(Some("BofA Checking"), Some("Taxes & Licenses"), false),
            vec!["bofa-checking".to_string(), "taxes-licenses".to_string()]
        );
        assert_eq!(
            register_slug_parts(None, None, true),
            vec!["uncategorized".to_string()]
        );
        assert!(register_slug_parts(None, None, false).is_empty());
        // A name that slugifies to nothing still marks the export as filtered:
        // dropping it would hand this export the unfiltered register's default
        // filename, silently overwriting a same-day unfiltered export.
        assert_eq!(
            register_slug_parts(Some("!!!"), None, false),
            vec!["filtered".to_string()]
        );
        assert_eq!(
            register_slug_parts(Some("現金"), Some("Rent"), false),
            vec!["filtered".to_string(), "rent".to_string()]
        );
    }

    #[test]
    fn test_register_uncategorized_filter_composes_with_account() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Other', 'checking')",
            [],
        )
        .unwrap();
        let other = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount) \
             VALUES (?1, '2025-04-01', 'OTHER MYSTERY', -5.0)",
            rusqlite::params![other],
        )
        .unwrap();
        let acct: i64 = conn
            .query_row("SELECT id FROM accounts WHERE name = 'Test'", [], |r| {
                r.get(0)
            })
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount) \
             VALUES (?1, '2025-04-02', 'TEST MYSTERY', -7.0)",
            rusqlite::params![acct],
        )
        .unwrap();

        let filters = RegisterFilters::resolve(&conn, Some("Test".into()), None, true).unwrap();
        let report = get_register(&conn, Some(2025), None, None, None, &filters).unwrap();
        assert_eq!(report.rows.len(), 1);
        assert_eq!(report.rows[0].description, "TEST MYSTERY");
    }

    #[test]
    fn test_register_filters_reject_category_and_uncategorized_together() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        // Clap refuses this combination at the CLI; the data layer must give a
        // programmatic caller the same answer, not silently prefer one filter.
        let err = RegisterFilters::resolve(&conn, None, Some("Rent".into()), true).unwrap_err();
        assert!(
            matches!(err, crate::error::NigelError::Invalid(_)),
            "expected Invalid, got {err:?}"
        );
    }

    #[test]
    fn test_register_filters_match_exact_case_and_only_active_categories() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);

        // The name match is exact, like account matching.
        let err =
            RegisterFilters::resolve(&conn, None, Some("software & subscriptions".into()), false)
                .unwrap_err();
        assert!(matches!(err, crate::error::NigelError::UnknownCategory(_)));

        // An inactive category no longer matches: it has zero transactions by
        // construction, and its name may since have been reused. With a dead
        // row shadowing a live one, the filter must bind to the live id.
        conn.execute(
            "INSERT INTO categories (name, category_type, is_active) \
             VALUES ('Ghost', 'expense', 0)",
            [],
        )
        .unwrap();
        let err = RegisterFilters::resolve(&conn, None, Some("Ghost".into()), false).unwrap_err();
        assert!(matches!(err, crate::error::NigelError::UnknownCategory(_)));

        conn.execute(
            "INSERT INTO categories (name, category_type, is_active) \
             VALUES ('Ghost', 'expense', 1)",
            [],
        )
        .unwrap();
        let live = conn.last_insert_rowid();
        let filters = RegisterFilters::resolve(&conn, None, Some("Ghost".into()), false).unwrap();
        assert_eq!(
            filters.category,
            Some(CategorySelection::Named {
                id: live,
                name: "Ghost".into()
            })
        );
    }

    #[test]
    fn test_k1_prep_basic() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report = get_k1_prep(&conn, Some(2025)).unwrap();
        assert!(report.gross_receipts >= 0.0);
        assert!(report.total_deductions >= 0.0);
        // Software & Subscriptions → 1120S-19 → should appear in deduction_lines
        let sw = report
            .deduction_lines
            .iter()
            .find(|d| d.category_name == "Software & Subscriptions");
        assert!(
            sw.is_some(),
            "Software & Subscriptions should appear in deduction_lines"
        );
        assert_eq!(sw.unwrap().total, 60.0); // abs of -60
        assert_eq!(report.validation.uncategorized_count, 0);
    }

    #[test]
    fn test_date_filter_rejects_from_without_to() {
        let (_dir, conn) = test_db();
        let result = get_pnl(&conn, None, None, Some("2025-01-01"), None);
        assert!(result.is_err());
        let msg = result.err().unwrap().to_string();
        assert!(msg.contains("--from requires --to"), "got: {msg}");
    }

    #[test]
    fn test_date_filter_rejects_to_without_from() {
        let (_dir, conn) = test_db();
        let result = get_pnl(&conn, None, None, None, Some("2025-12-31"));
        assert!(result.is_err());
        let msg = result.err().unwrap().to_string();
        assert!(msg.contains("--to requires --from"), "got: {msg}");
    }

    #[test]
    fn test_date_filter_accepts_both_from_and_to() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        // Jan range captures: 1×1000.0 income, 1×−50.0 expense (GitHub −10.0 is Feb)
        let report = get_pnl(&conn, None, None, Some("2025-01-01"), Some("2025-01-31")).unwrap();
        assert_eq!(report.total_income, 1000.0);
        assert_eq!(report.total_expenses, -50.0);
    }

    #[test]
    fn test_k1_meals_50_pct() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')",
            [],
        )
        .unwrap();
        let acct = conn.last_insert_rowid();
        let meals_cat: i64 = conn
            .query_row("SELECT id FROM categories WHERE name = 'Meals'", [], |r| {
                r.get(0)
            })
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-03-15', 'Business lunch', -100.0, ?2)",
            rusqlite::params![acct, meals_cat],
        )
        .unwrap();
        let report = get_k1_prep(&conn, Some(2025)).unwrap();
        let meals = report
            .other_deductions
            .iter()
            .find(|d| d.category_name == "Meals");
        assert!(meals.is_some(), "Meals should appear in other_deductions");
        let m = meals.unwrap();
        assert_eq!(m.total, 100.0);
        assert_eq!(m.deductible, 50.0); // 50% deductible
    }

    #[test]
    fn test_k1_other_income_sign_handling() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')",
            [],
        )
        .unwrap();
        let acct = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO categories (name, category_type, form_line) \
             VALUES ('Test Other Income', 'income', '1120S-5')",
            [],
        )
        .unwrap();
        let cat_id = conn.last_insert_rowid();
        // Refunds exceed income: net is negative
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-10', 'Misc income', 50.0, ?2)",
            rusqlite::params![acct, cat_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-15', 'Refund', -200.0, ?2)",
            rusqlite::params![acct, cat_id],
        )
        .unwrap();
        let report = get_k1_prep(&conn, Some(2025)).unwrap();
        // SUM = 50 + (-200) = -150 — net negative surfaces as-is
        assert_eq!(report.other_income, -150.0);
    }

    #[test]
    fn test_k1_gross_receipts_sign_handling() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')",
            [],
        )
        .unwrap();
        let acct = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO categories (name, category_type, form_line) \
             VALUES ('Test Gross Receipts', 'income', '1120S-1a')",
            [],
        )
        .unwrap();
        let cat_id = conn.last_insert_rowid();
        // Refunds exceed income: net is negative
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-10', 'Invoice', 100.0, ?2)",
            rusqlite::params![acct, cat_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-15', 'Refund', -300.0, ?2)",
            rusqlite::params![acct, cat_id],
        )
        .unwrap();
        let report = get_k1_prep(&conn, Some(2025)).unwrap();
        // SUM = 100 + (-300) = -200 — net negative surfaces as-is
        assert_eq!(report.gross_receipts, -200.0);
    }

    #[test]
    fn test_resolve_k1_mapping() {
        use K1Mapping::*;
        assert_eq!(
            resolve_k1_mapping(Some("excluded"), AccountClass::Expense),
            Excluded
        );
        assert_eq!(
            resolve_k1_mapping(Some("excluded"), AccountClass::Revenue),
            Excluded
        );
        assert_eq!(
            resolve_k1_mapping(Some("1120S-19"), AccountClass::Expense),
            Explicit("1120S-19".into())
        );
        assert_eq!(
            resolve_k1_mapping(Some("K-16d"), AccountClass::Expense),
            Explicit("K-16d".into())
        );
        assert_eq!(
            resolve_k1_mapping(None, AccountClass::Revenue),
            AutoGrossReceipts
        );
        assert_eq!(resolve_k1_mapping(None, AccountClass::Expense), Unmapped);
    }

    fn k1_fixture(conn: &Connection) -> i64 {
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('K1T', 'checking')",
            [],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn k1_cat(conn: &Connection, name: &str, ctype: &str, form_line: Option<&str>) -> i64 {
        let class = crate::db::class_for_category_type(ctype);
        conn.execute(
            "INSERT INTO categories (name, category_type, form_line, class) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![name, ctype, form_line, class.as_str()],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn k1_txn(conn: &Connection, acct: i64, date: &str, amount: f64, cat: i64) {
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, ?2, 'x', ?3, ?4)",
            rusqlite::params![acct, date, amount, cat],
        )
        .unwrap();
    }

    #[test]
    fn test_k1_custom_chart_income_falls_back_and_unmapped_surfaces() {
        let (_dir, conn) = test_db();
        let acct = k1_fixture(&conn);
        let inc = k1_cat(&conn, "Widget Sales", "income", None);
        let exp = k1_cat(&conn, "Mystery Spend", "expense", None);
        let odd = k1_cat(&conn, "Odd Mapping", "expense", Some("Schedule Z"));
        let skip = k1_cat(&conn, "Personal", "expense", Some("excluded"));
        k1_txn(&conn, acct, "2025-02-01", 5000.0, inc);
        k1_txn(&conn, acct, "2025-02-02", -400.0, exp);
        k1_txn(&conn, acct, "2025-02-03", -75.0, odd);
        k1_txn(&conn, acct, "2025-02-04", -999.0, skip);

        let r = get_k1_prep(&conn, Some(2025)).unwrap();
        assert_eq!(r.gross_receipts, 5000.0);
        assert_eq!(r.auto_mapped, vec!["Widget Sales".to_string()]);
        let unmapped_names: Vec<&str> = r
            .unmapped
            .iter()
            .map(|u| u.category_name.as_str())
            .collect();
        assert!(unmapped_names.contains(&"Mystery Spend"));
        assert!(unmapped_names.contains(&"Odd Mapping"));
        assert!(!unmapped_names.contains(&"Personal"));
        // unmapped and excluded activity stays out of the math
        assert_eq!(r.total_deductions, 0.0);
        assert_eq!(r.ordinary_business_income, 5000.0);
    }

    #[test]
    fn test_k1_cogs_and_gross_profit() {
        let (_dir, conn) = test_db();
        let acct = k1_fixture(&conn);
        let inc = k1_cat(&conn, "Sales", "income", Some("1120S-1a"));
        let cogs = k1_cat(&conn, "Materials", "expense", Some("1120S-2"));
        let rent = k1_cat(&conn, "Shop Rent", "expense", Some("1120S-11"));
        k1_txn(&conn, acct, "2025-03-01", 10000.0, inc);
        k1_txn(&conn, acct, "2025-03-02", -2500.0, cogs);
        k1_txn(&conn, acct, "2025-03-03", -1000.0, rent);

        let r = get_k1_prep(&conn, Some(2025)).unwrap();
        assert_eq!(r.gross_receipts, 10000.0);
        assert_eq!(r.cogs, 2500.0);
        assert_eq!(r.gross_profit, 7500.0);
        assert_eq!(r.total_deductions, 1000.0);
        assert_eq!(r.ordinary_business_income, 6500.0);
        // COGS is an income-summary line, not a deduction line
        assert!(r.deduction_lines.iter().all(|d| d.form_line != "1120S-2"));
    }

    #[test]
    fn test_k1_headline_deductions_use_deductible_meals() {
        let (_dir, conn) = test_db();
        let acct = k1_fixture(&conn);
        let inc = k1_cat(&conn, "Sales", "income", Some("1120S-1a"));
        let meals = k1_cat(&conn, "Team Meals", "expense", Some("1120S-19"));
        let sw = k1_cat(&conn, "Tools", "expense", Some("1120S-19"));
        k1_txn(&conn, acct, "2025-04-01", 1000.0, inc);
        k1_txn(&conn, acct, "2025-04-02", -100.0, meals);
        k1_txn(&conn, acct, "2025-04-03", -40.0, sw);

        let r = get_k1_prep(&conn, Some(2025)).unwrap();
        // headline = 50 (meals at 50%) + 40 = other_deductions_total
        assert_eq!(r.total_deductions, 90.0);
        assert_eq!(r.other_deductions_total, 90.0);
        assert_eq!(r.ordinary_business_income, 910.0);
    }

    #[test]
    fn test_cashflow_full_year_running_balance() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report = get_cashflow(&conn, Some(2025), None).unwrap();
        // Jan: +1000 -50 = +950, Feb: -10 → running = 940
        assert_eq!(report.months.len(), 2);
        assert_eq!(report.months[0].running_balance, 950.0);
        assert_eq!(report.months[1].running_balance, 940.0);
    }

    #[test]
    fn test_cashflow_single_month_includes_prior_balance() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        // Feb only — running balance should include Jan's cumulative (950.0)
        let report = get_cashflow(&conn, Some(2025), Some(2)).unwrap();
        assert_eq!(report.months.len(), 1);
        assert_eq!(report.months[0].net, -10.0);
        // Running balance = prior 950.0 + Feb net -10.0 = 940.0
        assert_eq!(report.months[0].running_balance, 940.0);
    }

    #[test]
    fn test_cashflow_january_has_no_prior_balance() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report = get_cashflow(&conn, Some(2025), Some(1)).unwrap();
        assert_eq!(report.months.len(), 1);
        // Jan starts at 0 — no prior months
        assert_eq!(report.months[0].running_balance, 950.0);
    }

    #[test]
    fn test_cashflow_cross_year_boundary() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn); // 2025 transactions
                                  // Add a 2024 transaction that should NOT affect 2025 prior balance
        let acct: i64 = conn
            .query_row("SELECT id FROM accounts WHERE name = 'Test'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let cat: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Client Services'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2024-12-15', 'Prior year payment', 5000.0, ?2)",
            rusqlite::params![acct, cat],
        )
        .unwrap();
        // Feb 2025 prior balance should only include Jan 2025, not Dec 2024
        let report = get_cashflow(&conn, Some(2025), Some(2)).unwrap();
        assert_eq!(report.months.len(), 1);
        assert_eq!(report.months[0].running_balance, 940.0);
    }

    #[test]
    fn test_cashflow_unfiltered_starts_at_zero() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        // No year or month filter — running balance starts at 0
        let report = get_cashflow(&conn, None, None).unwrap();
        assert!(report.months.len() >= 2);
        assert_eq!(report.months[0].running_balance, 950.0); // first month net only
    }

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
            .query_row(
                "SELECT id FROM accounts WHERE name = 'Harbor Card'",
                [],
                |r| r.get(0),
            )
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
        assert_eq!(
            card_row.natural_balance, 1200.0,
            "money owed reads positive"
        );
        // The cash position is still the cash position.
        assert_eq!(report.total, -1200.0);
    }

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
            (
                "2025-01-10",
                "CEDAR SYSTEMS INVOICE",
                8_000.0,
                cat("Client Services"),
            ),
            (
                "2025-02-05",
                "SOFTWARE RENEWAL",
                -300.0,
                cat("Software & Subscriptions"),
            ),
            (
                "2025-03-01",
                "OWNER DRAW",
                -2_000.0,
                cat("Owner Draw / Distribution"),
            ),
            (
                "2025-03-02",
                "OWNER FUNDS IN",
                1_000.0,
                cat("Owner Contribution"),
            ),
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

        assert_eq!(
            report.total_income, 8_000.0,
            "the contribution is not revenue"
        );
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
        assert_eq!(class_of("Owner Draw / Distribution"), AccountClass::Equity);
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
                get_pnl(&conn, Some(2025), None, None, None)
                    .unwrap()
                    .total_expenses,
                expected,
                "P&L expenses absorbed a {} category",
                class.as_str()
            );
            assert_eq!(
                get_expense_breakdown(&conn, Some(2025), None)
                    .unwrap()
                    .total,
                expected,
                "the expense breakdown absorbed a {} category",
                class.as_str()
            );
            let deductions = get_k1_prep(&conn, Some(2025)).unwrap().total_deductions;
            assert_eq!(
                deductions,
                0.0,
                "the K-1 deducted a {} category with no form line",
                class.as_str()
            );
        }
    }
}
