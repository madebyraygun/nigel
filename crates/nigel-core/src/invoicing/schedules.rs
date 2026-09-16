//! Recurring invoice schedules: a stored shape, a cadence, and the history of
//! what it has already produced.
//!
//! Nothing here reads the clock or the settings. A run's reference day is a
//! parameter, and the collaborators an autosend needs are injected by the
//! caller — the rule the whole of `src/invoicing/` keeps.

use chrono::{Datelike, NaiveDate};
use rusqlite::Connection;
use serde::Serialize;

use crate::error::{NigelError, Result};
use crate::invoicing::clients::ensure_client_active;
use crate::invoicing::gateway::{AssetPublisher, Mailer, PaymentGateway};
use crate::invoicing::invoices::{validate_currency, validate_date, validate_items, NewLineItem};
use crate::invoicing::render_html::Branding;

/// How often a schedule bills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cadence {
    Monthly,
    Quarterly,
    Yearly,
}

impl Cadence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Monthly => "monthly",
            Self::Quarterly => "quarterly",
            Self::Yearly => "yearly",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "monthly" => Ok(Self::Monthly),
            "quarterly" => Ok(Self::Quarterly),
            "yearly" => Ok(Self::Yearly),
            other => Err(NigelError::Invalid(format!(
                "Unknown cadence: {other} (expected monthly, quarterly or yearly)"
            ))),
        }
    }

    /// How many months one cycle covers.
    pub fn months(self) -> u32 {
        match self {
            Self::Monthly => 1,
            Self::Quarterly => 3,
            Self::Yearly => 12,
        }
    }
}

/// Which schedules a listing wants. `Active` is what a run walks: not paused
/// and not ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleScope {
    Active,
    All,
}

/// A schedule being created.
#[derive(Debug, Clone)]
pub struct NewSchedule {
    pub client_id: i64,
    pub cadence: Cadence,
    /// The day of the month the cycle is anchored on, `1..=31`. Remembered
    /// rather than clamped, so a short month does not permanently move it.
    pub anchor_day: u32,
    /// The first period's issue date.
    pub start_period: String,
    pub net_days: Option<i64>,
    pub currency: String,
    pub notes: Option<String>,
    pub terms: Option<String>,
    pub autosend: bool,
    pub items: Vec<NewLineItem>,
}

/// A stored schedule.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    pub id: i64,
    pub client_id: i64,
    pub cadence: String,
    pub anchor_day: u32,
    pub next_period: String,
    pub net_days: Option<i64>,
    pub currency: String,
    pub notes: Option<String>,
    pub terms: Option<String>,
    pub autosend: bool,
    pub paused: bool,
    /// The date it was paused on, and `None` when it is not paused.
    ///
    /// Also `None` for a schedule already paused before migration v13, where
    /// the date is not recoverable: a pause of unknown extent forgives nothing.
    pub paused_at: Option<String>,
    pub ended_at: Option<String>,
}

/// A partial edit. `Option<Option<T>>` is `InvoiceUpdate`'s shape and means the
/// same thing: absent leaves the field alone, `Some(None)` clears it.
#[derive(Debug, Clone, Default)]
pub struct ScheduleUpdate {
    pub anchor_day: Option<u32>,
    pub net_days: Option<Option<i64>>,
    pub currency: Option<String>,
    pub notes: Option<Option<String>>,
    pub terms: Option<Option<String>>,
    pub autosend: Option<bool>,
    pub items: Option<Vec<NewLineItem>>,
}

impl ScheduleUpdate {
    pub fn is_empty(&self) -> bool {
        self.anchor_day.is_none()
            && self.net_days.is_none()
            && self.currency.is_none()
            && self.notes.is_none()
            && self.terms.is_none()
            && self.autosend.is_none()
            && self.items.is_none()
    }
}

/// One period a schedule has already settled: an invoice it produced, or a
/// cycle a pause forgave. `invoice_id` and `skipped_reason` are exactly one
/// each way round, which the table's CHECK enforces.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleRun {
    pub period: String,
    pub invoice_id: Option<i64>,
    pub number: Option<i64>,
    pub generated_at: String,
    pub skipped_reason: Option<String>,
}

const SCHEDULE_COLS: &str = "id, client_id, cadence, anchor_day, next_period, net_days,
    currency, notes, terms, autosend, paused, paused_at, ended_at";

fn row_to_schedule(r: &rusqlite::Row) -> rusqlite::Result<Schedule> {
    Ok(Schedule {
        id: r.get(0)?,
        client_id: r.get(1)?,
        cadence: r.get(2)?,
        anchor_day: r.get::<_, i64>(3)? as u32,
        next_period: r.get(4)?,
        net_days: r.get(5)?,
        currency: r.get(6)?,
        notes: r.get(7)?,
        terms: r.get(8)?,
        autosend: r.get(9)?,
        paused: r.get(10)?,
        paused_at: r.get(11)?,
        ended_at: r.get(12)?,
    })
}

/// `1..=31`, the range the table's own CHECK enforces — refused here so the
/// sentence names the field rather than the constraint.
fn validate_anchor_day(day: u32) -> Result<u32> {
    if (1..=31).contains(&day) {
        return Ok(day);
    }
    Err(NigelError::Invalid(format!(
        "Anchor day must be between 1 and 31, got {day}."
    )))
}

/// Create a schedule and its line items in one transaction.
///
/// Validated with the invoice writers' own rules, so a schedule cannot be
/// created that would refuse on its first run: at least one line item, finite
/// figures, a total above zero, a real currency, a real start date, and a
/// client that is not archived.
pub fn add_schedule(conn: &Connection, new: &NewSchedule) -> Result<i64> {
    ensure_client_active(conn, new.client_id)?;
    validate_items(&new.items)?;
    let start_period = validate_date(&new.start_period, "start")?;
    let currency = validate_currency(&new.currency)?;
    let anchor_day = validate_anchor_day(new.anchor_day)?;

    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO invoice_schedules
            (client_id, cadence, anchor_day, next_period, net_days, currency, notes, terms, autosend)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            new.client_id,
            new.cadence.as_str(),
            anchor_day,
            start_period,
            new.net_days,
            currency,
            new.notes,
            new.terms,
            new.autosend,
        ],
    )?;
    let id = tx.last_insert_rowid();
    write_items(&tx, id, &new.items)?;
    tx.commit()?;
    Ok(id)
}

/// Rewrite a schedule's line items at dense positions `0..n-1`.
fn write_items(conn: &Connection, schedule_id: i64, items: &[NewLineItem]) -> Result<()> {
    conn.execute(
        "DELETE FROM invoice_schedule_items WHERE schedule_id = ?1",
        [schedule_id],
    )?;
    for (idx, item) in items.iter().enumerate() {
        conn.execute(
            "INSERT INTO invoice_schedule_items
                (schedule_id, description, quantity, unit_amount, position)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                schedule_id,
                item.description,
                item.quantity,
                item.unit_amount,
                idx as i64
            ],
        )?;
    }
    Ok(())
}

pub fn get_schedule(conn: &Connection, id: i64) -> Result<Schedule> {
    conn.query_row(
        &format!("SELECT {SCHEDULE_COLS} FROM invoice_schedules WHERE id = ?1"),
        [id],
        row_to_schedule,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            NigelError::NotFound(format!("Invoice schedule not found: id {id}"))
        }
        other => NigelError::Db(other),
    })
}

pub fn list_schedules(conn: &Connection, scope: ScheduleScope) -> Result<Vec<Schedule>> {
    let filter = match scope {
        ScheduleScope::Active => "WHERE paused = 0 AND ended_at IS NULL",
        ScheduleScope::All => "",
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT {SCHEDULE_COLS} FROM invoice_schedules {filter} ORDER BY id"
    ))?;
    let rows = stmt
        .query_map([], row_to_schedule)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The schedule's own line items, re-read per run — editing a schedule changes
