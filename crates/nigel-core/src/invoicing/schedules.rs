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
use crate::invoicing::invoices::{validate_currency, validate_date, validate_items, NewLineItem};

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

/// One invoice a schedule has already produced.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleRun {
    pub period: String,
    pub invoice_id: i64,
    pub number: i64,
    pub generated_at: String,
}

const SCHEDULE_COLS: &str = "id, client_id, cadence, anchor_day, next_period, net_days,
    currency, notes, terms, autosend, paused, ended_at";

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
        ended_at: r.get(11)?,
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

pub fn pause_schedule(conn: &Connection, id: i64) -> Result<()> {
    let _ = get_schedule(conn, id)?;
    conn.execute(
        "UPDATE invoice_schedules SET paused = 1 WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

pub fn resume_schedule(conn: &Connection, id: i64) -> Result<()> {
    let _ = get_schedule(conn, id)?;
    conn.execute(
        "UPDATE invoice_schedules SET paused = 0 WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

/// Stop a schedule for good. A timestamp rather than a delete, the way
/// `voided_at` and `archived_at` are: the invoices it produced keep their
/// provenance, and the history stays readable.
pub fn end_schedule(conn: &Connection, id: i64, on: &str) -> Result<()> {
    let _ = get_schedule(conn, id)?;
    let on = validate_date(on, "end")?;
    conn.execute(
        "UPDATE invoice_schedules SET ended_at = ?2 WHERE id = ?1",
        rusqlite::params![id, on],
    )?;
    Ok(())
}

/// What a schedule has already produced, oldest period first.
pub fn schedule_runs(conn: &Connection, schedule_id: i64) -> Result<Vec<ScheduleRun>> {
    let mut stmt = conn.prepare(
        "SELECT r.period, r.invoice_id, i.number, r.generated_at
           FROM invoice_schedule_runs r
           JOIN invoices i ON i.id = r.invoice_id
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
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Clamp a day to the last valid day of the given year and month.
pub fn clamp_day(year: i32, month: u32, day: u32) -> u32 {
    let first_of_next = NaiveDate::from_ymd_opt(year, month + 1, 1)
        .or_else(|| NaiveDate::from_ymd_opt(year + 1, 1, 1))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::invoicing::clients::{add_client, archive_client};
    use crate::migrations::run_migrations;

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    fn items() -> Vec<NewLineItem> {
        vec![NewLineItem {
            description: "Hosting & maintenance".into(),
            quantity: 1.0,
            unit_amount: 450.0,
        }]
    }

    fn seed(conn: &Connection) -> (i64, i64) {
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
                items: items(),
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
            items: items(),
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
    }

    #[test]
    fn pausing_resuming_and_ending_leave_the_row_and_its_history_alone() {
        // AC #8.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);

        pause_schedule(&conn, id).unwrap();
        assert!(get_schedule(&conn, id).unwrap().paused);
        assert!(list_schedules(&conn, ScheduleScope::Active)
            .unwrap()
            .is_empty());
        assert_eq!(list_schedules(&conn, ScheduleScope::All).unwrap().len(), 1);

        resume_schedule(&conn, id).unwrap();
        assert!(!get_schedule(&conn, id).unwrap().paused);
        assert_eq!(
            list_schedules(&conn, ScheduleScope::Active).unwrap().len(),
            1
        );

        end_schedule(&conn, id, "2026-08-20").unwrap();
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
            pause_schedule(&conn, 99),
            resume_schedule(&conn, 99),
            end_schedule(&conn, 99, "2026-08-20"),
        ] {
            assert!(matches!(result.unwrap_err(), NigelError::NotFound(_)));
        }
    }
}
