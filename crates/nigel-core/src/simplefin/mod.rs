//! Sync from a SimpleFIN bridge: linked accounts, the sync itself, and the
//! balances the bank reports.
//!
//! A sync is an import whose rows come from a feed rather than a file. Each
//! mapped account becomes one [`ParsedImport`] handed to
//! [`import_parsed`], so it gets an `imports` row (import history and undo),
//! a rejects record and the external-id dedup a file would. Categorization
//! runs once over the whole sync, in the caller's transaction.

pub mod client;

use chrono::{Duration, NaiveDate, TimeZone};
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::clock::local_day;
use crate::error::{NigelError, Result};
use crate::importer::{import_parsed, ParseOutcome, ParsedImport, RejectedRow};
use crate::models::ParsedRow;

pub use client::{claim, fetch_accounts, AccessUrl, Account, AccountSet, AccountsQuery};

/// The format name a SimpleFIN import reports and import history shows.
pub const FORMAT: &str = "simplefin";

/// The `source` recorded beside a balance the bridge reported.
pub const BALANCE_SOURCE: &str = "simplefin";

/// How far back a repeat sync re-reads, so a transaction the bank posts a
/// few days late is still picked up. The external id absorbs the overlap.
pub const RESYNC_OVERLAP_DAYS: i64 = 7;

/// A SimpleFIN account Nigel has seen, and the Nigel account it feeds.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedAccount {
    /// Stable across syncs: the organization, then the bridge's account id.
    pub key: String,
    pub org_name: String,
    pub name: String,
    pub currency: String,
    /// The Nigel account it is mapped to, or `None` when sync skips it.
    pub account: Option<String>,
    /// The last day a sync of this account committed.
    pub last_synced: Option<String>,
}

/// Record every account the bridge returned, keeping existing mappings.
pub fn remember_accounts(conn: &Connection, set: &AccountSet) -> Result<()> {
    let mut stmt = conn.prepare_cached(
        "INSERT INTO simplefin_accounts (sfin_key, org_name, name, currency) VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT(sfin_key) DO UPDATE SET org_name = excluded.org_name, name = excluded.name, \
         currency = excluded.currency",
    )?;
    for account in &set.accounts {
        stmt.execute(rusqlite::params![
            account.key(),
            account.org.label(),
            account.name,
            account.currency,
        ])?;
    }
    Ok(())
}