/// future invoices and never past ones.
pub fn schedule_items(conn: &Connection, schedule_id: i64) -> Result<Vec<NewLineItem>> {
    let mut stmt = conn.prepare(
        "SELECT description, quantity, unit_amount
           FROM invoice_schedule_items WHERE schedule_id = ?1 ORDER BY position",
    )?;
    let rows = stmt
        .query_map([schedule_id], |r| {
            Ok(NewLineItem {
                description: r.get(0)?,
                quantity: r.get(1)?,
                unit_amount: r.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Apply a partial edit. `next_period` is never touched: editing a schedule
/// changes what it bills, not where it is in its cycle.
pub fn update_schedule(conn: &Connection, id: i64, update: &ScheduleUpdate) -> Result<()> {
    let _ = get_schedule(conn, id)?;
    if update.is_empty() {
        return Err(NigelError::Invalid(
            "Nothing to change. Pass at least one field to edit.".to_string(),
        ));
    }
    if let Some(items) = &update.items {
        validate_items(items)?;
    }
    let anchor_day = update.anchor_day.map(validate_anchor_day).transpose()?;
    let currency = update
        .currency
        .as_deref()
        .map(validate_currency)
        .transpose()?;

    let tx = conn.unchecked_transaction()?;
    if let Some(day) = anchor_day {
        tx.execute(
            "UPDATE invoice_schedules SET anchor_day = ?2 WHERE id = ?1",
            rusqlite::params![id, day],
        )?;
    }
    if let Some(currency) = currency {
        tx.execute(
            "UPDATE invoice_schedules SET currency = ?2 WHERE id = ?1",
            rusqlite::params![id, currency],
        )?;
    }
    if let Some(net_days) = update.net_days {
        tx.execute(
            "UPDATE invoice_schedules SET net_days = ?2 WHERE id = ?1",
            rusqlite::params![id, net_days],
        )?;
    }
    if let Some(notes) = &update.notes {
        tx.execute(
            "UPDATE invoice_schedules SET notes = ?2 WHERE id = ?1",
            rusqlite::params![id, notes],
        )?;
    }
    if let Some(terms) = &update.terms {
        tx.execute(
            "UPDATE invoice_schedules SET terms = ?2 WHERE id = ?1",
            rusqlite::params![id, terms],
        )?;
    }
    if let Some(autosend) = update.autosend {
        tx.execute(
            "UPDATE invoice_schedules SET autosend = ?2 WHERE id = ?1",
            rusqlite::params![id, autosend],
        )?;
    }
    if let Some(items) = &update.items {
        write_items(&tx, id, items)?;
    }
    tx.commit()?;
    Ok(())
}

/// Stop generating without ending, recording the day it stopped.
///
/// `on` is what separates a pause from arrears. A schedule can be behind
/// *before* it is paused — months worked and never invoiced — and those stay
/// owed, while the cycles the pause itself covers do not: skipping them was the
/// point of pausing.
pub fn pause_schedule(conn: &Connection, id: i64, on: &str) -> Result<()> {
    let _ = get_schedule(conn, id)?;
    let on = validate_date(on, "pause")?;
    conn.execute(
        "UPDATE invoice_schedules SET paused = 1, paused_at = ?2 WHERE id = ?1",
        rusqlite::params![id, on],
    )?;
    Ok(())
}

/// Start a paused schedule generating again, recording the cycles the pause
/// covered so nothing bills them later. Answers the periods it forgave.
///
/// A pause forgives the cycles it covers — that is the rule `end_schedule`
/// already honours. Clearing the flag alone would leave `next_period` standing
/// behind the pause and the next run would bill straight through it, so the
/// forgiven periods are written as **skip rows**: the same
/// `invoice_schedule_runs` row a generated period writes, with no invoice
/// behind it. The walk already generates nothing for a period it finds a row
/// for, so the two paths agree by construction rather than by keeping a second
/// rule in step with the first.
///
/// Arrears *behind* the pause are not forgiven and get no rows — a schedule
/// late since January and paused in March owes January and February and not
/// March onward. One cursor cannot express a gap in the middle, which is why
/// this is rows rather than an advanced `next_period`.
///
/// A schedule paused before migration v13 carries no `paused_at` and so
/// forgives nothing: the extent of that pause is not recoverable, and billing a
/// cycle the operator meant to skip is the recoverable mistake of the two.
pub fn resume_schedule(conn: &Connection, id: i64, on: &str) -> Result<Vec<String>> {
    let schedule = get_schedule(conn, id)?;
    let on = validate_date(on, "resume")?;
    let mut skipped = Vec::new();

    let tx = conn.unchecked_transaction()?;
    if let Some(paused_at) = schedule.paused_at.as_deref() {
        let cadence = Cadence::parse(&schedule.cadence)?;
        let reason = format!("paused {paused_at}");
        let mut cursor = schedule.next_period.clone();
        while cursor.as_str() <= on.as_str() {
            if cursor.as_str() >= paused_at {
                let written = tx.execute(
                    "INSERT OR IGNORE INTO invoice_schedule_runs
                         (schedule_id, period, invoice_id, generated_at, skipped_reason)
                     VALUES (?1, ?2, NULL, ?3, ?4)",
                    rusqlite::params![id, cursor, on, reason],
                )?;
                if written > 0 {
                    skipped.push(cursor.clone());
                }
            }
            cursor = advance_period(cadence, schedule.anchor_day, &cursor)?;
        }
    }
    tx.execute(
        "UPDATE invoice_schedules SET paused = 0, paused_at = NULL WHERE id = ?1",
        [id],
    )?;
    tx.commit()?;
    Ok(skipped)
}

/// What to do about periods a schedule owed and never billed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndDisposition {
    /// End only if the schedule owes nothing, and refuse otherwise.
    RefuseIfOwed,
    /// End without billing them: the work is written off.
    Forgive,
    /// Generate them as drafts, then end.
    Bill,
}

/// What ending a schedule did.
///
/// An enum rather than a struct of optional fields because the outcomes are
/// mutually exclusive, and one of them is a failure returned as `Ok` — the
/// report of what *was* committed is worth more than an `Err` that discards it.
/// A caller that has to read an empty vector to learn which happened will
/// eventually forget to.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum EndOutcome {
    /// The schedule is ended as of `on`.
    Ended { on: String, settled: Settlement },
    /// Nothing was ended. The walk stopped partway, so the schedule is still
    /// active and can be retried — the invoices already generated are kept, and
    /// their run rows make the retry skip them.
    BillingStopped {
        billed: ScheduleRunReport,
        /// How many periods were owed when the walk started, so a caller can
        /// say how much of the backlog is still outstanding.
        owed: usize,
    },
}

/// How a schedule settled the periods it owed on the way out.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "disposition", rename_all = "camelCase")]
pub enum Settlement {
    /// It owed nothing, so the disposition never came into it.
    NothingOwed,
    /// The periods written off.
    Forgiven { periods: Vec<String> },
    /// The drafts generated on the way out.
    Billed { report: ScheduleRunReport },
}

/// The periods this schedule still owes on or before `on`.
///
/// The walk a run makes over one schedule — from `next_period` while the cursor
/// is not past `on`, skipping anything a run row already claims — so no period
/// after `on` is reachable.
///
/// A **paused** schedule owes only what it was already behind on when the pause
/// began. Cycles the pause itself covers are not owed — skipping them is what
/// pausing is for — so the walk stops at `paused_at`. A schedule paused before
/// migration v13 has no recorded date and owes everything, which is the
/// conservative reading of a pause whose extent is unknown.
///
/// An **ended** schedule owes nothing: ending already settled it.
///
/// The list is a preview, not a lock. A concurrent run can bill a period
/// between this read and any use of the result; [`generate_period`]'s own check
/// inside its transaction is the authority.
///
/// `on` is trusted. A date in the future reports cycles that are not yet due,
/// and every caller in this workspace passes today.
pub fn unbilled_periods(conn: &Connection, id: i64, on: &str) -> Result<Vec<String>> {
    let schedule = get_schedule(conn, id)?;
    let on = validate_date(on, "end")?;
    if schedule.ended_at.is_some() {
        return Ok(Vec::new());
    }
    let cadence = Cadence::parse(&schedule.cadence)?;

    let mut periods = Vec::new();
    let mut cursor = schedule.next_period.clone();
    while cursor <= on {
        // A pause forgives the cycles it covers. Once resumed those periods
        // carry skip rows and the `already` check below is what passes them;
        // this covers the schedule that is still paused, whose forgiven span
        // has no end yet and so is not written down.
        if schedule
            .paused_at
            .as_ref()
            .is_some_and(|since| cursor >= *since)
        {
            break;
        }
        let already: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM invoice_schedule_runs
               WHERE schedule_id = ?1 AND period = ?2)",
            rusqlite::params![id, cursor],
            |r| r.get(0),
        )?;
        if !already {
            periods.push(cursor.clone());
        }
        cursor = advance_period(cadence, schedule.anchor_day, &cursor)?;
    }
    Ok(periods)
}

/// Stop a schedule for good. A timestamp rather than a delete, the way
/// `voided_at` and `archived_at` are: the invoices it produced keep their
/// provenance, and the history stays readable.
///
/// Ending is refused while periods on or before the end date are still
/// unbilled, and `disposition` is how the caller says which it meant. Both
/// answers are defensible — you stopped billing them, or you worked the months
/// and never invoiced — so neither is taken silently on the operator's behalf.
///
/// `Bill` drafts and never sends, autosend or not. Ending a schedule is a
/// deliberate act with someone present, and drafting is the default generation
/// already takes. Each period commits on its own, so a walk that stops partway
/// leaves real invoices behind; the schedule is then left active rather than
/// ended, and the run rows make a retry skip what already landed.
///
/// Ending is terminal. A schedule that already carries an `ended_at` is
/// refused, because a second end would walk the same gap again and bill periods
/// dated after the day it stopped.
pub fn end_schedule(
    conn: &Connection,
    id: i64,
    on: &str,
    disposition: EndDisposition,
) -> Result<EndOutcome> {
    let schedule = get_schedule(conn, id)?;
    if let Some(ended) = &schedule.ended_at {
        return Err(NigelError::Conflict {
            code: "schedule_already_ended",
            message: format!(
                "Schedule {id} already ended on {ended}. Whatever it owed was \
                 settled then, and ending again would bill it a second time."
            ),
        });
    }
    let on = validate_date(on, "end")?;
    let owed = unbilled_periods(conn, id, &on)?;

    if owed.is_empty() {
        return finish(conn, id, on, Settlement::NothingOwed);
    }

    match disposition {
        EndDisposition::RefuseIfOwed => Err(NigelError::Conflict {
            code: "schedule_has_unbilled_periods",
            message: format!(
                "Schedule {id} has {} unbilled period{} on or before {on}: {}. \
                 Bill them or forgive them — ending cannot decide that for you.",
                owed.len(),
                if owed.len() == 1 { "" } else { "s" },
                owed.join(", "),
            ),
        }),
        EndDisposition::Forgive => finish(conn, id, on, Settlement::Forgiven { periods: owed }),
        EndDisposition::Bill => {
            let mut report = ScheduleRunReport::default();
            for period in &owed {
                let stopped = match generate_period(conn, &schedule, period, &on) {
                    Ok(Some(invoice_id)) => {
                        match describe(conn, &schedule, period, invoice_id) {
                            Ok(generated) => {
                                report.generated.push(generated);
                                None
                            }
                            // The invoice is already committed, so this cannot
                            // unwind with `?`: that would drop the record of
                            // every invoice this walk has created.
                            Err(e) => Some(format!(
                                "invoice {invoice_id} was created but could not be \
                                 read back: {e}"
                            )),
                        }
                    }
                    // A run row appeared between `unbilled_periods` and here: a
                    // concurrent `schedule run` billed this period. The invoice
                    // exists, so it is no longer owed.
                    Ok(None) => None,
                    Err(e) => Some(e.to_string()),
                };
                if let Some(message) = stopped {
                    report.failures.push(ScheduleFailure {
                        schedule_id: id,
                        period: period.clone(),
                        message,
                    });
                    // Ending here would strand the periods the walk never
                    // reached, with nothing left active to generate them.
                    return Ok(EndOutcome::BillingStopped {
                        billed: report,
                        owed: owed.len(),
                    });
                }
            }
            finish(conn, id, on, Settlement::Billed { report })
        }
    }
}

/// Write the end date and say what it settled.
fn finish(conn: &Connection, id: i64, on: String, settled: Settlement) -> Result<EndOutcome> {
    conn.execute(
        "UPDATE invoice_schedules SET ended_at = ?2 WHERE id = ?1",
        rusqlite::params![id, on],
    )?;
    Ok(EndOutcome::Ended { on, settled })
}

/// What a schedule has already produced, oldest period first.
pub fn schedule_runs(conn: &Connection, schedule_id: i64) -> Result<Vec<ScheduleRun>> {
    let mut stmt = conn.prepare(
        "SELECT r.period, r.invoice_id, i.number, r.generated_at, r.skipped_reason
           FROM invoice_schedule_runs r
           LEFT JOIN invoices i ON i.id = r.invoice_id
          WHERE r.schedule_id = ?1
          ORDER BY r.period",
    )?;
    let rows = stmt
        .query_map([schedule_id], |r| {
            Ok(ScheduleRun {
                period: r.get(0)?,
                invoice_id: r.get(1)?,
                number: r.get(2)?,
                generated_at: r.get(3)?,
                skipped_reason: r.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Clamp a day to the last valid day of the given year and month.
///
/// `month` is 1-12 and `year` inside chrono's range; anything else is a caller
/// bug and panics rather than answering. The rollover below is December's
/// alone — applied to any out-of-range month it would answer 31 for a month
/// that does not exist, which is a wrong figure on an invoice instead of a
/// stopped run.
pub fn clamp_day(year: i32, month: u32, day: u32) -> u32 {
    assert!(
        (1..=12).contains(&month),
        "clamp_day: month {month} is outside 1-12"
    );
    let first_of_next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .expect("valid year for date arithmetic");
    let last_day = first_of_next
        .pred_opt()
        .expect("predecessor of first-of-month is valid")
        .day();
    day.min(last_day)
}

/// The period after `period` for this cadence.
///
/// Anchored on `anchor_day` and clamped into short months, so a schedule
/// anchored on the 31st bills the 30th in April and the 31st again in May: the
/// **anchor** is what advances, never the clamped day it produced last time.
pub fn advance_period(cadence: Cadence, anchor_day: u32, period: &str) -> Result<String> {
    let anchor_day = validate_anchor_day(anchor_day)?;
    let current = crate::invoicing::invoices::parse_date(period, "period")?;
    let zero_based = (current.month() - 1) + cadence.months();
    let year = current.year() + (zero_based / 12) as i32;
    let month = zero_based % 12 + 1;
    let day = clamp_day(year, month, anchor_day);
    Ok(format!("{year:04}-{month:02}-{day:02}"))
}

/// The collaborators an autosend run needs.
///
/// Injected, never resolved here: `src/invoicing/` does not read settings, so
/// the branding and the three clients come from the front end — the same seam
/// `send_with` and `void_with` use.
pub struct Senders<'a, G, P, M> {
    pub branding: &'a Branding<'a>,
    pub gateway: &'a G,
    pub publisher: &'a P,
    pub mailer: &'a M,
}

/// One invoice a run produced, and what happened to it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Generated {
    pub schedule_id: i64,
    /// The cycle's scheduled issue date, which is also the invoice's.
    pub period: String,
    pub invoice_id: i64,
    pub number: i64,
    pub client_name: String,
    pub total: f64,
    pub currency: String,
    pub sent: bool,
    /// Why an autosend schedule's invoice is still a draft, or `None`. Never
    /// silently empty on a schedule that asked to send.
    pub not_sent: Option<String>,
}

/// A schedule whose walk stopped, and where.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleFailure {
    pub schedule_id: i64,
    pub period: String,
    pub message: String,
}

/// What one run did. Data rather than a print, so a terminal and a browser can
/// render the same run.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleRunReport {
    pub generated: Vec<Generated>,
    pub failures: Vec<ScheduleFailure>,
}

impl ScheduleRunReport {
    /// Anything a scheduled job must see in the exit status: a schedule that
    /// could not be generated, or a send that was asked for and did not happen.
    pub fn has_failures(&self) -> bool {
        !self.failures.is_empty() || self.generated.iter().any(|g| g.not_sent.is_some())
    }
}

/// The sentence an autosend schedule earns on an installation that cannot send.
const NOT_CONFIGURED: &str = "sending is not configured on this installation";

/// Generate every invoice currently due, drafting all of them.
///
/// `run_due_schedules` with nothing to send through: an autosend schedule still
/// gets its draft and is reported as unsent, which is what makes an
/// unconfigured installation honest rather than silent.
pub fn draft_due_schedules(conn: &Connection, today: &str) -> Result<ScheduleRunReport> {
    run_due_schedules::<
        crate::invoicing::stripe::StripeClient,
        crate::invoicing::r2::R2Publisher,
        crate::invoicing::mailgun::MailgunClient,
    >(conn, today, None)
}

/// Generate every invoice currently due, sending the ones whose schedule asked.
///
/// For each active schedule, periods are walked from `next_period` through
/// `today`: **each missed cycle generates its own invoice, dated that period's
/// issue date rather than the run day**, so catch-up bills every missed cycle in
/// order and a February invoice generated in March is already due.
///
/// Each generation is one transaction — the invoice, the run row recording which
/// schedule and which period produced it, and the advanced `next_period`
/// together. Sequential numbering falls out of `next_number` running inside it.
/// A rerun finds the `UNIQUE(schedule_id, period)` row and generates nothing.
///
/// Sending happens **after** that transaction commits, because it reaches the
/// network. A send that fails leaves the invoice the draft it already was, and
/// the report names it and why.
pub fn run_due_schedules<G: PaymentGateway, P: AssetPublisher, M: Mailer>(
    conn: &Connection,
    today: &str,
    senders: Option<&Senders<'_, G, P, M>>,
) -> Result<ScheduleRunReport> {
    let today = validate_date(today, "run")?;
    let mut report = ScheduleRunReport::default();

    for schedule in list_schedules(conn, ScheduleScope::Active)? {
        let mut cursor = schedule.next_period.clone();
        // ISO YYYY-MM-DD dates compare correctly as strings, and both sides are
        // padded by their own writers.
        while cursor <= today {
            match generate_period(conn, &schedule, &cursor, &today) {
                Ok(Some(invoice_id)) => {
                    report
                        .generated
                        .push(describe(conn, &schedule, &cursor, invoice_id)?);
                }
                Ok(None) => {}
                Err(e) => {
                    // One sick schedule does not stop the others: the rest of the
                    // book still has invoices that are due.
                    report.failures.push(ScheduleFailure {
                        schedule_id: schedule.id,
                        period: cursor.clone(),
                        message: e.to_string(),
                    });
                    break;
                }
            }
            cursor = advance_period(
                Cadence::parse(&schedule.cadence)?,
                schedule.anchor_day,
                &cursor,
            )?;
        }
    }

    for generated in &mut report.generated {
        if !autosend_for(conn, generated.schedule_id)? {
            continue;
        }
        match senders {
            None => generated.not_sent = Some(NOT_CONFIGURED.to_string()),
            Some(senders) => match crate::invoicing::send::send_invoice(
                conn,
                generated.invoice_id,
                &today,
                senders.branding,
                senders.gateway,
                senders.publisher,
                senders.mailer,
            ) {
                Ok(_) => generated.sent = true,
                Err(e) => generated.not_sent = Some(e.to_string()),
            },
        }
    }

    Ok(report)
}

fn autosend_for(conn: &Connection, schedule_id: i64) -> Result<bool> {
    Ok(get_schedule(conn, schedule_id)?.autosend)
}

/// One period, in one transaction. `Ok(None)` means this period was already
/// generated — the run row is the authority, so the cycle advances and nothing
/// is billed twice.
fn generate_period(
    conn: &Connection,
    schedule: &Schedule,
    period: &str,
    generated_at: &str,
) -> Result<Option<i64>> {
    let next = advance_period(
        Cadence::parse(&schedule.cadence)?,
        schedule.anchor_day,
        period,
    )?;
    let tx = conn.unchecked_transaction()?;

    let already: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM invoice_schedule_runs WHERE schedule_id = ?1 AND period = ?2)",
        rusqlite::params![schedule.id, period],
        |r| r.get(0),
    )?;
    if already {
        tx.execute(
            "UPDATE invoice_schedules SET next_period = ?2 WHERE id = ?1 AND next_period <= ?2",
            rusqlite::params![schedule.id, next],
        )?;
        tx.commit()?;
        return Ok(None);
    }

    let items = schedule_items(&tx, schedule.id)?;
    let due_date = schedule
        .net_days
        .map(|days| crate::invoicing::invoices::plus_days(period, days))
        .transpose()?;
    let invoice_id = crate::invoicing::invoices::insert_invoice(
        &tx,
        schedule.client_id,
        period,
        due_date.as_deref(),
        &schedule.currency,
        &items,
        schedule.notes.as_deref(),
        schedule.terms.as_deref(),
    )?;
    tx.execute(
        "INSERT INTO invoice_schedule_runs (schedule_id, period, invoice_id, generated_at)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![schedule.id, period, invoice_id, generated_at],
    )?;
    tx.execute(
        "UPDATE invoice_schedules SET next_period = ?2 WHERE id = ?1",
        rusqlite::params![schedule.id, next],
    )?;
    tx.commit()?;
    Ok(Some(invoice_id))
}

fn describe(
    conn: &Connection,
    schedule: &Schedule,
    period: &str,
    invoice_id: i64,
) -> Result<Generated> {
    let invoice = crate::invoicing::invoices::get_invoice(conn, invoice_id)?;
    let client = crate::invoicing::clients::get_client(conn, schedule.client_id)?;
    Ok(Generated {
        schedule_id: schedule.id,
        period: period.to_string(),
        invoice_id,
        number: invoice.number,
        client_name: client.name,
        total: invoice.total,
        currency: invoice.currency,
        sent: false,
        not_sent: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::invoicing::clients::{add_client, archive_client};
    use crate::migrations::run_migrations;

    pub(super) fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    pub(super) fn sample_items() -> Vec<NewLineItem> {
        vec![NewLineItem {
            description: "Hosting & maintenance".into(),
            quantity: 1.0,
            unit_amount: 450.0,
        }]
    }

    pub(super) fn seed(conn: &Connection) -> (i64, i64) {
        let client = add_client(conn, "Cedar Systems", Some("ops@cedar.test"), None, None).unwrap();
        let schedule = add_schedule(
            conn,
            &NewSchedule {
                client_id: client,
                cadence: Cadence::Monthly,
                anchor_day: 1,
                start_period: "2026-01-01".into(),
                net_days: Some(30),
                currency: "USD".into(),
                notes: Some("Monthly hosting.".into()),
                terms: Some("Net 30.".into()),
                autosend: false,
                items: sample_items(),
            },
        )
        .unwrap();
        (client, schedule)
    }

    #[test]
    fn a_schedule_stores_its_shape_and_starts_at_its_first_period() {
        let (_d, conn) = test_conn();
        let (client, id) = seed(&conn);

        let schedule = get_schedule(&conn, id).unwrap();
        assert_eq!(schedule.client_id, client);
        assert_eq!(schedule.cadence, "monthly");
        assert_eq!(schedule.anchor_day, 1);
        assert_eq!(schedule.next_period, "2026-01-01");
        assert_eq!(schedule.net_days, Some(30));
        assert_eq!(schedule.currency, "USD");
        assert_eq!(schedule.notes.as_deref(), Some("Monthly hosting."));
        assert_eq!(schedule.terms.as_deref(), Some("Net 30."));
        assert!(!schedule.autosend);
        assert!(!schedule.paused);
        assert_eq!(schedule.ended_at, None);

        let stored = schedule_items(&conn, id).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].description, "Hosting & maintenance");
        assert_eq!(stored[0].unit_amount, 450.0);
    }

    #[test]
    fn a_schedule_refuses_what_an_invoice_would_refuse() {
        let (_d, conn) = test_conn();
        let client = add_client(&conn, "Globex", Some("ap@globex.test"), None, None).unwrap();
        let base = NewSchedule {
            client_id: client,
            cadence: Cadence::Monthly,
            anchor_day: 1,
            start_period: "2026-01-01".into(),
            net_days: None,
            currency: "USD".into(),
            notes: None,
            terms: None,
            autosend: false,
            items: sample_items(),
        };

        let empty = NewSchedule {
            items: vec![],
            ..base.clone()
        };
        assert!(add_schedule(&conn, &empty).is_err(), "no line items");

        let bad_currency = NewSchedule {
            currency: "DOLLARS".into(),
            ..base.clone()
        };
        assert!(add_schedule(&conn, &bad_currency).is_err(), "currency");

        let bad_date = NewSchedule {
            start_period: "26-1-1".into(),
            ..base.clone()
        };
        assert!(add_schedule(&conn, &bad_date).is_err(), "start period");

        let bad_anchor = NewSchedule {
            anchor_day: 32,
            ..base.clone()
        };
        assert!(add_schedule(&conn, &bad_anchor).is_err(), "anchor day");

        archive_client(&conn, client, "2026-02-01").unwrap();
        let err = add_schedule(&conn, &base).unwrap_err();
        assert!(
            matches!(
                err,
                NigelError::Conflict {
                    code: "client_archived",
                    ..
                }
            ),
            "got: {err:?}"
        );
    }

    #[test]
    fn cadence_round_trips_through_its_stored_word() {
        for (word, cadence, months) in [
            ("monthly", Cadence::Monthly, 1),
            ("quarterly", Cadence::Quarterly, 3),
            ("yearly", Cadence::Yearly, 12),
        ] {
            assert_eq!(Cadence::parse(word).unwrap(), cadence);
            assert_eq!(cadence.as_str(), word);
            assert_eq!(cadence.months(), months);
        }
        assert!(Cadence::parse("weekly").is_err());
    }

    #[test]
    fn a_monthly_anchor_clamps_in_short_months_and_returns_to_the_anchor() {
        // AC #6: the anchor is remembered, never the clamped day it produced.
        let walk = [
            ("2026-01-31", "2026-02-28"),
            ("2026-02-28", "2026-03-31"),
            ("2026-03-31", "2026-04-30"),
            ("2026-04-30", "2026-05-31"),
            ("2026-11-30", "2026-12-31"),
            ("2026-12-31", "2027-01-31"),
            // A leap February takes the 29th.
            ("2028-01-31", "2028-02-29"),
        ];
        for (from, to) in walk {
            assert_eq!(
                advance_period(Cadence::Monthly, 31, from).unwrap(),
                to,
                "{from}"
            );
        }

        assert_eq!(
            advance_period(Cadence::Quarterly, 30, "2026-01-30").unwrap(),
            "2026-04-30"
        );
        assert_eq!(
            advance_period(Cadence::Quarterly, 31, "2026-11-30").unwrap(),
            "2027-02-28"
        );
        assert_eq!(
            advance_period(Cadence::Yearly, 29, "2028-02-29").unwrap(),
            "2029-02-28"
        );
    }

    #[test]
    fn clamp_day_answers_the_last_valid_day_of_the_month() {
        assert_eq!(clamp_day(2026, 2, 31), 28);
        assert_eq!(clamp_day(2028, 2, 31), 29);
        assert_eq!(clamp_day(2026, 4, 31), 30);
        assert_eq!(clamp_day(2026, 12, 31), 31);
        assert_eq!(clamp_day(2026, 1, 15), 15);
        // December rolls into the next January rather than off the end of the
        // year, and every month before it stays inside its own.
        assert_eq!(clamp_day(2026, 11, 31), 30);
    }

    /// The rollover is December's, so a month outside 1-12 is refused rather
    /// than answered with the 31 that fallback used to produce.
    #[test]
    #[should_panic(expected = "month 13 is outside 1-12")]
    fn clamp_day_refuses_a_month_that_is_not_a_month() {
        clamp_day(2026, 13, 31);
    }

    #[test]
    fn pausing_resuming_and_ending_leave_the_row_and_its_history_alone() {
        // AC #8.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);

        pause_schedule(&conn, id, "2026-01-01").unwrap();
        assert!(get_schedule(&conn, id).unwrap().paused);
        assert!(list_schedules(&conn, ScheduleScope::Active)
            .unwrap()
            .is_empty());
        assert_eq!(list_schedules(&conn, ScheduleScope::All).unwrap().len(), 1);

        resume_schedule(&conn, id, "2026-01-01").unwrap();
        assert!(!get_schedule(&conn, id).unwrap().paused);
        assert_eq!(
            list_schedules(&conn, ScheduleScope::Active).unwrap().len(),
            1
        );

        end_schedule(&conn, id, "2026-08-20", EndDisposition::Forgive).unwrap();
        let ended = get_schedule(&conn, id).unwrap();
        assert_eq!(ended.ended_at.as_deref(), Some("2026-08-20"));
        assert!(list_schedules(&conn, ScheduleScope::Active)
            .unwrap()
            .is_empty());
        // Nothing is deleted: the row and its items are still readable.
        assert_eq!(schedule_items(&conn, id).unwrap().len(), 1);
        assert_eq!(list_schedules(&conn, ScheduleScope::All).unwrap().len(), 1);
    }

    #[test]
    fn editing_replaces_only_what_is_given() {
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);

        update_schedule(
            &conn,
            id,
            &ScheduleUpdate {
                net_days: Some(Some(14)),
                autosend: Some(true),
                items: Some(vec![NewLineItem {
                    description: "Hosting & maintenance".into(),
                    quantity: 1.0,
                    unit_amount: 495.0,
                }]),
                ..ScheduleUpdate::default()
            },
        )
        .unwrap();

        let schedule = get_schedule(&conn, id).unwrap();
        assert_eq!(schedule.net_days, Some(14));
        assert!(schedule.autosend);
        assert_eq!(schedule.currency, "USD", "an omitted field is left alone");
        assert_eq!(schedule.notes.as_deref(), Some("Monthly hosting."));
        assert_eq!(
            schedule.next_period, "2026-01-01",
            "editing never moves the cycle"
        );

        let stored = schedule_items(&conn, id).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].unit_amount, 495.0);

        // `null` clears, which is what a nested `Some(None)` means.
        update_schedule(
            &conn,
            id,
            &ScheduleUpdate {
                net_days: Some(None),
                notes: Some(None),
                ..ScheduleUpdate::default()
            },
        )
        .unwrap();
        let schedule = get_schedule(&conn, id).unwrap();
        assert_eq!(schedule.net_days, None);
        assert_eq!(schedule.notes, None);

        let empty = update_schedule(&conn, id, &ScheduleUpdate::default());
        assert!(empty.is_err(), "an edit with nothing in it is a refusal");
    }

    #[test]
    fn a_schedule_that_is_not_there_is_not_found() {
        let (_d, conn) = test_conn();
        for result in [
            get_schedule(&conn, 99).map(|_| ()),
            pause_schedule(&conn, 99, "2026-01-01"),
            resume_schedule(&conn, 99, "2026-01-01").map(|_| ()),
            end_schedule(&conn, 99, "2026-08-20", EndDisposition::RefuseIfOwed).map(|_| ()),
            unbilled_periods(&conn, 99, "2026-08-20").map(|_| ()),
        ] {
            assert!(matches!(result.unwrap_err(), NigelError::NotFound(_)));
        }
    }

    fn numbers(report: &ScheduleRunReport) -> Vec<i64> {
        report.generated.iter().map(|g| g.number).collect()
    }

    /// The three destructures these tests need, so each asserts the shape it
    /// expects rather than reading emptiness to infer what happened.
    fn ended_on(outcome: &EndOutcome) -> &str {
        match outcome {
            EndOutcome::Ended { on, .. } => on,
            other => panic!("expected an ended schedule, got {other:?}"),
        }
    }

    fn settlement(outcome: &EndOutcome) -> &Settlement {
        match outcome {
            EndOutcome::Ended { settled, .. } => settled,
            other => panic!("expected an ended schedule, got {other:?}"),
        }
    }

    fn billed(outcome: &EndOutcome) -> &ScheduleRunReport {
        match settlement(outcome) {
            Settlement::Billed { report } => report,
            other => panic!("expected a billed settlement, got {other:?}"),
        }
    }

    #[test]
    fn ending_a_schedule_that_owes_nothing_is_unchanged() {
        // AC #1: the refusal is about owed periods, and a schedule level with
        // its cycle has none — so every disposition ends it the same way.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);

        draft_due_schedules(&conn, "2026-04-01").unwrap();
        assert_eq!(
            unbilled_periods(&conn, id, "2026-04-01").unwrap(),
            Vec::<String>::new()
        );

        let outcome = end_schedule(&conn, id, "2026-04-01", EndDisposition::RefuseIfOwed).unwrap();
        assert_eq!(ended_on(&outcome), "2026-04-01");
        assert!(matches!(settlement(&outcome), Settlement::NothingOwed));
        assert_eq!(
            get_schedule(&conn, id).unwrap().ended_at.as_deref(),
            Some("2026-04-01")
        );
    }

    #[test]
    fn ending_with_periods_still_owed_refuses_and_writes_nothing() {
        // AC #1. The whole point: neither answer is taken on the operator's
        // behalf, so the schedule is untouched until they say which they meant.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);

        assert_eq!(
            unbilled_periods(&conn, id, "2026-05-15").unwrap(),
            [
                "2026-01-01",
                "2026-02-01",
                "2026-03-01",
                "2026-04-01",
                "2026-05-01"
            ]
        );

        let err = end_schedule(&conn, id, "2026-05-15", EndDisposition::RefuseIfOwed).unwrap_err();
        let NigelError::Conflict { code, message } = &err else {
            panic!("expected a conflict naming the periods, got {err:?}");
        };
        assert_eq!(*code, "schedule_has_unbilled_periods");
        assert_eq!(
            message,
            &format!(
                "Schedule {id} has 5 unbilled periods on or before 2026-05-15: \
                 2026-01-01, 2026-02-01, 2026-03-01, 2026-04-01, 2026-05-01. \
                 Bill them or forgive them — ending cannot decide that for you."
            )
        );

        let schedule = get_schedule(&conn, id).unwrap();
        assert_eq!(schedule.ended_at, None, "a refusal ends nothing");
        assert_eq!(schedule.next_period, "2026-01-01", "and moves nothing");
        assert!(schedule_runs(&conn, id).unwrap().is_empty());
    }

    #[test]
    fn forgiving_ends_the_schedule_and_bills_none_of_what_it_owed() {
        // AC #1.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);

        let outcome = end_schedule(&conn, id, "2026-05-01", EndDisposition::Forgive).unwrap();
        assert_eq!(ended_on(&outcome), "2026-05-01");
        let Settlement::Forgiven { periods } = settlement(&outcome) else {
            panic!("expected a forgiven settlement");
        };
        assert_eq!(periods.len(), 5);

        assert!(
            schedule_runs(&conn, id).unwrap().is_empty(),
            "nothing was billed"
        );
        assert_eq!(
            get_schedule(&conn, id).unwrap().next_period,
            "2026-01-01",
            "next_period records where it stopped"
        );
        assert!(draft_due_schedules(&conn, "2026-12-01")
            .unwrap()
            .generated
            .is_empty());
    }

    #[test]
    fn billing_issues_nothing_for_a_period_after_the_end_date() {
        // AC #2. The end date deliberately falls mid-cycle: that distinguishes
        // the real rule from "bill through the end of the cycle it lands in",
        // which a boundary date cannot tell apart.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);

        let outcome = end_schedule(&conn, id, "2026-05-15", EndDisposition::Bill).unwrap();
        assert_eq!(ended_on(&outcome), "2026-05-15");
        assert_eq!(
            periods(billed(&outcome)),
            [
                "2026-01-01",
                "2026-02-01",
                "2026-03-01",
                "2026-04-01",
                "2026-05-01"
            ],
            "June is a cycle too far even though May 15 sits inside May's"
        );

        let ended_at = get_schedule(&conn, id).unwrap().ended_at.unwrap();
        for run in schedule_runs(&conn, id).unwrap() {
            assert!(
                run.period <= ended_at,
                "period {} is dated after the schedule ended on {ended_at}",
                run.period
            );
        }
        // Only the issue date is bounded. The seed is Net 30, so the last
        // invoice falls due after the schedule ended — which is correct, and is
        // why the docs say "no invoice is issued for a period after the end
        // date" rather than "nothing is dated after it".
        assert_eq!(
            issued(&conn, 1248),
            ("2026-01-01".into(), Some("2026-01-31".into()))
        );
        assert_eq!(
            issued(&conn, 1252),
            ("2026-05-01".into(), Some("2026-05-31".into()))
        );
    }

    #[test]
    fn billing_a_quarterly_schedule_restores_its_anchor_after_a_short_month() {
        // The one thing clamping could plausibly get wrong on this path: the
        // anchor advances, never the clamped day it produced last time.
        let (_d, conn) = test_conn();
        let client =
            add_client(&conn, "Juniper Labs", Some("ap@juniper.test"), None, None).unwrap();
        let id = add_schedule(
            &conn,
            &NewSchedule {
                client_id: client,
                cadence: Cadence::Quarterly,
                anchor_day: 31,
                start_period: "2025-12-31".into(),
                net_days: None,
                currency: "USD".into(),
                notes: None,
                terms: None,
                autosend: false,
                items: sample_items(),
            },
        )
        .unwrap();

        let outcome = end_schedule(&conn, id, "2026-09-30", EndDisposition::Bill).unwrap();
        assert_eq!(
            periods(billed(&outcome)),
            ["2025-12-31", "2026-03-31", "2026-06-30", "2026-09-30"]
        );
        assert_eq!(
            get_schedule(&conn, id).unwrap().next_period,
            "2026-12-31",
            "the anchor is restored, not stuck on the clamped 30th"
        );
    }

    #[test]
    fn billing_several_at_once_keeps_the_numbering_sequential() {
        // The property TASK-81 AC #7 pinned for a run, holding on this path too.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);

        let outcome = end_schedule(&conn, id, "2026-04-01", EndDisposition::Bill).unwrap();
        assert_eq!(numbers(billed(&outcome)), [1248, 1249, 1250, 1251]);
    }

    #[test]
    fn billing_drafts_even_when_the_schedule_autosends() {
        // Ending is deliberate and someone is present. Drafting is the default
        // generation already takes, and this path never reaches a mailer.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);
        update_schedule(
            &conn,
            id,
            &ScheduleUpdate {
                autosend: Some(true),
                ..ScheduleUpdate::default()
            },
        )
        .unwrap();

        let outcome = end_schedule(&conn, id, "2026-02-01", EndDisposition::Bill).unwrap();
        assert_eq!(billed(&outcome).generated.len(), 2);
        for generated in &billed(&outcome).generated {
            assert!(!generated.sent, "ending never sends");
        }
        for run in schedule_runs(&conn, id).unwrap() {
            let invoice =
                crate::invoicing::invoices::get_invoice(&conn, run.invoice_id.unwrap()).unwrap();
            assert_eq!(invoice.status, "draft");
        }
    }

    #[test]
    fn billing_that_fails_leaves_the_schedule_alive_to_retry() {
        // Ending on a broken walk would strand the periods it never reached,
        // with nothing left active to generate them.
        //
        // `ensure_client_active` fails uniformly, so this stops on the first
        // period rather than partway — it pins the not-ended outcome, not the
        // partial-commit report.
        let (_d, conn) = test_conn();
        let (client, id) = seed(&conn);
        archive_client(&conn, client, "2026-01-01").unwrap();

        let outcome = end_schedule(&conn, id, "2026-05-01", EndDisposition::Bill).unwrap();
        let EndOutcome::BillingStopped { billed, owed } = &outcome else {
            panic!("expected the walk to stop, got {outcome:?}");
        };
        assert_eq!(*owed, 5, "the caller can say how much is still outstanding");
        assert_eq!(billed.failures.len(), 1);
        assert_eq!(get_schedule(&conn, id).unwrap().ended_at, None);
    }

    #[test]
    fn a_period_already_billed_is_not_owed_a_second_time() {
        // The run row is the authority here exactly as it is in a run.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);
        draft_due_schedules(&conn, "2026-02-01").unwrap();

        assert_eq!(
            unbilled_periods(&conn, id, "2026-04-01").unwrap(),
            ["2026-03-01", "2026-04-01"]
        );
        let outcome = end_schedule(&conn, id, "2026-04-01", EndDisposition::Bill).unwrap();
        assert_eq!(periods(billed(&outcome)), ["2026-03-01", "2026-04-01"]);
        assert_eq!(schedule_runs(&conn, id).unwrap().len(), 4);
    }

    #[test]
    fn ending_an_already_ended_schedule_is_refused() {
        // Forgiving leaves `next_period` where it stood, so without this guard
        // a second end walks the same gap again — billing periods dated after
        // the day the schedule stopped, and moving `ended_at` forward to cover
        // them. Forgiveness has to be durable.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);
        end_schedule(&conn, id, "2026-05-01", EndDisposition::Forgive).unwrap();

        for disposition in [
            EndDisposition::RefuseIfOwed,
            EndDisposition::Forgive,
            EndDisposition::Bill,
        ] {
            let err = end_schedule(&conn, id, "2026-08-01", disposition).unwrap_err();
            let NigelError::Conflict { code, .. } = &err else {
                panic!("expected a conflict, got {err:?}");
            };
            assert_eq!(*code, "schedule_already_ended");
        }

        let schedule = get_schedule(&conn, id).unwrap();
        assert_eq!(schedule.ended_at.as_deref(), Some("2026-05-01"));
        assert!(schedule_runs(&conn, id).unwrap().is_empty());
        // And the query says so too, rather than reporting a backlog nobody
        // can act on.
        assert!(unbilled_periods(&conn, id, "2026-08-01")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_pause_forgives_the_cycles_it_covers_but_not_the_arrears_behind_it() {
        // Skipping those cycles is the point of pausing, so they are never
        // owed — not on the way out here, and not on a later resume.
        //
        // Arrears are a different thing. A schedule can be behind before it is
        // paused, and those are months worked and never invoiced; pausing in
        // March says nothing about January. `paused_at` is what tells them
        // apart, which is the whole reason the column exists.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);
        pause_schedule(&conn, id, "2026-03-01").unwrap();

        assert_eq!(
            unbilled_periods(&conn, id, "2026-06-01").unwrap(),
            ["2026-01-01", "2026-02-01"],
            "January and February predate the pause; March onward it covers"
        );

        // So ending it offers only the arrears, and billing them invoices
        // nothing from inside the pause.
        let outcome = end_schedule(&conn, id, "2026-06-01", EndDisposition::Bill).unwrap();
        assert_eq!(periods(billed(&outcome)), ["2026-01-01", "2026-02-01"]);
    }

    #[test]
    fn a_pause_with_nothing_behind_it_leaves_a_schedule_owing_nothing() {
        // The ordinary wind-down: a schedule that was current when it was
        // paused owes nothing, so `end` takes neither flag.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);
        pause_schedule(&conn, id, "2026-01-01").unwrap();

        assert!(unbilled_periods(&conn, id, "2026-06-01")
            .unwrap()
            .is_empty());
        let outcome = end_schedule(&conn, id, "2026-06-01", EndDisposition::RefuseIfOwed).unwrap();
        assert!(matches!(settlement(&outcome), Settlement::NothingOwed));
    }

    #[test]
    fn a_pause_from_before_the_column_existed_forgives_nothing() {
        // Migration v13 leaves `paused_at` NULL on a schedule already paused,
        // because the date is not recoverable. An unknown extent must not write
        // off a cycle the operator meant to bill, so such a pause forgives
        // nothing and ending still asks.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);
        conn.execute(
            "UPDATE invoice_schedules SET paused = 1, paused_at = NULL WHERE id = ?1",
            [id],
        )
        .unwrap();

        assert_eq!(unbilled_periods(&conn, id, "2026-05-01").unwrap().len(), 5);
        let err = end_schedule(&conn, id, "2026-05-01", EndDisposition::RefuseIfOwed).unwrap_err();
        assert!(matches!(err, NigelError::Conflict { .. }));
    }

    fn periods(report: &ScheduleRunReport) -> Vec<String> {
        report.generated.iter().map(|g| g.period.clone()).collect()
    }

    fn issued(conn: &Connection, number: i64) -> (String, Option<String>) {
        conn.query_row(
            "SELECT issue_date, due_date FROM invoices WHERE number = ?1",
            [number],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    }

    #[test]
    fn a_run_generates_every_missed_cycle_dated_by_its_own_period() {
        // AC #5: catch-up bills every missed cycle, in order, dated the period's
        // issue date rather than the run day.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);

        let report = draft_due_schedules(&conn, "2026-04-15").unwrap();
        assert_eq!(
            periods(&report),
            ["2026-01-01", "2026-02-01", "2026-03-01", "2026-04-01"]
        );
        assert_eq!(
            numbers(&report),
            [1248, 1249, 1250, 1251],
            "AC #7: sequential"
        );

        assert_eq!(
            issued(&conn, 1248),
            ("2026-01-01".into(), Some("2026-01-31".into()))
        );
        assert_eq!(
            issued(&conn, 1251),
            ("2026-04-01".into(), Some("2026-05-01".into()))
        );

        assert_eq!(get_schedule(&conn, id).unwrap().next_period, "2026-05-01");
        for generated in &report.generated {
            assert!(!generated.sent, "AC #4: drafting is the default");
            assert_eq!(generated.not_sent, None);
            assert_eq!(generated.client_name, "Cedar Systems");
            assert_eq!(generated.total, 450.0);
        }
        assert!(report.failures.is_empty());
        assert!(!report.has_failures());
    }

    #[test]
    fn running_twice_for_the_same_period_generates_nothing_the_second_time() {
        // AC #3, by recorded provenance rather than date inference.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);

        let first = draft_due_schedules(&conn, "2026-02-15").unwrap();
        assert_eq!(numbers(&first), [1248, 1249]);

        // Rewound to the first period so the second run really walks the two
        // cycles it already billed. Left where the first run put it,
        // `next_period` is already past `today` and the loop body never runs —
        // the run-row check this test is about would never be consulted.
        conn.execute(
            "UPDATE invoice_schedules SET next_period = '2026-01-01' WHERE id = ?1",
            [id],
        )
        .unwrap();

        let second = draft_due_schedules(&conn, "2026-02-15").unwrap();
        assert!(second.generated.is_empty(), "{:?}", second.generated);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM invoices", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);

        let runs = schedule_runs(&conn, id).unwrap();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].period, "2026-01-01");
        assert_eq!(runs[0].number, Some(1248));
        assert_eq!(runs[1].period, "2026-02-01");
        assert_eq!(runs[1].generated_at, "2026-02-15");
        assert_eq!(
            get_schedule(&conn, id).unwrap().next_period,
            "2026-03-01",
            "the rewound cycle is walked back to where the first run left it"
        );
    }

    #[test]
    fn a_run_row_already_there_advances_the_cycle_without_billing_again() {
        // The row is the authority even if `next_period` was rewound by hand.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);
        draft_due_schedules(&conn, "2026-01-15").unwrap();

        conn.execute(
            "UPDATE invoice_schedules SET next_period = '2026-01-01' WHERE id = ?1",
            [id],
        )
        .unwrap();

        let again = draft_due_schedules(&conn, "2026-01-15").unwrap();
        assert!(again.generated.is_empty());
        assert_eq!(get_schedule(&conn, id).unwrap().next_period, "2026-02-01");
    }

    #[test]
    fn a_monthly_schedule_anchored_at_month_end_bills_the_short_months() {
        // AC #6, end to end through a run.
        let (_d, conn) = test_conn();
        let client = add_client(&conn, "Initech", Some("ap@initech.test"), None, None).unwrap();
        add_schedule(
            &conn,
            &NewSchedule {
                client_id: client,
                cadence: Cadence::Monthly,
                anchor_day: 31,
                start_period: "2026-01-31".into(),
                net_days: None,
                currency: "USD".into(),
                notes: None,
                terms: None,
                autosend: false,
                items: sample_items(),
            },
        )
        .unwrap();

        let report = draft_due_schedules(&conn, "2026-05-15").unwrap();
        assert_eq!(
            periods(&report),
            ["2026-01-31", "2026-02-28", "2026-03-31", "2026-04-30"]
        );
        assert_eq!(issued(&conn, 1251), ("2026-04-30".into(), None));
    }

    /// The scenario TASK-143 names: late since January, paused in March,
    /// resumed in May. January and February were worked and never invoiced;
    /// March through May are what the pause forgave. One cursor cannot express
    /// that gap, which is why the forgiven periods are written as rows.
    #[test]
    fn resuming_forgives_the_cycles_the_pause_covered_and_still_owes_what_predates_it() {
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);

        pause_schedule(&conn, id, "2026-03-01").unwrap();
        let skipped = resume_schedule(&conn, id, "2026-05-15").unwrap();
        assert_eq!(
            skipped,
            ["2026-03-01", "2026-04-01", "2026-05-01"],
            "the pause forgives every cycle it covered, up to the day it lifted"
        );

        let report = draft_due_schedules(&conn, "2026-05-15").unwrap();
        assert_eq!(
            periods(&report),
            ["2026-01-01", "2026-02-01"],
            "arrears behind the pause are still owed"
        );

        // The forgiven periods are unbillable because they carry rows, so the
        // walk that already skips a generated period skips these for free.
        let again = draft_due_schedules(&conn, "2026-05-15").unwrap();
        assert!(again.generated.is_empty(), "a second run bills nothing");
    }

    #[test]
    fn a_forgiven_cycle_is_recorded_rather_than_merely_absent() {
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);
        draft_due_schedules(&conn, "2026-02-15").unwrap();

        pause_schedule(&conn, id, "2026-03-01").unwrap();
        resume_schedule(&conn, id, "2026-04-15").unwrap();

        let runs = schedule_runs(&conn, id).unwrap();
        let shape: Vec<(&str, Option<i64>, Option<&str>)> = runs
            .iter()
            .map(|r| (r.period.as_str(), r.number, r.skipped_reason.as_deref()))
            .collect();
        assert_eq!(
            shape,
            [
                ("2026-01-01", Some(1248), None),
                ("2026-02-01", Some(1249), None),
                ("2026-03-01", None, Some("paused 2026-03-01")),
                ("2026-04-01", None, Some("paused 2026-03-01")),
            ],
            "a skipped period is listed beside the billed ones, and says why"
        );
    }

    /// The database holds the invariant rather than each writer remembering it.
    #[test]
    fn a_run_row_cannot_be_both_an_invoice_and_a_skip_or_neither() {
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);
        draft_due_schedules(&conn, "2026-01-15").unwrap();
        let invoice: i64 = conn
            .query_row("SELECT id FROM invoices LIMIT 1", [], |r| r.get(0))
            .unwrap();

        for (invoice_id, reason) in [(Some(invoice), Some("paused 2026-03-01")), (None, None)] {
            let refused = conn.execute(
                "INSERT INTO invoice_schedule_runs
                     (schedule_id, period, invoice_id, generated_at, skipped_reason)
                 VALUES (?1, '2026-09-01', ?2, '2026-09-01', ?3)",
                rusqlite::params![id, invoice_id, reason],
            );
            assert!(
                refused.is_err(),
                "the CHECK refuses {invoice_id:?}/{reason:?}"
            );
        }
    }

    /// AC #5: a pause taken before v13 has no date, so its extent is not
    /// recoverable. Forgiving nothing bills a cycle that might have been meant
    /// to skip; forgiving blindly writes one off. The first is the recoverable
    /// mistake.
    #[test]
    fn a_pause_with_no_date_forgives_nothing_on_resume() {
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);

        pause_schedule(&conn, id, "2026-03-01").unwrap();
        conn.execute(
            "UPDATE invoice_schedules SET paused_at = NULL WHERE id = ?1",
            [id],
        )
        .unwrap();

        let skipped = resume_schedule(&conn, id, "2026-05-15").unwrap();
        assert!(skipped.is_empty(), "an undated pause forgives nothing");
        assert_eq!(
            periods(&draft_due_schedules(&conn, "2026-05-15").unwrap()),
            [
                "2026-01-01",
                "2026-02-01",
                "2026-03-01",
                "2026-04-01",
                "2026-05-01"
            ]
        );
    }

    /// AC #6. The two paths read the same rows now, so this holds by
    /// construction rather than by keeping two rules in step.
    #[test]
    fn ending_and_resuming_agree_about_what_a_paused_schedule_owes() {
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);
        pause_schedule(&conn, id, "2026-03-01").unwrap();

        let owed_if_ended = unbilled_periods(&conn, id, "2026-05-15").unwrap();
        assert_eq!(owed_if_ended, ["2026-01-01", "2026-02-01"]);

        resume_schedule(&conn, id, "2026-05-15").unwrap();
        let owed_after_resume = unbilled_periods(&conn, id, "2026-05-15").unwrap();
        assert_eq!(
            owed_after_resume, owed_if_ended,
            "resuming must not turn a forgiven cycle back into an owed one"
        );
        assert_eq!(
            periods(&draft_due_schedules(&conn, "2026-05-15").unwrap()),
            owed_if_ended,
            "and what a run bills is exactly what ending would have offered"
        );
    }

    #[test]
    fn paused_and_ended_schedules_generate_nothing() {
        let (_d, conn) = test_conn();
        let (_client, paused) = seed(&conn);
        pause_schedule(&conn, paused, "2026-01-01").unwrap();
        assert!(draft_due_schedules(&conn, "2026-06-01")
            .unwrap()
            .generated
            .is_empty());

        resume_schedule(&conn, paused, "2026-01-01").unwrap();
        end_schedule(&conn, paused, "2026-01-01", EndDisposition::Forgive).unwrap();
        assert!(draft_due_schedules(&conn, "2026-06-01")
            .unwrap()
            .generated
            .is_empty());
    }

    #[test]
    fn several_schedules_at_once_keep_the_numbering_sequential() {
        // AC #7 across schedules, not just within one.
        let (_d, conn) = test_conn();
        seed(&conn);
        let other = add_client(&conn, "Juniper Labs", Some("ap@juniper.test"), None, None).unwrap();
        add_schedule(
            &conn,
            &NewSchedule {
                client_id: other,
                cadence: Cadence::Quarterly,
                anchor_day: 1,
                start_period: "2026-01-01".into(),
                net_days: None,
                currency: "USD".into(),
                notes: None,
                terms: None,
                autosend: false,
                items: sample_items(),
            },
        )
        .unwrap();

        let report = draft_due_schedules(&conn, "2026-04-15").unwrap();
        let all = numbers(&report);
        assert_eq!(all.len(), 6, "four monthly plus two quarterly: {all:?}");
        let mut sorted = all.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, (1248..=1253).collect::<Vec<_>>(), "{all:?}");
    }

    #[test]
    fn a_schedule_whose_client_was_archived_is_reported_and_does_not_stop_the_others() {
        let (_d, conn) = test_conn();
        let (archived_client, archived) = seed(&conn);
        let ok_client = add_client(&conn, "Globex", Some("ap@globex.test"), None, None).unwrap();
        add_schedule(
            &conn,
            &NewSchedule {
                client_id: ok_client,
                cadence: Cadence::Monthly,
                anchor_day: 1,
                start_period: "2026-01-01".into(),
                net_days: None,
                currency: "USD".into(),
                notes: None,
                terms: None,
                autosend: false,
                items: sample_items(),
            },
        )
        .unwrap();
        archive_client(&conn, archived_client, "2026-01-01").unwrap();

        let report = draft_due_schedules(&conn, "2026-02-15").unwrap();
        assert_eq!(
            numbers(&report),
            [1248, 1249],
            "the healthy schedule still billed"
        );
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].schedule_id, archived);
        assert!(
            report.failures[0].message.contains("archived"),
            "{:?}",
            report.failures[0]
        );
        assert!(report.has_failures());
    }

    #[test]
    fn an_autosend_schedule_with_nothing_to_send_through_still_drafts_and_says_why() {
        // AC #9: reported, never silently skipped, never half-sent.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);
        update_schedule(
            &conn,
            id,
            &ScheduleUpdate {
                autosend: Some(true),
                ..ScheduleUpdate::default()
            },
        )
        .unwrap();

        let report = draft_due_schedules(&conn, "2026-01-15").unwrap();
        assert_eq!(numbers(&report), [1248]);
        let generated = &report.generated[0];
        assert!(!generated.sent);
        assert_eq!(
            generated.not_sent.as_deref(),
            Some("sending is not configured on this installation")
        );
        assert!(
            report.has_failures(),
            "cron has to see this in the exit status"
        );

        let status: String = conn
            .query_row("SELECT status FROM invoices WHERE number = 1248", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "draft");
    }
}