pub fn list_accounts(conn: &Connection) -> Result<Vec<LinkedAccount>> {
    let mut stmt = conn.prepare(
        "SELECT s.sfin_key, s.org_name, s.name, s.currency, a.name, s.last_synced \
         FROM simplefin_accounts s LEFT JOIN accounts a ON a.id = s.account_id \
         ORDER BY s.org_name, s.name, s.sfin_key",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(LinkedAccount {
                key: r.get(0)?,
                org_name: r.get(1)?,
                name: r.get(2)?,
                currency: r.get(3)?,
                account: r.get(4)?,
                last_synced: r.get(5)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Point a SimpleFIN account at a Nigel account. One feed per Nigel
/// account: a second would double every transaction the two share.
pub fn map_account(conn: &Connection, key: &str, account_name: &str) -> Result<()> {
    let account_id: i64 = conn
        .query_row(
            "SELECT id FROM accounts WHERE name = ?1",
            [account_name],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| NigelError::UnknownAccount(account_name.to_string()))?;
    let taken: Option<String> = conn
        .query_row(
            "SELECT sfin_key FROM simplefin_accounts WHERE account_id = ?1 AND sfin_key <> ?2",
            rusqlite::params![account_id, key],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(other) = taken {
        return Err(NigelError::Invalid(format!(
            "{account_name} is already fed by SimpleFIN account {other}; unmap that one first"
        )));
    }
    let changed = conn.execute(
        "UPDATE simplefin_accounts SET account_id = ?1, \
             last_synced = CASE WHEN account_id IS ?1 THEN last_synced END \
         WHERE sfin_key = ?2",
        rusqlite::params![account_id, key],
    )?;
    if changed == 0 {
        return Err(unknown_key(key));
    }
    Ok(())
}

pub fn unmap_account(conn: &Connection, key: &str) -> Result<()> {
    let changed = conn.execute(
        "UPDATE simplefin_accounts SET account_id = NULL, last_synced = NULL WHERE sfin_key = ?1",
        [key],
    )?;
    if changed == 0 {
        return Err(unknown_key(key));
    }
    Ok(())
}

fn unknown_key(key: &str) -> NigelError {
    NigelError::NotFound(format!(
        "no SimpleFIN account {key}; `nigel sync accounts` lists them"
    ))
}

/// A mapped account and the first day its sync should read.
struct Plan {
    key: String,
    account_id: i64,
    account_name: String,
    start: Option<NaiveDate>,
}

fn parse_day(day: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .map_err(|_| NigelError::Invalid(format!("{day:?} is not a YYYY-MM-DD date")))
}

/// Where each mapped account's sync starts.
///
/// `--since` wins. Otherwise a repeat sync re-reads from its last day minus
/// [`RESYNC_OVERLAP_DAYS`], and a first sync starts the day after the
/// account's latest transaction. Neither ever starts on or before the latest
/// transaction that came without an id — a CSV row — because a feed's row
/// cannot be matched against one and would import twice. An account with no
/// history at all takes whatever the bridge offers.
fn plan(conn: &Connection, since: Option<NaiveDate>) -> Result<Vec<Plan>> {
    let mut stmt = conn.prepare(
        "SELECT s.sfin_key, a.id, a.name, s.last_synced, \
                (SELECT MAX(date) FROM transactions t WHERE t.account_id = a.id), \
                (SELECT MAX(date) FROM transactions t \
                  WHERE t.account_id = a.id AND t.external_id IS NULL) \
         FROM simplefin_accounts s JOIN accounts a ON a.id = s.account_id \
         ORDER BY s.sfin_key",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut plans = Vec::with_capacity(rows.len());
    for (key, account_id, account_name, last_synced, latest, latest_without_id) in rows {
        let start = if since.is_some() {
            since
        } else {
            let base = match (last_synced, latest) {
                (Some(last), _) => Some(parse_day(&last)? - Duration::days(RESYNC_OVERLAP_DAYS)),
                (None, Some(latest)) => Some(parse_day(&latest)? + Duration::days(1)),
                (None, None) => None,
            };
            let floor = latest_without_id
                .map(|day| parse_day(&day).map(|d| d + Duration::days(1)))
                .transpose()?;
            base.max(floor)
        };
        plans.push(Plan {
            key,
            account_id,
            account_name,
            start,
        });
    }
    Ok(plans)
}

/// Local midnight of `day`, in Unix seconds: what `start-date` takes.
fn local_midnight(day: NaiveDate) -> i64 {
    let midnight = day.and_hms_opt(0, 0, 0).expect("midnight exists");
    chrono::Local
        .from_local_datetime(&midnight)
        .earliest()
        .map_or_else(|| midnight.and_utc().timestamp(), |t| t.timestamp())
}

/// What a sync should ask the bridge for: the earliest start any mapped
/// account needs, or `None` when there is nothing mapped.
pub fn query_for(conn: &Connection, since: Option<NaiveDate>) -> Result<Option<AccountsQuery>> {
    let plans = plan(conn, since)?;
    if plans.is_empty() {
        return Ok(None);
    }
    // One account with no start means no start for the request: the others
    // trim their own rows afterwards.
    let start = plans
        .iter()
        .map(|p| p.start)
        .collect::<Option<Vec<_>>>()
        .and_then(|starts| starts.into_iter().min());
    Ok(Some(AccountsQuery {
        start: start.map(local_midnight),
        end: None,
        balances_only: false,
    }))
}

/// One account's part of a sync.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSync {
    pub key: String,
    pub label: String,
    pub account: String,
    pub start: Option<String>,
    pub imported: usize,
    pub skipped: usize,
    pub malformed: usize,
    /// The `imports` row written, or `None` for a dry run or nothing new.
    pub import_id: Option<i64>,
    pub balance: Option<ReportedBalance>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportedBalance {
    pub balance: f64,
    pub as_of: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub accounts: Vec<AccountSync>,
    /// Bridge accounts with no Nigel account, as "org — name".
    pub unmapped: Vec<String>,
    /// Messages the bridge sent, sanitized for display.
    pub warnings: Vec<String>,
}

impl SyncReport {
    pub fn imported(&self) -> usize {
        self.accounts.iter().map(|a| a.imported).sum()
    }
}

fn parse_money(raw: &str) -> Option<f64> {
    let value: f64 = raw.trim().parse().ok()?;
    value.is_finite().then(|| (value * 100.0).round() / 100.0)
}

/// Turn one account's feed into rows and rejects. Pending transactions are
/// left for a later sync, when they have posted and settled their amount.
fn outcome_for(account: &Account, start: Option<NaiveDate>) -> ParseOutcome {
    let mut out = ParseOutcome::default();
    let key = account.key();
    for (index, txn) in account.transactions.iter().enumerate() {
        if txn.pending || txn.posted == 0 {
            continue;
        }
        let reject = |reason: String| RejectedRow {
            row_number: index as u64 + 1,
            content: format!(
                "id={},posted={},amount={},description={}",
                txn.id, txn.posted, txn.amount, txn.description
            ),
            reason,
        };
        let Some(date) = local_day(txn.posted) else {
            out.rejects
                .push(reject(format!("posted time {} is not a date", txn.posted)));
            continue;
        };
        if let Some(start) = start {
            if parse_day(&date).is_ok_and(|d| d < start) {
                continue;
            }
        }
        let Some(amount) = parse_money(&txn.amount) else {
            out.rejects
                .push(reject(format!("amount {:?} is not a number", txn.amount)));
            continue;
        };
        if txn.id.trim().is_empty() {
            out.rejects
                .push(reject("the transaction has no id".to_string()));
            continue;
        }
        out.rows.push(ParsedRow {
            date,
            description: txn.description.trim().to_string(),
            amount,
            external_id: Some(format!("{FORMAT}:{key}:{}", txn.id)),
        });
    }
    out
}

fn checksum_for(key: &str, outcome: &ParseOutcome) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{FORMAT}\n{key}\n"));
    for row in &outcome.rows {
        hasher.update(format!(
            "{}\t{}\t{}\t{}\n",
            row.external_id.as_deref().unwrap_or(""),
            row.date,
            row.amount,
            row.description
        ));
    }
    hex::encode(hasher.finalize())
}

/// Apply a bridge response to the books. Runs in the caller's transaction;
/// the caller snapshots before and categorizes after. A dry run writes
/// nothing, not even the account list.
pub fn apply(
    conn: &Connection,
    set: &AccountSet,
    since: Option<NaiveDate>,
    today: &str,
    dry_run: bool,
) -> Result<SyncReport> {
    if !dry_run {
        remember_accounts(conn, set)?;
    }
    let plans = plan(conn, since)?;
    let mut report = SyncReport {
        warnings: set
            .errors
            .iter()
            .map(|m| client::sanitize_message(m))
            .filter(|m| !m.is_empty())
            .collect(),
        ..SyncReport::default()
    };

    for account in &set.accounts {
        let key = account.key();
        let label = format!("{} — {}", account.org.label(), account.name);
        let Some(plan) = plans.iter().find(|p| p.key == key) else {
            report.unmapped.push(label);
            continue;
        };

        let outcome = outcome_for(account, plan.start);
        let balance = parse_money(&account.balance).and_then(|balance| {
            local_day(account.balance_date).map(|as_of| ReportedBalance { balance, as_of })
        });
        let malformed = outcome.malformed();
        let parsed = ParsedImport {
            account_id: plan.account_id,
            source_name: format!("SimpleFIN: {label}"),
            checksum: checksum_for(&key, &outcome),
            format: FORMAT.to_string(),
            outcome,
        };

        // Counted first, read-only: an account with nothing new writes no
        // `imports` row, so a quiet day does not fill import history with
        // empty entries or become what `undo` takes back. Rejects ride on an
        // import that adds something; on a day with nothing new they are
        // reported here only, and the overlap window brings them back.
        let preview = if parsed.outcome.rows.is_empty() {
            None
        } else {
            Some(import_parsed(conn, &parsed, true)?)
        };
        let (imported, skipped, import_id) = match preview {
            Some(p) if !dry_run && p.imported > 0 => {
                let result = import_parsed(conn, &parsed, false)?;
                (result.imported, result.skipped, result.import_id)
            }
            Some(p) => (p.imported, p.skipped, None),
            None => (0, 0, None),
        };

        if !dry_run {
            if let Some(b) = &balance {
                conn.execute(
                    "INSERT INTO account_balances (account_id, balance, as_of, source) \
                     VALUES (?1, ?2, ?3, ?4) \
                     ON CONFLICT(account_id, source, as_of) DO UPDATE SET balance = excluded.balance",
                    rusqlite::params![plan.account_id, b.balance, b.as_of, BALANCE_SOURCE],
                )?;
            }
            // Synced only as far as the bank has answered: the bridge's balance
            // date stops moving while a connection is broken, so a stale feed
            // leaves the next window reaching back over the days it never
            // fetched. Capped at today, and never moved backwards.
            let reported = (account.balance_date > 0)
                .then(|| local_day(account.balance_date))
                .flatten();
            if let Some(through) = reported.map(|day| day.min(today.to_string())) {
                conn.execute(
                    "UPDATE simplefin_accounts SET last_synced = ?1 \
                     WHERE sfin_key = ?2 AND (last_synced IS NULL OR last_synced < ?1)",
                    rusqlite::params![through, key],
                )?;
            }
        }

        report.accounts.push(AccountSync {
            key,
            label,
            account: plan.account_name.clone(),
            start: plan.start.map(|d| d.format("%Y-%m-%d").to_string()),
            imported,
            skipped,
            malformed,
            import_id,
            balance,
        });
    }
    Ok(report)
}

/// The latest balance a source reported for each account, by account id.
pub fn latest_reported_balances(
    conn: &Connection,
) -> Result<std::collections::HashMap<i64, ReportedBalance>> {
    let mut stmt = conn.prepare(
        "SELECT account_id, balance, as_of FROM account_balances b \
         WHERE as_of = (SELECT MAX(as_of) FROM account_balances WHERE account_id = b.account_id) \
         ORDER BY account_id, recorded_at DESC",
    )?;
    let mut out = std::collections::HashMap::new();
    for row in stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            ReportedBalance {
                balance: r.get(1)?,
                as_of: r.get(2)?,
            },
        ))
    })? {
        let (id, balance) = row?;
        out.entry(id).or_insert(balance);
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