#[cfg(all(test, feature = "pdf"))]
mod autosend_tests {
    use super::tests::*;
    use super::*;
    use crate::invoicing::gateway::{
        fake_logo_publishing, AssetPublisher, Mailer, PaidSession, PaymentGateway, PaymentLink,
    };
    use crate::invoicing::render_html::DEFAULT_TEMPLATE;
    use crate::models::{Client, Invoice};
    use std::cell::RefCell;

    struct Gateway;
    impl PaymentGateway for Gateway {
        fn create_payment_link(&self, invoice: &Invoice, _c: &Client) -> Result<PaymentLink> {
            Ok(PaymentLink {
                id: format!("plink_{}", invoice.number),
                url: format!("https://pay.example.test/{}", invoice.number),
            })
        }
        fn paid_sessions(&self, _id: &str) -> Result<Vec<PaidSession>> {
            Ok(Vec::new())
        }
        fn deactivate_payment_link(&self, _id: &str) -> Result<()> {
            Ok(())
        }
    }

    struct Publisher;
    impl AssetPublisher for Publisher {
        fn publish(&self, token: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
            Ok(format!("https://billing.example.test/i/{token}/index.html"))
        }
        fn publish_page(&self, token: &str, _h: &[u8]) -> Result<String> {
            Ok(format!("https://billing.example.test/i/{token}/index.html"))
        }
        fake_logo_publishing!("https://billing.example.test/i");
    }

    #[derive(Default)]
    struct Post {
        to: RefCell<Vec<String>>,
    }
    impl Mailer for Post {
        fn send_invoice(
            &self,
            to: &str,
            _cc: &[String],
            _s: &str,
            _h: &str,
            _p: &[u8],
        ) -> Result<()> {
            self.to.borrow_mut().push(to.to_string());
            Ok(())
        }
    }

    #[test]
    fn an_autosend_schedule_sends_and_a_drafting_one_beside_it_does_not() {
        // AC #4: explicit per schedule, drafting the default.
        let (_d, conn) = test_conn();
        let (_client, drafting) = seed(&conn);
        let sending_client = crate::invoicing::clients::add_client(
            &conn,
            "Globex",
            Some("ap@globex.test"),
            None,
            None,
        )
        .unwrap();
        let sending = add_schedule(
            &conn,
            &NewSchedule {
                client_id: sending_client,
                cadence: Cadence::Monthly,
                anchor_day: 1,
                start_period: "2026-01-01".into(),
                net_days: None,
                currency: "USD".into(),
                notes: None,
                terms: None,
                autosend: true,
                items: sample_items(),
            },
        )
        .unwrap();

        let branding = crate::invoicing::render_html::Branding {
            company: "Bluepeak",
            contact_email: "billing@bluepeak.test",
            ..crate::invoicing::render_html::Branding::with_template(DEFAULT_TEMPLATE)
        };
        let post = Post::default();
        let senders = Senders {
            branding: &branding,
            gateway: &Gateway,
            publisher: &Publisher,
            mailer: &post,
        };

        let report = run_due_schedules(&conn, "2026-01-15", Some(&senders)).unwrap();
        assert_eq!(report.generated.len(), 2);
        assert!(!report.has_failures(), "{report:?}");

        let drafted = report
            .generated
            .iter()
            .find(|g| g.schedule_id == drafting)
            .unwrap();
        assert!(!drafted.sent);
        assert_eq!(drafted.not_sent, None);

        let sent = report
            .generated
            .iter()
            .find(|g| g.schedule_id == sending)
            .unwrap();
        assert!(sent.sent);
        assert_eq!(post.to.borrow().as_slice(), ["ap@globex.test"]);

        let status: String = conn
            .query_row(
                "SELECT status FROM invoices WHERE number = ?1",
                [sent.number],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "sent");
    }

    #[test]
    fn an_unsendable_client_still_gets_its_draft_and_the_run_says_why() {
        // AC #9, with sending fully configured: the refusal is about the client.
        let (_d, conn) = test_conn();
        let no_address =
            crate::invoicing::clients::add_client(&conn, "Harbor & Vale", None, None, None)
                .unwrap();
        add_schedule(
            &conn,
            &NewSchedule {
                client_id: no_address,
                cadence: Cadence::Monthly,
                anchor_day: 1,
                start_period: "2026-01-01".into(),
                net_days: None,
                currency: "USD".into(),
                notes: None,
                terms: None,
                autosend: true,
                items: sample_items(),
            },
        )
        .unwrap();

        let branding = crate::invoicing::render_html::Branding {
            company: "Bluepeak",
            contact_email: "billing@bluepeak.test",
            ..crate::invoicing::render_html::Branding::with_template(DEFAULT_TEMPLATE)
        };
        let post = Post::default();
        let senders = Senders {
            branding: &branding,
            gateway: &Gateway,
            publisher: &Publisher,
            mailer: &post,
        };

        let report = run_due_schedules(&conn, "2026-01-15", Some(&senders)).unwrap();
        assert_eq!(report.generated.len(), 1);
        let generated = &report.generated[0];
        assert!(!generated.sent);
        assert!(
            generated
                .not_sent
                .as_deref()
                .unwrap_or_default()
                .contains("no email"),
            "got: {:?}",
            generated.not_sent
        );
        assert!(report.has_failures());
        assert!(post.to.borrow().is_empty(), "nothing was half-sent");

        let status: String = conn
            .query_row("SELECT status FROM invoices WHERE number = 1248", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "draft");
    }
}
