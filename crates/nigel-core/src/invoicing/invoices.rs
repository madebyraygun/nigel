use chrono::NaiveDate;
use rand::distributions::Alphanumeric;
use rand::Rng;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::{get_metadata, set_metadata};
use crate::error::{DeleteBlock, NigelError, Result};
use crate::invoicing::clients::ensure_client_active;
use crate::models::{Invoice, InvoiceLineItem, InvoicePayment, InvoiceStatus};

const NEXT_NUMBER_KEY: &str = "next_invoice_number";
const NEXT_NUMBER_DEFAULT: i64 = 1248;

/// Also `Deserialize`: a line item is a request input as well as a response
/// field, unlike every other struct in this module.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewLineItem {
    pub description: String,
    pub quantity: f64,
    pub unit_amount: f64,
}

/// Refuse line items that would write a junk row or a junk total.
///
/// It lives here, called by [`create_invoice`] and [`update_invoice`] rather
/// than by their callers, for `payment_amount`'s reason: a non-finite figure
/// poisons every later `SUM` over the column, and both front ends have to
/// refuse the same items in the same words. `--item`'s parser accepts `NaN`
/// and `inf` because `f64::from_str` does, and JSON reaches the same place with
/// an overflowing literal.
///
/// **Finite inputs do not make a finite result**, which is why the checks run
/// after the arithmetic rather than only on the figures that went into it:
/// `1e308 * 1e308` is a non-finite product of two finite factors, and two
/// finite line totals of `1e308` sum past `f64::MAX` to infinity. Either would
/// otherwise be written as the invoice's `total` and serialized as `null`
/// against a field a client has typed as a number.
///
/// Refusing a non-finite line total per line is also what makes a NaN sum
/// unreachable: NaN needs an infinity of each sign, and neither survives the
/// per-line check.
pub fn validate_items(items: &[NewLineItem]) -> Result<()> {
    if items.is_empty() {
        return Err(NigelError::Invalid(
            "An invoice needs at least one line item.".to_string(),
        ));
    }
    let mut total = 0.0;
    for item in items {
        let line_total = item.quantity * item.unit_amount;
        if !item.quantity.is_finite() || !item.unit_amount.is_finite() || !line_total.is_finite() {
            return Err(NigelError::Invalid(format!(
                "Line item '{}' needs a finite quantity and unit amount that multiply to a finite amount.",
                item.description
            )));
        }
        total += line_total;
    }
    if !total.is_finite() {
        return Err(NigelError::Invalid(
            "The line items do not add up to a finite total.".to_string(),
        ));
    }
    // Every figure is finite by now, so a plain comparison says what it means.
    if total <= 0.0 {
        return Err(NigelError::Invalid(format!(
            "An invoice must total more than zero, got {total:.2}."
        )));
    }
    Ok(())
}

pub fn gen_token() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect()
}

pub fn next_number(conn: &Connection) -> Result<i64> {
    let n = get_metadata(conn, NEXT_NUMBER_KEY)
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(NEXT_NUMBER_DEFAULT);
    Ok(n)
}

#[allow(clippy::too_many_arguments)]
pub fn create_invoice(
    conn: &Connection,
    client_id: i64,
    issue_date: &str,
    due_date: Option<&str>,
    currency: &str,
    items: &[NewLineItem],
    notes: Option<&str>,
    terms: Option<&str>,
) -> Result<i64> {
    // Before the transaction opens, so a refusal writes nothing. This is also
    // the existence check: `ensure_client_active` reads the row, so a missing
    // client is its `NotFound` rather than a second query's.
    ensure_client_active(conn, client_id)?;
    validate_items(items)?;
    let issue_date = validate_date(issue_date, "issue")?;
    let due_date = match due_date {
        Some(due) => Some(validate_date(due, "due")?),
        None => None,
    };
    let currency = validate_currency(currency)?;
    let tx = conn.unchecked_transaction()?;

    let number = next_number(&tx)?;
    let subtotal: f64 = items.iter().map(|i| i.quantity * i.unit_amount).sum();
    let tax = 0.0;
    let total = subtotal + tax;
    let token = gen_token();

    tx.execute(
        "INSERT INTO invoices
            (number, client_id, issue_date, due_date, status, currency, subtotal, tax, total, notes, terms, token)
         VALUES (?1, ?2, ?3, ?4, 'draft', ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            number, client_id, issue_date, due_date, currency, subtotal, tax, total, notes, terms, token
        ],
    )?;
    let invoice_id = tx.last_insert_rowid();

    for (idx, item) in items.iter().enumerate() {
        let line_total = item.quantity * item.unit_amount;
        tx.execute(
            "INSERT INTO invoice_line_items
                (invoice_id, description, quantity, unit_amount, line_total, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                invoice_id,
                item.description,
                item.quantity,
                item.unit_amount,
                line_total,
                idx as i64
            ],
        )?;
    }

    set_metadata(&tx, NEXT_NUMBER_KEY, &(number + 1).to_string())?;
    tx.commit()?;
    Ok(invoice_id)
}

/// What an invoice *is*, independent of what state it is in.
///
/// The one reader both duplication and a schedule seed go through, so
/// "duplicate this invoice" and "bill this shape every month" cannot drift
/// apart about what gets carried across.
#[derive(Debug, Clone)]
pub struct InvoiceShape {
    pub client_id: i64,
    pub currency: String,
    pub notes: Option<String>,
    pub terms: Option<String>,
    pub items: Vec<NewLineItem>,
    /// The source's issue-to-due gap in days, or `None` when it had no due
    /// date. Days rather than a date, because the shape outlives the calendar
    /// it was first written against.
    pub net_days: Option<i64>,
}

pub fn invoice_shape(conn: &Connection, invoice_id: i64) -> Result<InvoiceShape> {
    let invoice = get_invoice(conn, invoice_id)?;
    let net_days = match invoice.due_date.as_deref() {
        Some(due) => {
            let issued = parse_date(&invoice.issue_date, "issue")?;
            Some((parse_date(due, "due")? - issued).num_days())
        }
        None => None,
    };
    let items = line_items(conn, invoice.id)?
        .into_iter()
        .map(|item| NewLineItem {
            description: item.description,
            quantity: item.quantity,
            unit_amount: item.unit_amount,
        })
        .collect();
    Ok(InvoiceShape {
        client_id: invoice.client_id,
        currency: invoice.currency,
        notes: invoice.notes,
        terms: invoice.terms,
        items,
        net_days,
    })
}

/// `date` shifted by `days`, as a zero-padded `YYYY-MM-DD`.
///
/// The one place a term in days becomes a due date: a duplicate's preserved
/// offset and a schedule's `net_days` are the same arithmetic, and two copies
/// of it would eventually disagree at a month boundary.
pub fn plus_days(date: &str, days: i64) -> Result<String> {
    let parsed = parse_date(date, "issue")?;
    parsed
        .checked_add_signed(chrono::Duration::days(days))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .ok_or_else(|| NigelError::Invalid(format!("{date} plus {days} days is not a real date.")))
}

/// Duplicate an invoice into a fresh draft.
///
/// **Copied:** client, currency, notes, terms, and every line item's
/// description, quantity and unit amount.
///
/// **Regenerated:** the number (a fresh `next_number`), the token, and
/// `status = 'draft'`. `published_at`, `voided_at` and the Stripe link fields
/// start empty — a duplicate has published nothing and been paid for nothing.
///
/// **Dates:** `issue_date` is the caller's, because nothing here reads the
/// clock. When the source carries a due date the new draft **preserves the
/// source's issue-to-due offset in days** — a Net-14 invoice duplicates as
/// Net-14 — and a source with no due date yields a draft with none.
///
/// Any source duplicates, whatever state it is in: duplication reads a shape,
/// not a state. It goes through `create_invoice`, so an archived client refuses
/// exactly as it would for a hand-created invoice.
pub fn duplicate_invoice(conn: &Connection, source_id: i64, issue_date: &str) -> Result<i64> {
    let shape = invoice_shape(conn, source_id)?;
    let issue_date = validate_date(issue_date, "issue")?;
    let due_date = shape
        .net_days
        .map(|days| plus_days(&issue_date, days))
        .transpose()?;
    create_invoice(
        conn,
        shape.client_id,
        &issue_date,
        due_date.as_deref(),
        &shape.currency,
        &shape.items,
        shape.notes.as_deref(),
        shape.terms.as_deref(),
    )
}

fn row_to_invoice(r: &rusqlite::Row) -> rusqlite::Result<Invoice> {
    Ok(Invoice {
        id: r.get(0)?,
        number: r.get(1)?,
        client_id: r.get(2)?,
        issue_date: r.get(3)?,
        due_date: r.get(4)?,
        status: r.get(5)?,
        currency: r.get(6)?,
        subtotal: r.get(7)?,
        tax: r.get(8)?,
        total: r.get(9)?,
        notes: r.get(10)?,
        terms: r.get(11)?,
        token: r.get(12)?,
        stripe_payment_link_id: r.get(13)?,
        stripe_payment_link_url: r.get(14)?,
        published_at: r.get(15)?,
        voided_at: r.get(16)?,
    })
}

const INVOICE_COLS: &str = "id, number, client_id, issue_date, due_date, status, currency,
    subtotal, tax, total, notes, terms, token, stripe_payment_link_id,
    stripe_payment_link_url, published_at, voided_at";

pub fn get_invoice(conn: &Connection, id: i64) -> Result<Invoice> {
    conn.query_row(
        &format!("SELECT {INVOICE_COLS} FROM invoices WHERE id = ?1"),
        [id],
        row_to_invoice,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            NigelError::NotFound(format!("Invoice not found: id {id}"))
        }
        other => NigelError::Db(other),
    })
}

/// An absent row is `NotFound`, exactly as it is for [`get_invoice`]. Leaving it
/// as the raw rusqlite error made "no such invoice" a 500 over HTTP, and every
/// number-keyed caller had to remember to narrow it by hand.
pub fn get_invoice_by_number(conn: &Connection, number: i64) -> Result<Invoice> {
    conn.query_row(
        &format!("SELECT {INVOICE_COLS} FROM invoices WHERE number = ?1"),
        [number],
        row_to_invoice,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            NigelError::NotFound(format!("Invoice not found: #{number}"))
        }
        other => NigelError::Db(other),
    })
}

pub fn paid_amount(conn: &Connection, invoice_id: i64) -> Result<f64> {
    let sum: Option<f64> = conn.query_row(
        "SELECT SUM(amount) FROM invoice_payments WHERE invoice_id = ?1",
        [invoice_id],
        |r| r.get(0),
    )?;
    Ok(sum.unwrap_or(0.0))
}

/// Half a cent, the finest any real balance is ever settled to.
///
/// Payments that should sum to the total land a hair under it in binary
/// floating point, so every place that asks "is this paid?" has to allow for
/// the gap. It is defined once here and used everywhere, because a document
/// that disagreed with `refresh_status` about the same invoice would print
/// "Balance due $0.01" on a page whose status says `paid`.
pub const CENT_SLACK: f64 = 0.005;

/// Is this invoice paid in full? The one settled test, inclusive at the edge.
pub fn is_settled(total: f64, paid: f64) -> bool {
    paid >= total - CENT_SLACK
}

pub fn record_payment(
    conn: &Connection,
    invoice_id: i64,
    amount: f64,
    paid_date: &str,
    method: &str,
    stripe_session: Option<&str>,
) -> Result<bool> {
    validate_payment_method(method)?;
    let paid_date = validate_date(paid_date, "payment")?;
    ensure_not_void(&get_invoice(conn, invoice_id)?, "paid")?;
    if let Some(sid) = stripe_session {
        let seen: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM invoice_payments WHERE stripe_checkout_session_id = ?1)",
            [sid],
            |r| r.get(0),
        )?;
        if seen {
            return Ok(false);
        }
    }
    conn.execute(
        "INSERT INTO invoice_payments (invoice_id, amount, paid_date, method, stripe_checkout_session_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![invoice_id, amount, paid_date, method, stripe_session],
    )?;
    // The payment date, not the wall clock, is the reference day so the derived
    // status is deterministic regardless of when the payment is entered.
    refresh_status(conn, invoice_id, &paid_date)?;
    Ok(true)
}

pub fn refresh_status(conn: &Connection, invoice_id: i64, today: &str) -> Result<String> {
    let inv = get_invoice(conn, invoice_id)?;
    if inv.voided_at.is_some() {
        let void = InvoiceStatus::Void.as_str();
        conn.execute(
            "UPDATE invoices SET status = ?1 WHERE id = ?2",
            rusqlite::params![void, invoice_id],
        )?;
        return Ok(void.to_string());
    }
    let paid = paid_amount(conn, invoice_id)?;
    let published = inv.published_at.is_some();
    let owing = inv.total - paid;

    let status = if is_settled(inv.total, paid) && inv.total > 0.0 {
        InvoiceStatus::Paid
    } else if !published {
        InvoiceStatus::Draft
    } else if is_overdue(inv.due_date.as_deref(), today) && owing > 0.0 {
        InvoiceStatus::Overdue
    } else if paid > 0.0 {
        InvoiceStatus::Partial
    } else {
        InvoiceStatus::Sent
    };

    conn.execute(
        "UPDATE invoices SET status = ?1 WHERE id = ?2",
        rusqlite::params![status.as_str(), invoice_id],
    )?;
    Ok(status.as_str().to_string())
}

fn is_overdue(due_date: Option<&str>, today: &str) -> bool {
    // ISO YYYY-MM-DD dates compare correctly as strings.
    match due_date {
        Some(d) => today > d,
        None => false,
    }
}

/// The one date rule every invoicing writer applies: a real calendar day written
/// `YYYY-MM-DD` with a four-digit year.
///
/// chrono's `%Y` alone is not that rule — it reads `26-8-9` as the year 26 AD,
/// which would book a payment two millennia off and hand `refresh_status` that
/// day as its reference. The year is therefore counted before chrono sees it.
pub fn parse_date(value: &str, what: &str) -> Result<NaiveDate> {
    let invalid = || {
        NigelError::Invalid(format!(
            "Invalid {what} date: {value} (expected YYYY-MM-DD)"
        ))
    };
    let trimmed = value.trim();
    let year = trimmed.split('-').next().unwrap_or_default();
    if year.len() != 4 || !year.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid());
    }
    NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").map_err(|_| invalid())
}

/// `YYYY-MM-DD`, zero-padded, or an `Invalid` error naming the field.
///
/// Returns the re-formatted date rather than the caller's string: `2026-8-7` is a
/// real day, and a value stored that way is never `>` its own month in
/// `is_overdue`'s string comparison. Same shape as `validate_currency` below,
/// which has always returned the normalized code.
pub fn validate_date(value: &str, what: &str) -> Result<String> {
    Ok(parse_date(value, what)?.format("%Y-%m-%d").to_string())
}

/// Normalizes a 3-letter code to uppercase, or an `Invalid` error.
pub fn validate_currency(code: &str) -> Result<String> {
    if code.len() == 3 && code.chars().all(|c| c.is_ascii_alphabetic()) {
        Ok(code.to_ascii_uppercase())
    } else {
        Err(NigelError::Invalid(format!(
            "Invalid currency: {code} (expected a 3-letter code like USD)"
        )))
    }
}

/// Fields to change on a draft invoice. Same `Option`/`Option<Option<_>>`
/// convention as `ClientUpdate`. `items: Some(v)` replaces the entire line-item
/// set; `None` leaves the existing lines alone.
#[derive(Debug, Default)]
pub struct InvoiceUpdate {
    pub issue_date: Option<String>,
    pub due_date: Option<Option<String>>,
    pub currency: Option<String>,
    pub notes: Option<Option<String>>,
    pub terms: Option<Option<String>>,
    pub items: Option<Vec<NewLineItem>>,
}

impl InvoiceUpdate {
    pub fn is_empty(&self) -> bool {
        self.issue_date.is_none()
            && self.due_date.is_none()
            && self.currency.is_none()
            && self.notes.is_none()
            && self.terms.is_none()
            && self.items.is_none()
    }
}

/// Rewrite an invoice's line items at dense positions `0..n-1`, returning the
/// recomputed `(subtotal, total)`. `tax` is read from the row and left alone.
fn replace_line_items(
    conn: &Connection,
    invoice_id: i64,
    items: &[NewLineItem],
) -> Result<(f64, f64)> {
    conn.execute(
        "DELETE FROM invoice_line_items WHERE invoice_id = ?1",
        [invoice_id],
    )?;
    for (idx, item) in items.iter().enumerate() {
        let line_total = item.quantity * item.unit_amount;
        conn.execute(
            "INSERT INTO invoice_line_items
                (invoice_id, description, quantity, unit_amount, line_total, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                invoice_id,
                item.description,
                item.quantity,
                item.unit_amount,
                line_total,
                idx as i64
            ],
        )?;
    }
    let subtotal: f64 = items.iter().map(|i| i.quantity * i.unit_amount).sum();
    let tax: f64 = conn.query_row(
        "SELECT tax FROM invoices WHERE id = ?1",
        [invoice_id],
        |r| r.get(0),
    )?;
    Ok((subtotal, subtotal + tax))
}

/// Apply a partial update to a draft invoice, guarded by `ensure_editable`.
///
/// `today` is the reference day the resulting status is derived against, taken
/// as a parameter like `void_invoice`'s `voided_on`: nothing under
/// `src/invoicing/` reads the clock.
pub fn update_invoice(
    conn: &Connection,
    invoice_id: i64,
    update: &InvoiceUpdate,
    today: &str,
) -> Result<()> {
    let invoice = get_invoice(conn, invoice_id)?;
    ensure_editable(conn, &invoice)?;
    if update.is_empty() {
        return Err(NigelError::Invalid(
            "Nothing to update — provide at least one flag".to_string(),
        ));
    }
    let today = validate_date(today, "reference")?;

    if let Some(ref items) = update.items {
        validate_items(items)?;
    }
    let issue_date = match update.issue_date {
        Some(ref d) => Some(validate_date(d, "issue")?),
        None => None,
    };
    let due_date = match update.due_date {
        Some(Some(ref d)) => Some(Some(validate_date(d, "due")?)),
        Some(None) => Some(None),
        None => None,
    };
    let currency = match update.currency {
        Some(ref code) => Some(validate_currency(code)?),
        None => None,
    };

    let tx = conn.unchecked_transaction()?;

    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(ref issue_date) = issue_date {
        params.push(Box::new(issue_date.clone()));
        updates.push(format!("issue_date = ?{}", params.len()));
    }
    if let Some(ref due_date) = due_date {
        params.push(Box::new(due_date.clone()));
        updates.push(format!("due_date = ?{}", params.len()));
    }
    if let Some(ref currency) = currency {
        params.push(Box::new(currency.clone()));
        updates.push(format!("currency = ?{}", params.len()));
    }
    if let Some(ref notes) = update.notes {
        params.push(Box::new(notes.clone()));
        updates.push(format!("notes = ?{}", params.len()));
    }
    if let Some(ref terms) = update.terms {
        params.push(Box::new(terms.clone()));
        updates.push(format!("terms = ?{}", params.len()));
    }
    if !updates.is_empty() {
        params.push(Box::new(invoice_id));
        let sql = format!(
            "UPDATE invoices SET {} WHERE id = ?{}",
            updates.join(", "),
            params.len()
        );
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        tx.execute(&sql, param_refs.as_slice())?;
    }

    let mut total = invoice.total;
    if let Some(ref items) = update.items {
        let (subtotal, new_total) = replace_line_items(&tx, invoice_id, items)?;
        tx.execute(
            "UPDATE invoices SET subtotal = ?1, total = ?2 WHERE id = ?3",
            rusqlite::params![subtotal, new_total, invoice_id],
        )?;
        total = new_total;
    }

    // A Stripe link is priced in the currency and amount it was created with,
    // so an edit that moves either leaves it pointing at the wrong charge.
    let money_moved = total != invoice.total
        || currency
            .as_deref()
            .is_some_and(|c| c != invoice.currency.as_str());
    if money_moved && invoice.stripe_payment_link_id.is_some() {
        tx.execute(
            "UPDATE invoices SET stripe_payment_link_id = NULL, stripe_payment_link_url = NULL
             WHERE id = ?1",
            [invoice_id],
        )?;
    }

    tx.commit()?;
    refresh_status(conn, invoice_id, &today)?;
    Ok(())
}

/// Cancel an invoice, guarded by `ensure_voidable`. Writes `voided_at` and lets
/// `refresh_status` derive the `void` status from it.
pub fn void_invoice(conn: &Connection, invoice_id: i64, voided_on: &str) -> Result<()> {
    let invoice = get_invoice(conn, invoice_id)?;
    ensure_voidable(conn, &invoice)?;
    let voided_on = validate_date(voided_on, "void")?;
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE invoices SET voided_at = ?1 WHERE id = ?2",
        rusqlite::params![voided_on, invoice_id],
    )?;
    refresh_status(&tx, invoice_id, &voided_on)?;
    tx.commit()?;
    Ok(())
}

/// May this invoice be edited? Draft, not void, and with no recorded payments —
/// a payment against it means the client has settled against these figures.
pub fn ensure_editable(conn: &Connection, invoice: &Invoice) -> Result<()> {
    if invoice.voided_at.is_some() {
        return Err(NigelError::Conflict {
            code: "void",
            message: format!("Invoice #{} is void and cannot be edited.", invoice.number),
        });
    }
    let not_draft = invoice.status != InvoiceStatus::Draft.as_str();
    // Only a published invoice has "already been sent". `invoice pay` can drive
    // an unsent draft to `paid`, and that invoice is refused for its payments.
    if not_draft && invoice.published_at.is_some() {
        return Err(NigelError::Conflict {
            code: "not_draft",
            message: format!(
                "Invoice #{} has already been sent and cannot be edited. Void it and issue a new one.",
                invoice.number
            ),
        });
    }
    let paid = paid_amount(conn, invoice.id)?;
    if paid > 0.0 {
        return Err(NigelError::Conflict {
            code: "has_payments",
            message: format!(
                "Invoice #{} has {paid:.2} in recorded payments and cannot be edited.",
                invoice.number
            ),
        });
    }
    if not_draft {
        return Err(NigelError::Conflict {
            code: "not_draft",
            message: format!(
                "Invoice #{} is {} and cannot be edited.",
                invoice.number, invoice.status
            ),
        });
    }
    Ok(())
}

/// `voided_at` is the fact; `status` is derived from it. Reading the timestamp
/// first means a void whose status write did not land still reads as void.
pub fn is_void(invoice: &Invoice) -> bool {
    invoice.voided_at.is_some() || invoice.status == InvoiceStatus::Void.as_str()
}

/// Void is terminal: it blocks send, pay and edit. The guard lives here rather
/// than in the CLI wrapper so a caller that reaches `send_invoice` or
/// `record_payment` directly cannot get past it.
pub fn ensure_not_void(invoice: &Invoice, action: &str) -> Result<()> {
    if is_void(invoice) {
        return Err(NigelError::Conflict {
            code: "void",
            message: format!(
                "Invoice #{} is void and cannot be {action}.",
                invoice.number
            ),
        });
    }
    Ok(())
}

/// Resolve the amount to record against an invoice: the explicit request, or
/// the whole outstanding balance. Rejects amounts that would write a junk
/// payment row.
///
/// It lives here rather than in `cli/invoice.rs` so both front ends refuse the
/// same amounts with the same words, and so the refusals are typed: "already
/// settled" is a conflict, not an internal error.
pub fn payment_amount(invoice: &Invoice, paid: f64, requested: Option<f64>) -> Result<f64> {
    match requested {
        // Negated positive test, not `amount <= 0.0`: NaN compares false against
        // every bound, and a NaN payment row poisons every later SUM.
        Some(amount) if !(amount.is_finite() && amount > 0.0) => Err(NigelError::Invalid(format!(
            "--amount must be a finite number greater than zero, got {amount:.2}."
        ))),
        Some(amount) => Ok(amount),
        None => {
            // The same question `refresh_status` asks: anything settled is paid
            // in full, not a balance worth recording.
            if is_settled(invoice.total, paid) {
                return Err(NigelError::Conflict {
                    code: "no_balance",
                    message: format!(
                        "Invoice #{} has no outstanding balance (total {:.2}, paid {:.2}). Pass --amount to record a payment anyway.",
                        invoice.number, invoice.total, paid
                    ),
                });
            }
            Ok(invoice.total - paid)
        }
    }
}

/// Why this invoice cannot be deleted, or `None` when it can.
///
/// Delete is for the draft that should never have existed — the wrong client,
/// a duplicated command, a test row on real books. Void is for everything else,
/// and the line between them is whether anybody outside this machine has seen
/// the invoice. Published means its token URL and its emailed PDF are already
/// in somebody's hands; void means it is a record that something happened; a
/// payment means money arrived against it. Each of those is a tombstone worth
/// keeping, and all three refuse in one sentence.
///
/// The shape is `clients::delete_blocker`'s so both are asked the same way: the
/// CLI and the TUI put the question before the confirmation, and the API turns
/// the same block into a 409 with a machine-readable reason.
pub fn delete_blocker(conn: &Connection, invoice: &Invoice) -> Result<Option<DeleteBlock>> {
    let blocked = is_void(invoice)
        || invoice.published_at.is_some()
        || invoice.status != InvoiceStatus::Draft.as_str()
        || has_payments(conn, invoice.id)?;
    Ok(blocked.then(|| DeleteBlock::not_deletable("invoice")))
}

/// Whether any payment row names this invoice — **existence, not a sum**.
///
/// `paid_amount` answers a different question and would answer it differently
/// here: a row of exactly `0.00`, or two rows that cancel out, total nothing
/// while still being payments somebody recorded. A summed guard would call such
/// an invoice deletable in every pre-flight and then have the write refuse it,
/// which is a screen offering an action the server rejects. One predicate,
/// asked by [`delete_blocker`] and by [`delete_invoice`] inside its own
/// transaction, is what keeps the two from disagreeing.
fn has_payments(conn: &Connection, invoice_id: i64) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM invoice_payments WHERE invoice_id = ?1)",
        [invoice_id],
        |r| r.get(0),
    )?)
}

/// Remove a draft entered by mistake, with its line items, in one transaction.
///
/// The guard runs inside that transaction rather than above it, so a delete
/// races nothing: `delete_blocker` reads the row this statement is about to
/// remove. **The invoice number is not reused** — `next_invoice_number` is
/// deliberately left where it is, because a gap in a numbering sequence is
/// normal and auditable where reissuing a number that may already have been
/// exported or quoted is not.
///
/// Payments are asserted rather than cascaded, against the same `has_payments`
/// predicate the guard just applied — so this is a real assertion about the
/// rows this statement is about to orphan, not a second, looser rule that could
/// refuse what the pre-flight allowed.
pub fn delete_invoice(conn: &Connection, invoice_id: i64) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let invoice = get_invoice(&tx, invoice_id)?;
    if let Some(block) = delete_blocker(&tx, &invoice)? {
        return Err(NigelError::Blocked(block));
    }
    debug_assert!(
        !has_payments(&tx, invoice_id)?,
        "delete_blocker allowed an invoice with payment rows"
    );
    tx.execute(
        "DELETE FROM invoice_line_items WHERE invoice_id = ?1",
        [invoice_id],
    )?;
    tx.execute("DELETE FROM invoices WHERE id = ?1", [invoice_id])?;
    tx.commit()?;
    Ok(())
}

/// May this invoice be voided? Not already void, and with no recorded payments.
pub fn ensure_voidable(conn: &Connection, invoice: &Invoice) -> Result<()> {
    if invoice.voided_at.is_some() {
        return Err(NigelError::Conflict {
            code: "already_void",
            message: format!("Invoice #{} is already void.", invoice.number),
        });
    }
    let paid = paid_amount(conn, invoice.id)?;
    if paid > 0.0 {
        return Err(NigelError::Conflict {
            code: "has_payments",
            message: format!(
                "Invoice #{} has {paid:.2} in recorded payments and cannot be voided.",
                invoice.number
            ),
        });
    }
    Ok(())
}

pub fn set_payment_link(conn: &Connection, id: i64, link_id: &str, url: &str) -> Result<()> {
    conn.execute(
        "UPDATE invoices SET stripe_payment_link_id = ?1, stripe_payment_link_url = ?2 WHERE id = ?3",
        rusqlite::params![link_id, url, id],
    )?;
    Ok(())
}

pub fn mark_published(conn: &Connection, id: i64, published_at: &str) -> Result<()> {
    let published_at = validate_date(published_at, "published")?;
    conn.execute(
        "UPDATE invoices SET published_at = ?1 WHERE id = ?2",
        rusqlite::params![published_at, id],
    )?;
    refresh_status(conn, id, &published_at)?;
    Ok(())
}

pub fn line_items(conn: &Connection, invoice_id: i64) -> Result<Vec<InvoiceLineItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, invoice_id, description, quantity, unit_amount, line_total, position
         FROM invoice_line_items WHERE invoice_id = ?1 ORDER BY position",
    )?;
    let rows = stmt
        .query_map([invoice_id], |r| {
            Ok(InvoiceLineItem {
                id: r.get(0)?,
                invoice_id: r.get(1)?,
                description: r.get(2)?,
                quantity: r.get(3)?,
                unit_amount: r.get(4)?,
                line_total: r.get(5)?,
                position: r.get(6)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// One row of the invoice list: everything a list screen prints, including the
/// balance, without a second query per invoice.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceListRow {
    pub id: i64,
    pub number: i64,
    pub status: String,
    pub client_id: i64,
    /// `None` when the client row is gone. The join is a LEFT JOIN, so an
    /// orphaned invoice appears with a dash rather than vanishing from the list.
    pub client_name: Option<String>,
    pub issue_date: String,
    pub due_date: Option<String>,
    pub currency: String,
    pub total: f64,
    pub paid: f64,
    pub balance: f64,
}

/// The `status` filter vocabulary: the six status words plus `open`, the
/// sent/partial/overdue set `sync` and the aging report already work in.
pub const OPEN_STATUSES: [&str; 3] = ["sent", "partial", "overdue"];
const STATUS_WORDS: [&str; 6] = ["draft", "sent", "partial", "paid", "overdue", "void"];

/// The `invoice_payments.method` CHECK set, checked before the insert so an
/// unknown method is a named refusal rather than a constraint violation.
pub const PAYMENT_METHODS: [&str; 4] = ["stripe", "ach", "direct_deposit", "other"];

pub fn validate_payment_method(method: &str) -> Result<()> {
    if PAYMENT_METHODS.contains(&method) {
        return Ok(());
    }
    Err(NigelError::Invalid(format!(
        "Invalid payment method: {method} (expected one of {})",
        PAYMENT_METHODS.join(", ")
    )))
}

/// Expand a `status` filter to the statuses it selects.
fn statuses_for(filter: &str) -> Result<Vec<&'static str>> {
    if filter == "open" {
        return Ok(OPEN_STATUSES.to_vec());
    }
    STATUS_WORDS
        .iter()
        .find(|word| **word == filter)
        .map(|word| vec![*word])
        .ok_or_else(|| {
            NigelError::Invalid(format!(
                "Invalid status: {filter} (expected one of {}, open)",
                STATUS_WORDS.join(", ")
            ))
        })
}

/// Every invoice, newest number first, with its client and paid-to-date.
///
/// `status` takes a status word or `open`; `client_id` narrows to one client.
/// Paid amounts come from one `GROUP BY` aggregate rather than a `SELECT SUM`
/// per row, which is what keeps a screen that redraws off an N+1.
pub fn list_invoices(
    conn: &Connection,
    status: Option<&str>,
    client_id: Option<i64>,
) -> Result<Vec<InvoiceListRow>> {
    let statuses = match status {
        Some(filter) => Some(statuses_for(filter)?),
        None => None,
    };

    let mut wheres = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(ref statuses) = statuses {
        let slots: Vec<String> = statuses
            .iter()
            .map(|status| {
                params.push(Box::new(*status));
                format!("?{}", params.len())
            })
            .collect();
        wheres.push(format!("i.status IN ({})", slots.join(", ")));
    }
    if let Some(id) = client_id {
        params.push(Box::new(id));
        wheres.push(format!("i.client_id = ?{}", params.len()));
    }
    let filter = if wheres.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", wheres.join(" AND "))
    };

    let sql = format!(
        "SELECT i.id, i.number, i.status, i.client_id, c.name, i.issue_date, i.due_date,
                i.currency, i.total, COALESCE(p.paid, 0)
         FROM invoices i
         LEFT JOIN clients c ON c.id = i.client_id
         LEFT JOIN (SELECT invoice_id, SUM(amount) AS paid FROM invoice_payments
                    GROUP BY invoice_id) p ON p.invoice_id = i.id
         {filter}
         ORDER BY i.number DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt
        .query_map(param_refs.as_slice(), |r| {
            let total: f64 = r.get(8)?;
            let paid: f64 = r.get(9)?;
            Ok(InvoiceListRow {
                id: r.get(0)?,
                number: r.get(1)?,
                status: r.get(2)?,
                client_id: r.get(3)?,
                client_name: r.get(4)?,
                issue_date: r.get(5)?,
                due_date: r.get(6)?,
                currency: r.get(7)?,
                total,
                paid,
                balance: total - paid,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// An invoice's payment history, oldest first.
pub fn payments(conn: &Connection, invoice_id: i64) -> Result<Vec<InvoicePayment>> {
    let mut stmt = conn.prepare(
        "SELECT id, invoice_id, amount, paid_date, method, stripe_checkout_session_id
         FROM invoice_payments WHERE invoice_id = ?1 ORDER BY paid_date, id",
    )?;
    let rows = stmt
        .query_map([invoice_id], |r| {
            Ok(InvoicePayment {
                id: r.get(0)?,
                invoice_id: r.get(1)?,
                amount: r.get(2)?,
                paid_date: r.get(3)?,
                method: r.get(4)?,
                stripe_checkout_session_id: r.get(5)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgingBucket {
    pub label: &'static str,
    pub count: usize,
    pub total: f64,
}

/// One open invoice, with the balance and the bucket it was counted in.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgingInvoice {
    pub number: i64,
    pub client: String,
    /// The date the bucket aged from: the due date, or the issue date when there is none.
    pub due_date: String,
    pub days_past_due: i64,
    pub bucket: &'static str,
    pub total: f64,
    pub paid: f64,
    pub balance: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgingReport {
    pub as_of: String,
    pub buckets: Vec<AgingBucket>,
    pub invoices: Vec<AgingInvoice>,
    pub outstanding: f64,
}

const AGING_LABELS: [&str; 5] = ["current", "1-30", "31-60", "61-90", "90+"];

pub fn ar_aging_detail(conn: &Connection, today: &str) -> Result<AgingReport> {
    let today = parse_date(today, "as-of")?;
    let as_of = today.format("%Y-%m-%d").to_string();

    let mut buckets: Vec<AgingBucket> = AGING_LABELS
        .iter()
        .map(|label| AgingBucket {
            label,
            count: 0,
            total: 0.0,
        })
        .collect();

    let mut stmt = conn.prepare(
        "SELECT i.id, i.number, c.name, i.total, COALESCE(i.due_date, i.issue_date)
         FROM invoices i JOIN clients c ON c.id = i.client_id
         WHERE i.status IN ('sent','partial','overdue')",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, f64>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut invoices = Vec::new();
    let mut outstanding = 0.0;

    for (id, number, client, total, due) in rows {
        let paid = paid_amount(conn, id)?;
        let owing = total - paid;
        if owing <= 0.0 {
            continue;
        }
        let due_date = NaiveDate::parse_from_str(&due, "%Y-%m-%d").unwrap_or(today);
        let days = (today - due_date).num_days();
        let idx = if days <= 0 {
            0
        } else if days <= 30 {
            1
        } else if days <= 60 {
            2
        } else if days <= 90 {
            3
        } else {
            4
        };
        buckets[idx].count += 1;
        buckets[idx].total += owing;
        outstanding += owing;
        invoices.push(AgingInvoice {
            number,
            client,
            due_date: due,
            days_past_due: days,
            bucket: AGING_LABELS[idx],
            total,
            paid,
            balance: owing,
        });
    }

    invoices.sort_by_key(|i| std::cmp::Reverse(i.days_past_due));

    Ok(AgingReport {
        as_of,
        buckets,
        invoices,
        outstanding,
    })
}

pub fn ar_aging(conn: &Connection, today: &str) -> Result<Vec<AgingBucket>> {
    Ok(ar_aging_detail(conn, today)?.buckets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::invoicing::clients::add_client;
    use crate::migrations::run_migrations;

    fn test_conn() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    #[test]
    fn a_line_item_reads_and_writes_camel_case_json() {
        let item: NewLineItem =
            serde_json::from_str(r#"{"description":"Design","quantity":2,"unitAmount":100}"#)
                .unwrap();
        assert_eq!(item.description, "Design");
        assert_eq!(item.unit_amount, 100.0);
        assert_eq!(
            serde_json::to_value(&item).unwrap()["unitAmount"],
            serde_json::json!(100.0)
        );
    }

    #[test]
    fn token_is_16_alphanumeric() {
        let t = gen_token();
        assert_eq!(t.len(), 16);
        assert!(t.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn first_number_defaults_to_1248_and_increments() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![
            NewLineItem {
                description: "Design".into(),
                quantity: 2.0,
                unit_amount: 100.0,
            },
            NewLineItem {
                description: "Dev".into(),
                quantity: 1.0,
                unit_amount: 50.0,
            },
        ];
        let id1 = create_invoice(
            &conn,
            cid,
            "2026-08-04",
            Some("2026-09-03"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        let inv1 = get_invoice(&conn, id1).unwrap();
        assert_eq!(inv1.number, 1248);
        assert_eq!(inv1.subtotal, 250.0);
        assert_eq!(inv1.total, 250.0);
        assert_eq!(inv1.status, "draft");

        let id2 =
            create_invoice(&conn, cid, "2026-08-05", None, "USD", &items, None, None).unwrap();
        assert_eq!(get_invoice(&conn, id2).unwrap().number, 1249);
    }

    #[test]
    fn creating_an_invoice_for_an_unknown_client_is_not_found_and_writes_nothing() {
        let (_d, conn) = test_conn();
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 10.0,
        }];

        let err = create_invoice(&conn, 99, "2026-08-04", None, "USD", &items, None, None)
            .map(|_| ())
            .unwrap_err();
        assert!(
            matches!(err, crate::error::NigelError::NotFound(_)),
            "got: {err:?}"
        );
        assert!(err.to_string().contains("id 99"), "got: {err}");

        let invoices: i64 = conn
            .query_row("SELECT COUNT(*) FROM invoices", [], |r| r.get(0))
            .unwrap();
        assert_eq!(invoices, 0);
        assert_eq!(next_number(&conn).unwrap(), 1248);
    }

    #[test]
    fn failed_create_rolls_back_and_leaves_numbering_usable() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Design".into(),
            quantity: 1.0,
            unit_amount: 10.0,
        }];

        conn.execute_batch(
            "CREATE TRIGGER fail_line_items BEFORE INSERT ON invoice_line_items
             BEGIN SELECT RAISE(ABORT, 'line item insert failed'); END;",
        )
        .unwrap();
        assert!(create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).is_err());

        let invoices: i64 = conn
            .query_row("SELECT COUNT(*) FROM invoices", [], |r| r.get(0))
            .unwrap();
        assert_eq!(invoices, 0);
        assert_eq!(next_number(&conn).unwrap(), 1248);

        conn.execute_batch("DROP TRIGGER fail_line_items;").unwrap();
        let id = create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap();
        assert_eq!(get_invoice(&conn, id).unwrap().number, 1248);
    }

    #[test]
    fn recording_full_payment_marks_paid() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 200.0,
        }];
        let id = create_invoice(
            &conn,
            cid,
            "2026-08-04",
            Some("2026-09-03"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();

        assert!(record_payment(&conn, id, 200.0, "2026-08-10", "direct_deposit", None).unwrap());
        assert_eq!(paid_amount(&conn, id).unwrap(), 200.0);
        assert_eq!(refresh_status(&conn, id, "2026-08-11").unwrap(), "paid");
    }

    #[test]
    fn partial_then_overdue_is_derived() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 200.0,
        }];
        let id = create_invoice(
            &conn,
            cid,
            "2026-08-04",
            Some("2026-08-20"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE invoices SET status='sent', published_at='2026-08-04' WHERE id=?1",
            [id],
        )
        .unwrap();

        record_payment(&conn, id, 50.0, "2026-08-10", "ach", None).unwrap();
        assert_eq!(refresh_status(&conn, id, "2026-08-15").unwrap(), "partial");
        assert_eq!(refresh_status(&conn, id, "2026-08-25").unwrap(), "overdue");
    }

    #[test]
    fn stripe_session_is_idempotent() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        let id = create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap();

        assert!(record_payment(&conn, id, 100.0, "2026-08-10", "stripe", Some("cs_1")).unwrap());
        assert!(!record_payment(&conn, id, 100.0, "2026-08-10", "stripe", Some("cs_1")).unwrap());
        assert_eq!(paid_amount(&conn, id).unwrap(), 100.0);
    }

    #[test]
    fn installments_summing_a_hair_short_still_mark_paid() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 100.20,
        }];
        let id = create_invoice(
            &conn,
            cid,
            "2026-08-04",
            Some("2026-08-20"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE invoices SET status='sent', published_at='2026-08-04' WHERE id=?1",
            [id],
        )
        .unwrap();

        // 33.40 * 3 lands on 100.19999999999999 in binary floating point, a hair under
        // the 100.20 total. The invoice is settled in full and must read as paid.
        for _ in 0..3 {
            record_payment(&conn, id, 33.40, "2026-08-10", "ach", None).unwrap();
        }
        assert!(paid_amount(&conn, id).unwrap() < 100.20);
        assert_eq!(refresh_status(&conn, id, "2026-08-25").unwrap(), "paid");
    }

    #[test]
    fn a_cent_short_is_not_paid() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 100.00,
        }];
        let id = create_invoice(
            &conn,
            cid,
            "2026-08-04",
            Some("2026-08-20"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE invoices SET status='sent', published_at='2026-08-04' WHERE id=?1",
            [id],
        )
        .unwrap();

        record_payment(&conn, id, 99.99, "2026-08-10", "ach", None).unwrap();
        assert_eq!(refresh_status(&conn, id, "2026-08-15").unwrap(), "partial");
    }

    #[test]
    fn void_is_derived_from_voided_at() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        let id = create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap();
        conn.execute(
            "UPDATE invoices SET voided_at = '2026-08-06' WHERE id = ?1",
            [id],
        )
        .unwrap();

        assert_eq!(refresh_status(&conn, id, "2026-08-25").unwrap(), "void");
        insert_payment(&conn, id, 100.0);
        assert_eq!(refresh_status(&conn, id, "2026-08-25").unwrap(), "void");
    }

    #[test]
    fn void_is_never_downgraded() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        let id = create_invoice(
            &conn,
            cid,
            "2026-08-04",
            Some("2026-08-20"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE invoices SET voided_at='2026-08-06' WHERE id=?1",
            [id],
        )
        .unwrap();

        assert_eq!(refresh_status(&conn, id, "2026-08-25").unwrap(), "void");
        insert_payment(&conn, id, 100.0);
        assert_eq!(refresh_status(&conn, id, "2026-08-25").unwrap(), "void");
        assert_eq!(get_invoice(&conn, id).unwrap().status, "void");
    }

    /// A payment row written past `record_payment`, which refuses a void
    /// invoice. These two tests are about what `refresh_status` derives from a
    /// payment total, so the row has to arrive some other way.
    fn insert_payment(conn: &Connection, invoice_id: i64, amount: f64) {
        conn.execute(
            "INSERT INTO invoice_payments (invoice_id, amount, paid_date, method)
             VALUES (?1, ?2, '2026-08-10', 'other')",
            rusqlite::params![invoice_id, amount],
        )
        .unwrap();
    }

    /// The client with this name, created on first use. Client names are unique
    /// now, and these helpers are called once per invoice.
    fn client_id(conn: &Connection, name: &str) -> i64 {
        match add_client(conn, name, None, None, None) {
            Ok(id) => id,
            Err(NigelError::DuplicateName { .. }) => conn
                .query_row("SELECT id FROM clients WHERE name = ?1", [name], |r| {
                    r.get(0)
                })
                .unwrap(),
            Err(e) => panic!("add_client({name}): {e}"),
        }
    }

    /// One 100.00 draft invoice (number 1248) and its row id.
    fn seed_draft(conn: &Connection) -> i64 {
        let cid = client_id(conn, "Acme");
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        create_invoice(conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap()
    }

    fn block_code(err: &crate::error::NigelError) -> &'static str {
        match err {
            crate::error::NigelError::Blocked(block) => block.reason_code(),
            other => panic!("expected a Blocked, got: {other:?}"),
        }
    }

    #[test]
    fn a_draft_that_was_never_published_and_has_no_payments_can_be_deleted() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);

        assert!(delete_blocker(&conn, &get_invoice(&conn, id).unwrap())
            .unwrap()
            .is_none());
        delete_invoice(&conn, id).unwrap();

        assert!(list_invoices(&conn, None, None).unwrap().is_empty());
        assert!(matches!(
            get_invoice(&conn, id),
            Err(NigelError::NotFound(_))
        ));
    }

    #[test]
    fn deleting_an_invoice_takes_its_line_items_with_it() {
        let (_d, conn) = test_conn();
        let cid = client_id(&conn, "Acme");
        let items = vec![
            NewLineItem {
                description: "Design".into(),
                quantity: 2.0,
                unit_amount: 100.0,
            },
            NewLineItem {
                description: "Build".into(),
                quantity: 1.0,
                unit_amount: 400.0,
            },
        ];
        let id = create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap();
        assert_eq!(line_items(&conn, id).unwrap().len(), 2);

        delete_invoice(&conn, id).unwrap();

        let orphans: i64 = conn
            .query_row("SELECT COUNT(*) FROM invoice_line_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(orphans, 0, "the line items went with the invoice");
    }

    /// The counter only moves forward, and this pins the decision: a gap in the
    /// numbering is normal and auditable, where reissuing a number that may
    /// already have been exported or quoted is not.
    #[test]
    fn deleting_the_newest_draft_does_not_move_the_invoice_number_counter() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        let after_create = next_number(&conn).unwrap();
        assert_eq!(get_invoice(&conn, id).unwrap().number, after_create - 1);

        delete_invoice(&conn, id).unwrap();

        assert_eq!(
            next_number(&conn).unwrap(),
            after_create,
            "the deleted number is not handed out again"
        );
    }

    #[test]
    fn a_published_invoice_refuses_deletion() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        mark_published(&conn, id, "2026-08-05").unwrap();

        let err = delete_invoice(&conn, id).unwrap_err();
        assert_eq!(block_code(&err), "not_deletable");
        assert!(get_invoice(&conn, id).is_ok(), "nothing was removed");
    }

    #[test]
    fn a_paid_invoice_refuses_deletion() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        record_payment(&conn, id, 100.0, "2026-08-06", "direct_deposit", None).unwrap();

        let err = delete_invoice(&conn, id).unwrap_err();
        assert_eq!(block_code(&err), "not_deletable");
    }

    /// A payment against an unpublished draft leaves it `draft` while it is only
    /// partial, so status alone would let this one through — the guard asks
    /// about the money as well.
    #[test]
    fn a_draft_with_a_partial_payment_refuses_deletion() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        record_payment(&conn, id, 40.0, "2026-08-06", "direct_deposit", None).unwrap();
        assert_eq!(get_invoice(&conn, id).unwrap().status, "draft");

        let err = delete_invoice(&conn, id).unwrap_err();
        assert_eq!(block_code(&err), "not_deletable");
    }

    /// The tombstone argument: a void invoice is a record that something
    /// happened, and deleting it would erase the record rather than the mistake.
    #[test]
    fn a_void_invoice_refuses_deletion() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        void_invoice(&conn, id, "2026-08-06").unwrap();

        let err = delete_invoice(&conn, id).unwrap_err();
        assert_eq!(block_code(&err), "not_deletable");
        assert_eq!(get_invoice(&conn, id).unwrap().status, "void");
    }

    /// The pre-flight and the delete ask the same question, so a screen can
    /// never offer an action the write then refuses.
    ///
    /// A summed guard would answer differently here: a payment row of exactly
    /// zero — or of two rows that cancel out — totals nothing, so `> 0.0` would
    /// call this invoice deletable and the delete would refuse it. The rule is
    /// therefore row existence, in both.
    #[test]
    fn a_zero_amount_payment_row_stops_the_delete_and_the_pre_flight_agrees() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        conn.execute(
            "INSERT INTO invoice_payments (invoice_id, amount, paid_date, method)
             VALUES (?1, 0.0, '2026-08-06', 'other')",
            [id],
        )
        .unwrap();

        let invoice = get_invoice(&conn, id).unwrap();
        assert_eq!(
            delete_blocker(&conn, &invoice)
                .unwrap()
                .map(|b| b.reason_code()),
            Some("not_deletable"),
            "the pre-flight must refuse what the delete refuses"
        );
        let err = delete_invoice(&conn, id).unwrap_err();
        assert_eq!(block_code(&err), "not_deletable");
        assert!(get_invoice(&conn, id).is_ok());
    }

    /// Two payments that sum to nothing are still payments. The same disagreement
    /// as the zero row, arrived at from the other side.
    #[test]
    fn payments_that_cancel_out_stop_the_delete_and_the_pre_flight_agrees() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        for amount in ["40.0", "-40.0"] {
            conn.execute(
                &format!(
                    "INSERT INTO invoice_payments (invoice_id, amount, paid_date, method)
                     VALUES (?1, {amount}, '2026-08-06', 'other')"
                ),
                [id],
            )
            .unwrap();
        }
        assert_eq!(paid_amount(&conn, id).unwrap(), 0.0);

        let invoice = get_invoice(&conn, id).unwrap();
        assert!(delete_blocker(&conn, &invoice).unwrap().is_some());
        assert_eq!(
            block_code(&delete_invoice(&conn, id).unwrap_err()),
            "not_deletable"
        );
    }

    #[test]
    fn deleting_an_invoice_that_is_not_there_is_a_not_found() {
        let (_d, conn) = test_conn();
        assert!(matches!(
            delete_invoice(&conn, 404),
            Err(NigelError::NotFound(_))
        ));
    }

    /// One sentence, wherever the refusal is read.
    #[test]
    fn every_refused_delete_reads_the_same_way() {
        let (_d, conn) = test_conn();
        let expected = "Cannot delete: invoice has been sent, paid or voided — only an unsent draft with no payments can be deleted";

        for prepare in [
            (|conn: &Connection, id: i64| mark_published(conn, id, "2026-08-05").unwrap())
                as fn(&Connection, i64),
            |conn, id| void_invoice(conn, id, "2026-08-05").unwrap(),
            |conn, id| {
                record_payment(conn, id, 100.0, "2026-08-05", "direct_deposit", None).unwrap();
            },
        ] {
            let id = seed_draft(&conn);
            prepare(&conn, id);
            assert_eq!(delete_invoice(&conn, id).unwrap_err().to_string(), expected);
        }
    }

    #[test]
    fn a_new_invoice_for_an_archived_client_is_refused_naming_the_reason() {
        use crate::invoicing::clients::archive_client;

        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme Co", None, None, None).unwrap();
        archive_client(&conn, cid, "2026-08-11").unwrap();

        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        let err = create_invoice(&conn, cid, "2026-08-12", None, "USD", &items, None, None)
            .map(|_| ())
            .unwrap_err();
        assert_eq!(conflict_code(&err), "client_archived");
        assert!(err.to_string().contains("Acme Co"), "got: {err}");
        assert_eq!(
            list_invoices(&conn, None, None).unwrap().len(),
            0,
            "nothing was written"
        );
    }

    #[test]
    fn unarchiving_makes_the_client_invoiceable_again() {
        use crate::invoicing::clients::{archive_client, unarchive_client};

        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme Co", None, None, None).unwrap();
        archive_client(&conn, cid, "2026-08-11").unwrap();
        unarchive_client(&conn, cid).unwrap();

        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        create_invoice(&conn, cid, "2026-08-12", None, "USD", &items, None, None).unwrap();
        assert_eq!(list_invoices(&conn, None, None).unwrap().len(), 1);
    }

    /// AC #3: archive changes no query that lists or ages an invoice.
    #[test]
    fn an_archived_clients_existing_invoices_stay_in_every_list() {
        use crate::invoicing::clients::archive_client;

        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme Co", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        for issued in ["2026-06-01", "2026-07-01"] {
            let id = create_invoice(
                &conn,
                cid,
                issued,
                Some("2026-07-15"),
                "USD",
                &items,
                None,
                None,
            )
            .unwrap();
            mark_published(&conn, id, issued).unwrap();
            refresh_status(&conn, id, "2026-08-11").unwrap();
        }
        archive_client(&conn, cid, "2026-08-11").unwrap();

        let rows = list_invoices(&conn, None, None).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows
            .iter()
            .all(|r| r.client_name.as_deref() == Some("Acme Co")));

        let aging = ar_aging_detail(&conn, "2026-08-11").unwrap();
        assert!(
            aging.invoices.iter().any(|i| i.client == "Acme Co"),
            "the archived client is still on the aging report"
        );
    }

    fn conflict_code(err: &crate::error::NigelError) -> &'static str {
        match err {
            crate::error::NigelError::Conflict { code, .. } => code,
            other => panic!("expected a Conflict, got: {other:?}"),
        }
    }

    #[test]
    fn default_payment_is_the_outstanding_balance() {
        let (_d, conn) = test_conn();
        let invoice = get_invoice(&conn, seed_draft(&conn)).unwrap();

        assert_eq!(payment_amount(&invoice, 0.0, None).unwrap(), 100.0);
        assert_eq!(payment_amount(&invoice, 40.0, None).unwrap(), 60.0);
    }

    #[test]
    fn no_outstanding_balance_is_a_conflict_not_an_internal_error() {
        let (_d, conn) = test_conn();
        let invoice = get_invoice(&conn, seed_draft(&conn)).unwrap();

        for paid in [100.0, 100.001, 150.0] {
            let err = payment_amount(&invoice, paid, None).unwrap_err();
            assert_eq!(conflict_code(&err), "no_balance");
            // The CLI's sentence is unchanged, verbatim.
            assert!(
                err.to_string().contains("no outstanding balance"),
                "got: {err}"
            );
        }
    }

    #[test]
    fn a_nan_or_negative_amount_is_invalid_not_a_junk_payment_row() {
        let (_d, conn) = test_conn();
        let invoice = get_invoice(&conn, seed_draft(&conn)).unwrap();

        for amount in [0.0, -25.0] {
            let err = payment_amount(&invoice, 0.0, Some(amount)).unwrap_err();
            assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
            assert!(err.to_string().contains("greater than zero"), "got: {err}");
        }
        for amount in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let err = payment_amount(&invoice, 0.0, Some(amount)).unwrap_err();
            assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
            assert!(err.to_string().contains("finite number"), "got: {err}");
        }
        // An overpayment is a real thing a bank does; only zero and negative are junk.
        assert_eq!(payment_amount(&invoice, 0.0, Some(250.0)).unwrap(), 250.0);
    }

    #[test]
    fn record_payment_refuses_a_void_invoice_without_the_cli_wrapper() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        void_invoice(&conn, id, "2026-08-06").unwrap();

        let err = record_payment(&conn, id, 10.0, "2026-08-07", "ach", None).unwrap_err();
        assert_eq!(conflict_code(&err), "void");
        assert!(payments(&conn, id).unwrap().is_empty(), "a row was written");
    }

    #[test]
    fn a_stale_void_status_also_refuses_a_payment() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        // A void whose status write did not land: the timestamp is the fact.
        conn.execute(
            "UPDATE invoices SET voided_at='2026-08-06', status='draft' WHERE id=?1",
            [id],
        )
        .unwrap();

        let err = record_payment(&conn, id, 10.0, "2026-08-07", "ach", None).unwrap_err();
        assert_eq!(conflict_code(&err), "void");
    }

    #[test]
    fn update_invoice_refuses_a_published_invoice_without_the_cli_wrapper() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        mark_published(&conn, id, "2026-08-05").unwrap();

        let update = InvoiceUpdate {
            issue_date: Some("2026-08-09".into()),
            ..InvoiceUpdate::default()
        };
        let err = update_invoice(&conn, id, &update, "2026-08-11").unwrap_err();
        assert_eq!(conflict_code(&err), "not_draft");
        assert_eq!(get_invoice(&conn, id).unwrap().issue_date, "2026-08-04");
    }

    #[test]
    fn a_clean_draft_is_editable_and_voidable() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        let invoice = get_invoice(&conn, id).unwrap();

        assert!(ensure_editable(&conn, &invoice).is_ok());
        assert!(ensure_voidable(&conn, &invoice).is_ok());
    }

    #[test]
    fn a_published_invoice_refuses_edits() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        mark_published(&conn, id, "2026-08-05").unwrap();
        let invoice = get_invoice(&conn, id).unwrap();

        let err = ensure_editable(&conn, &invoice).unwrap_err();
        assert_eq!(conflict_code(&err), "not_draft");
        assert!(
            err.to_string()
                .contains("has already been sent and cannot be edited"),
            "got: {err}"
        );
        assert!(ensure_voidable(&conn, &invoice).is_ok());
    }

    #[test]
    fn a_void_invoice_refuses_edits() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        void_at(&conn, id, "2026-08-06");
        let invoice = get_invoice(&conn, id).unwrap();

        let err = ensure_editable(&conn, &invoice).unwrap_err();
        assert_eq!(conflict_code(&err), "void");
        assert_eq!(
            err.to_string(),
            "Invoice #1248 is void and cannot be edited."
        );
    }

    #[test]
    fn a_paid_draft_is_refused_for_its_payments_not_for_being_sent() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        record_payment(&conn, id, 100.0, "2026-08-10", "ach", None).unwrap();
        let invoice = get_invoice(&conn, id).unwrap();
        assert_eq!(invoice.status, "paid");
        assert_eq!(invoice.published_at, None);

        let err = ensure_editable(&conn, &invoice).unwrap_err();
        assert_eq!(conflict_code(&err), "has_payments");
        assert!(
            !err.to_string().contains("already been sent"),
            "an unsent invoice must not be told it was sent: {err}"
        );
    }

    #[test]
    fn an_invoice_with_payments_refuses_edit_and_void() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        record_payment(&conn, id, 50.0, "2026-08-10", "ach", None).unwrap();
        let invoice = get_invoice(&conn, id).unwrap();

        let edit_err = ensure_editable(&conn, &invoice).unwrap_err();
        assert_eq!(conflict_code(&edit_err), "has_payments");
        assert_eq!(
            edit_err.to_string(),
            "Invoice #1248 has 50.00 in recorded payments and cannot be edited."
        );

        let void_err = ensure_voidable(&conn, &invoice).unwrap_err();
        assert_eq!(conflict_code(&void_err), "has_payments");
        assert_eq!(
            void_err.to_string(),
            "Invoice #1248 has 50.00 in recorded payments and cannot be voided."
        );
    }

    #[test]
    fn voiding_a_void_invoice_is_already_void() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        void_at(&conn, id, "2026-08-06");
        let invoice = get_invoice(&conn, id).unwrap();

        let err = ensure_voidable(&conn, &invoice).unwrap_err();
        assert_eq!(conflict_code(&err), "already_void");
        assert_eq!(err.to_string(), "Invoice #1248 is already void.");
    }

    #[test]
    fn editing_the_due_date_leaves_everything_else_alone() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);

        update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                due_date: Some(Some("2026-09-30".into())),
                ..Default::default()
            },
            "2026-08-11",
        )
        .unwrap();

        let inv = get_invoice(&conn, id).unwrap();
        assert_eq!(inv.due_date.as_deref(), Some("2026-09-30"));
        assert_eq!(inv.issue_date, "2026-08-04");
        assert_eq!(inv.total, 100.0);
        assert_eq!(inv.currency, "USD");
        assert_eq!(line_items(&conn, id).unwrap().len(), 1);
    }

    #[test]
    fn clearing_the_due_date_writes_null() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                due_date: Some(Some("2026-09-30".into())),
                ..Default::default()
            },
            "2026-08-11",
        )
        .unwrap();

        update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                due_date: Some(None),
                ..Default::default()
            },
            "2026-08-11",
        )
        .unwrap();

        assert_eq!(get_invoice(&conn, id).unwrap().due_date, None);
    }

    #[test]
    fn editing_notes_and_terms_persists_both() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);

        update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                notes: Some(Some("Thanks".into())),
                terms: Some(Some("Net 30".into())),
                ..Default::default()
            },
            "2026-08-11",
        )
        .unwrap();

        let inv = get_invoice(&conn, id).unwrap();
        assert_eq!(inv.notes.as_deref(), Some("Thanks"));
        assert_eq!(inv.terms.as_deref(), Some("Net 30"));
    }

    #[test]
    fn an_empty_invoice_update_is_rejected() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);

        let err = update_invoice(&conn, id, &InvoiceUpdate::default(), "2026-08-11").unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
        assert_eq!(
            err.to_string(),
            "Nothing to update — provide at least one flag"
        );
    }

    #[test]
    fn replacing_line_items_renumbers_positions_densely() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![
            NewLineItem {
                description: "A".into(),
                quantity: 1.0,
                unit_amount: 10.0,
            },
            NewLineItem {
                description: "B".into(),
                quantity: 1.0,
                unit_amount: 20.0,
            },
            NewLineItem {
                description: "C".into(),
                quantity: 1.0,
                unit_amount: 30.0,
            },
        ];
        let id = create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap();

        update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                items: Some(vec![
                    NewLineItem {
                        description: "Rework".into(),
                        quantity: 2.0,
                        unit_amount: 250.0,
                    },
                    NewLineItem {
                        description: "Extras".into(),
                        quantity: 1.0,
                        unit_amount: 50.0,
                    },
                ]),
                ..Default::default()
            },
            "2026-08-11",
        )
        .unwrap();

        let rows = line_items(&conn, id).unwrap();
        let positions: Vec<i64> = rows.iter().map(|r| r.position).collect();
        assert_eq!(positions, vec![0, 1]);
        let descriptions: Vec<&str> = rows.iter().map(|r| r.description.as_str()).collect();
        assert_eq!(descriptions, vec!["Rework", "Extras"]);
    }

    #[test]
    fn replacing_line_items_recomputes_subtotal_and_total() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);

        update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                items: Some(vec![NewLineItem {
                    description: "Rework".into(),
                    quantity: 2.0,
                    unit_amount: 250.0,
                }]),
                ..Default::default()
            },
            "2026-08-11",
        )
        .unwrap();

        let inv = get_invoice(&conn, id).unwrap();
        assert_eq!(inv.subtotal, 500.0);
        assert_eq!(inv.total, 500.0);
    }

    #[test]
    fn omitting_items_leaves_the_existing_lines_alone() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        let before = line_items(&conn, id).unwrap();

        update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                notes: Some(Some("Thanks".into())),
                ..Default::default()
            },
            "2026-08-11",
        )
        .unwrap();

        let after = line_items(&conn, id).unwrap();
        assert_eq!(after.len(), before.len());
        assert_eq!(after[0].description, before[0].description);
        assert_eq!(get_invoice(&conn, id).unwrap().total, 100.0);
    }

    #[test]
    fn a_failed_line_item_insert_leaves_the_invoice_untouched() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);

        conn.execute_batch(
            "CREATE TRIGGER fail_line_items BEFORE INSERT ON invoice_line_items
             BEGIN SELECT RAISE(ABORT, 'line item insert failed'); END;",
        )
        .unwrap();

        assert!(update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                items: Some(vec![NewLineItem {
                    description: "Rework".into(),
                    quantity: 2.0,
                    unit_amount: 250.0,
                }]),
                ..Default::default()
            },
            "2026-08-11",
        )
        .is_err());

        conn.execute_batch("DROP TRIGGER fail_line_items;").unwrap();
        let rows = line_items(&conn, id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].description, "Work");
        assert_eq!(get_invoice(&conn, id).unwrap().total, 100.0);
    }

    /// One line item, so a case reads as the figures it is varying.
    fn line(quantity: f64, unit_amount: f64) -> Vec<NewLineItem> {
        vec![NewLineItem {
            description: "Work".into(),
            quantity,
            unit_amount,
        }]
    }

    #[test]
    fn validate_items_refuses_an_empty_list_and_a_total_of_zero() {
        assert!(matches!(
            validate_items(&[]).unwrap_err(),
            NigelError::Invalid(_)
        ));
        assert!(validate_items(&line(0.0, 150.0)).is_err(), "a zero total");
        assert!(
            validate_items(&line(-1.0, 150.0)).is_err(),
            "a negative total"
        );
        assert!(validate_items(&line(2.0, 150.0)).is_ok());
    }

    /// A NaN or infinite figure poisons every later SUM over the column, and
    /// `--item`'s `f64::from_str` accepts both words.
    #[test]
    fn validate_items_refuses_a_non_finite_figure() {
        for items in [
            line(f64::NAN, 10.0),
            line(1.0, f64::NAN),
            line(f64::INFINITY, 10.0),
            line(1.0, f64::NEG_INFINITY),
        ] {
            let err = validate_items(&items).unwrap_err();
            assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
        }
    }

    /// Two finite factors do not make a finite product: the check has to run
    /// after the multiply, not on the figures that went into it.
    #[test]
    fn validate_items_refuses_an_overflowing_product() {
        let err = validate_items(&line(1e308, 1e308)).unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
        assert!(
            err.to_string().contains("finite"),
            "the sentence names the problem: {err}"
        );
        // The figure that would otherwise have been written.
        assert!(!(1e308_f64 * 1e308_f64).is_finite());
    }

    /// And two finite line totals do not make a finite sum, which is a separate
    /// check: each line here multiplies out to exactly `1e308`.
    #[test]
    fn validate_items_refuses_a_sum_that_overflows_finite_lines() {
        let big = || NewLineItem {
            description: "Big".into(),
            quantity: 1e154,
            unit_amount: 1e154,
        };
        assert!(
            (big().quantity * big().unit_amount).is_finite(),
            "each line total is finite on its own"
        );

        let err = validate_items(&[big(), big()]).unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
        assert!(
            err.to_string().contains("finite total"),
            "the sum is what failed, not a line: {err}"
        );
    }

    #[test]
    fn a_non_finite_line_item_is_refused_on_create_and_on_edit() {
        let (_d, conn) = test_conn();
        let cid = client_id(&conn, "Acme");

        let err = create_invoice(
            &conn,
            cid,
            "2026-06-01",
            None,
            "USD",
            &line(f64::NAN, 100.0),
            None,
            None,
        )
        .map(|_| ())
        .unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");

        let id = seed_draft(&conn);
        let err = update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                items: Some(line(1e308, 1e308)),
                ..Default::default()
            },
            "2026-08-11",
        )
        .unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
        // The refusal came before the write: the draft still totals 100.
        assert_eq!(get_invoice(&conn, id).unwrap().total, 100.0);
    }

    #[test]
    fn a_malformed_date_is_rejected_on_create_and_on_edit() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];

        let err = create_invoice(&conn, cid, "2026-13-45", None, "USD", &items, None, None)
            .map(|_| ())
            .unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
        assert_eq!(
            err.to_string(),
            "Invalid issue date: 2026-13-45 (expected YYYY-MM-DD)"
        );

        let id = seed_draft(&conn);
        let err = update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                issue_date: Some("2026-13-45".into()),
                ..Default::default()
            },
            "2026-08-11",
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Invalid issue date: 2026-13-45 (expected YYYY-MM-DD)"
        );

        let err = update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                due_date: Some(Some("nope".into())),
                ..Default::default()
            },
            "2026-08-11",
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Invalid due date: nope (expected YYYY-MM-DD)"
        );
    }

    #[test]
    fn currency_is_normalized_to_uppercase_and_must_be_three_letters() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);

        update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                currency: Some("eur".into()),
                ..Default::default()
            },
            "2026-08-11",
        )
        .unwrap();
        assert_eq!(get_invoice(&conn, id).unwrap().currency, "EUR");

        let err = update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                currency: Some("dollars".into()),
                ..Default::default()
            },
            "2026-08-11",
        )
        .unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
        assert_eq!(
            err.to_string(),
            "Invalid currency: dollars (expected a 3-letter code like USD)"
        );
    }

    #[test]
    fn changing_the_total_clears_a_stale_stripe_link() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        set_payment_link(&conn, id, "plink_1", "https://pay.test/plink_1").unwrap();

        update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                items: Some(vec![NewLineItem {
                    description: "Rework".into(),
                    quantity: 2.0,
                    unit_amount: 250.0,
                }]),
                ..Default::default()
            },
            "2026-08-11",
        )
        .unwrap();

        let inv = get_invoice(&conn, id).unwrap();
        assert_eq!(inv.stripe_payment_link_id, None);
        assert_eq!(inv.stripe_payment_link_url, None);
    }

    #[test]
    fn an_edit_that_does_not_move_the_money_keeps_the_link() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        set_payment_link(&conn, id, "plink_1", "https://pay.test/plink_1").unwrap();

        update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                notes: Some(Some("Thanks".into())),
                ..Default::default()
            },
            "2026-08-11",
        )
        .unwrap();

        let inv = get_invoice(&conn, id).unwrap();
        assert_eq!(inv.stripe_payment_link_id.as_deref(), Some("plink_1"));
        assert_eq!(
            inv.stripe_payment_link_url.as_deref(),
            Some("https://pay.test/plink_1")
        );
    }

    #[test]
    fn update_invoice_refuses_a_published_invoice() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        mark_published(&conn, id, "2026-08-05").unwrap();

        let err = update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                notes: Some(Some("Thanks".into())),
                ..Default::default()
            },
            "2026-08-11",
        )
        .unwrap_err();
        assert_eq!(conflict_code(&err), "not_draft");
        assert_eq!(get_invoice(&conn, id).unwrap().notes, None);
    }

    #[test]
    fn voiding_a_draft_sets_voided_at_and_the_void_status() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);

        void_invoice(&conn, id, "2026-08-06").unwrap();

        let inv = get_invoice(&conn, id).unwrap();
        assert_eq!(inv.voided_at.as_deref(), Some("2026-08-06"));
        assert_eq!(inv.status, "void");
    }

    #[test]
    fn voiding_a_sent_invoice_is_allowed() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        mark_published(&conn, id, "2026-08-05").unwrap();

        void_invoice(&conn, id, "2026-08-06").unwrap();
        assert_eq!(get_invoice(&conn, id).unwrap().status, "void");
    }

    #[test]
    fn voiding_an_invoice_with_payments_is_refused() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        record_payment(&conn, id, 50.0, "2026-08-10", "ach", None).unwrap();

        let err = void_invoice(&conn, id, "2026-08-11").unwrap_err();
        assert_eq!(conflict_code(&err), "has_payments");
        assert_eq!(get_invoice(&conn, id).unwrap().voided_at, None);
    }

    #[test]
    fn voiding_a_void_invoice_is_refused() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        void_invoice(&conn, id, "2026-08-06").unwrap();

        let err = void_invoice(&conn, id, "2026-08-07").unwrap_err();
        assert_eq!(conflict_code(&err), "already_void");
        assert_eq!(
            get_invoice(&conn, id).unwrap().voided_at.as_deref(),
            Some("2026-08-06")
        );
    }

    #[test]
    fn a_voided_invoice_leaves_the_aging_buckets() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        mark_published(&conn, id, "2026-06-01").unwrap();
        assert!(ar_aging(&conn, "2026-08-04")
            .unwrap()
            .iter()
            .any(|b| b.total > 0.0));

        void_invoice(&conn, id, "2026-08-04").unwrap();

        for bucket in ar_aging(&conn, "2026-08-04").unwrap() {
            assert_eq!(
                bucket.total, 0.0,
                "bucket {} still carries money",
                bucket.label
            );
        }
    }

    #[test]
    fn voiding_a_missing_invoice_is_not_found() {
        let (_d, conn) = test_conn();
        let err = void_invoice(&conn, 99, "2026-08-06").unwrap_err();
        assert!(matches!(err, NigelError::NotFound(_)), "got: {err:?}");
        assert_eq!(err.to_string(), "Invoice not found: id 99");
    }

    /// The by-number lookup answers an absent invoice the way its by-id sibling
    /// does. Left as a raw rusqlite error it was a 500 over HTTP, and every
    /// number-keyed caller had to remember to narrow it.
    #[test]
    fn an_unknown_invoice_number_is_not_found_not_a_database_error() {
        let (_d, conn) = test_conn();
        let err = get_invoice_by_number(&conn, 4242).unwrap_err();
        assert!(matches!(err, NigelError::NotFound(_)), "got: {err:?}");
        assert_eq!(err.to_string(), "Invoice not found: #4242");
    }

    /// Void by hand, the way migration v5 leaves a voided row.
    fn void_at(conn: &Connection, id: i64, on: &str) {
        conn.execute(
            "UPDATE invoices SET voided_at = ?1 WHERE id = ?2",
            rusqlite::params![on, id],
        )
        .unwrap();
        refresh_status(conn, id, on).unwrap();
    }

    #[test]
    fn line_items_come_back_in_position_order() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![
            NewLineItem {
                description: "Design".into(),
                quantity: 2.0,
                unit_amount: 100.0,
            },
            NewLineItem {
                description: "Dev".into(),
                quantity: 3.0,
                unit_amount: 50.0,
            },
        ];
        let id = create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap();

        let rows = line_items(&conn, id).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].description, "Design");
        assert_eq!(rows[0].position, 0);
        assert_eq!(rows[0].line_total, 200.0);
        assert_eq!(rows[1].description, "Dev");
        assert_eq!(rows[1].position, 1);
        assert_eq!(rows[1].line_total, 150.0);
        assert_eq!(rows[1].invoice_id, Some(id));
    }

    #[test]
    fn aging_buckets_split_by_days_past_due() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem {
            description: "W".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        // due 5 days ago -> "1-30"
        let a = create_invoice(
            &conn,
            cid,
            "2026-07-01",
            Some("2026-07-30"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        // due 45 days ago -> "31-60"
        let b = create_invoice(
            &conn,
            cid,
            "2026-06-01",
            Some("2026-06-20"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE invoices SET status='sent', published_at='x' WHERE id IN (?1,?2)",
            [a, b],
        )
        .unwrap();

        let buckets = ar_aging(&conn, "2026-08-04").unwrap();
        let get = |label: &str| buckets.iter().find(|x| x.label == label).unwrap().total;
        assert_eq!(get("1-30"), 100.0);
        assert_eq!(get("31-60"), 100.0);
        assert_eq!(get("90+"), 0.0);
    }

    /// A published, unpaid invoice for `client`, owing `amount`, due on `due`.
    fn open_invoice(
        conn: &Connection,
        client: &str,
        issue: &str,
        due: Option<&str>,
        amount: f64,
    ) -> i64 {
        let cid = client_id(conn, client);
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: amount,
        }];
        let id = create_invoice(conn, cid, issue, due, "USD", &items, None, None).unwrap();
        mark_published(conn, id, issue).unwrap();
        id
    }

    const AGING_TODAY: &str = "2026-08-04";

    fn number_of(conn: &Connection, id: i64) -> i64 {
        get_invoice(conn, id).unwrap().number
    }

    fn bucket_of(report: &AgingReport, number: i64) -> &'static str {
        report
            .invoices
            .iter()
            .find(|i| i.number == number)
            .unwrap_or_else(|| panic!("invoice #{number} missing from the aging report"))
            .bucket
    }

    #[test]
    fn aging_detail_buckets_by_days_past_due() {
        let (_d, conn) = test_conn();
        // Due dates at 0, 1, 30, 31, 60, 61, 90 and 91 days before AGING_TODAY.
        let cases = [
            ("2026-08-04", "current"),
            ("2026-08-03", "1-30"),
            ("2026-07-05", "1-30"),
            ("2026-07-04", "31-60"),
            ("2026-06-05", "31-60"),
            ("2026-06-04", "61-90"),
            ("2026-05-06", "61-90"),
            ("2026-05-05", "90+"),
        ];
        let numbers: Vec<i64> = cases
            .iter()
            .map(|(due, _)| {
                let id = open_invoice(&conn, "Acme", "2026-01-05", Some(due), 100.0);
                number_of(&conn, id)
            })
            .collect();

        let report = ar_aging_detail(&conn, AGING_TODAY).unwrap();
        for (number, (due, expected)) in numbers.iter().zip(cases) {
            assert_eq!(bucket_of(&report, *number), expected, "due {due}");
        }
    }

    #[test]
    fn aging_detail_falls_back_to_issue_date() {
        let (_d, conn) = test_conn();
        let id = open_invoice(&conn, "Acme", "2026-06-04", None, 100.0);
        let number = number_of(&conn, id);

        let report = ar_aging_detail(&conn, AGING_TODAY).unwrap();
        let row = report.invoices.iter().find(|i| i.number == number).unwrap();
        assert_eq!(row.due_date, "2026-06-04");
        assert_eq!(row.days_past_due, 61);
        assert_eq!(row.bucket, "61-90");
    }

    #[test]
    fn aging_detail_subtracts_payments() {
        let (_d, conn) = test_conn();
        let partial = open_invoice(&conn, "Acme", "2026-07-01", Some("2026-07-20"), 100.0);
        let settled = open_invoice(&conn, "Globex", "2026-07-01", Some("2026-07-20"), 250.0);
        record_payment(&conn, partial, 40.0, "2026-07-25", "ach", None).unwrap();
        record_payment(&conn, settled, 250.0, "2026-07-25", "ach", None).unwrap();

        let report = ar_aging_detail(&conn, AGING_TODAY).unwrap();
        let partial_number = number_of(&conn, partial);
        let settled_number = number_of(&conn, settled);

        let row = report
            .invoices
            .iter()
            .find(|i| i.number == partial_number)
            .unwrap();
        assert_eq!(row.total, 100.0);
        assert_eq!(row.paid, 40.0);
        assert_eq!(row.balance, 60.0);
        assert!(
            !report.invoices.iter().any(|i| i.number == settled_number),
            "a paid invoice should not appear"
        );

        let bucket = report.buckets.iter().find(|b| b.label == "1-30").unwrap();
        assert_eq!(bucket.total, 60.0);
        assert_eq!(bucket.count, 1);
    }

    #[test]
    fn aging_detail_excludes_draft_and_void() {
        let (_d, conn) = test_conn();
        let draft_client = add_client(&conn, "Draft Co", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 500.0,
        }];
        create_invoice(
            &conn,
            draft_client,
            "2026-07-01",
            Some("2026-07-10"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        let voided = open_invoice(&conn, "Void Co", "2026-07-01", Some("2026-07-10"), 700.0);
        void_at(&conn, voided, "2026-08-01");
        let open = open_invoice(&conn, "Acme", "2026-07-01", Some("2026-07-10"), 100.0);
        let open_number = number_of(&conn, open);

        let report = ar_aging_detail(&conn, AGING_TODAY).unwrap();
        assert_eq!(report.invoices.len(), 1);
        assert_eq!(report.invoices[0].number, open_number);
        assert_eq!(report.outstanding, 100.0);
    }

    #[test]
    fn aging_detail_counts_and_total() {
        let (_d, conn) = test_conn();
        open_invoice(&conn, "Acme", "2026-01-05", Some("2026-08-31"), 100.0);
        open_invoice(&conn, "Globex", "2026-01-05", Some("2026-07-20"), 200.0);
        open_invoice(&conn, "Initech", "2026-01-05", Some("2026-07-10"), 400.0);

        let report = ar_aging_detail(&conn, AGING_TODAY).unwrap();
        for bucket in &report.buckets {
            let listed = report
                .invoices
                .iter()
                .filter(|i| i.bucket == bucket.label)
                .count();
            assert_eq!(bucket.count, listed, "count for {}", bucket.label);
        }
        let summed: f64 = report.buckets.iter().map(|b| b.total).sum();
        assert_eq!(report.outstanding, summed);
        assert_eq!(report.outstanding, 700.0);
        assert_eq!(report.as_of, AGING_TODAY);
    }

    #[test]
    fn a_malformed_as_of_date_is_invalid_not_other() {
        let (_d, conn) = test_conn();
        // Zero-padding is not *rejected* here: chrono's %Y-%m-%d accepts
        // "2026-3-1", which validate_date normalizes rather than refuses. The
        // HTTP layer is where the stricter parse belongs.
        for as_of in ["March", "2026-08-32", ""] {
            let err = ar_aging_detail(&conn, as_of).unwrap_err();
            assert!(matches!(err, NigelError::Invalid(_)), "{as_of}: {err:?}");
        }
    }

    #[test]
    fn aging_detail_orders_oldest_first() {
        let (_d, conn) = test_conn();
        open_invoice(&conn, "Acme", "2026-01-05", Some("2026-08-31"), 100.0);
        open_invoice(&conn, "Globex", "2026-01-05", Some("2026-04-01"), 200.0);
        open_invoice(&conn, "Initech", "2026-01-05", Some("2026-07-20"), 300.0);

        let report = ar_aging_detail(&conn, AGING_TODAY).unwrap();
        let days: Vec<i64> = report.invoices.iter().map(|i| i.days_past_due).collect();
        let mut sorted = days.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(days, sorted);
    }

    #[test]
    fn aging_detail_carries_client_name() {
        let (_d, conn) = test_conn();
        open_invoice(&conn, "Initech", "2026-01-05", Some("2026-07-01"), 100.0);

        let report = ar_aging_detail(&conn, AGING_TODAY).unwrap();
        assert_eq!(report.invoices[0].client, "Initech");
    }

    /// One client, three invoices of 100/200/300, newest number last returned.
    fn seed_three(conn: &Connection) -> (i64, Vec<i64>) {
        let cid = add_client(conn, "Cedar Systems", Some("ops@cedar.test"), None, None).unwrap();
        let ids = [100.0, 200.0, 300.0]
            .into_iter()
            .map(|amount| {
                let items = vec![NewLineItem {
                    description: "Work".into(),
                    quantity: 1.0,
                    unit_amount: amount,
                }];
                create_invoice(conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap()
            })
            .collect();
        (cid, ids)
    }

    #[test]
    fn list_invoices_is_newest_first_and_carries_client_and_paid() {
        let (_d, conn) = test_conn();
        let (_cid, ids) = seed_three(&conn);
        record_payment(&conn, ids[1], 50.0, "2026-08-05", "ach", None).unwrap();

        let rows = list_invoices(&conn, None, None).unwrap();
        assert_eq!(rows.len(), 3);

        let numbers: Vec<i64> = rows.iter().map(|r| r.number).collect();
        let mut descending = numbers.clone();
        descending.sort_by(|a, b| b.cmp(a));
        assert_eq!(numbers, descending, "newest first");

        for row in &rows {
            assert_eq!(row.client_name.as_deref(), Some("Cedar Systems"));
            assert_eq!(row.issue_date, "2026-08-04");
            assert_eq!(row.currency, "USD");
        }
        let middle = rows.iter().find(|r| r.id == ids[1]).unwrap();
        assert_eq!(middle.paid, 50.0);
        assert_eq!(middle.total, 200.0);
        assert_eq!(middle.balance, 150.0);
        // The stored status, whatever it is — an unpublished draft stays draft.
        assert_eq!(middle.status, get_invoice(&conn, ids[1]).unwrap().status);
        for row in rows.iter().filter(|r| r.id != ids[1]) {
            assert_eq!(row.paid, 0.0, "invoice #{} has no payments", row.number);
            assert_eq!(row.balance, row.total);
        }
    }

    #[test]
    fn list_invoices_computes_paid_in_one_aggregate_not_one_query_per_row() {
        let (_d, conn) = test_conn();
        let (_cid, ids) = seed_three(&conn);
        // Five payments spread across three invoices, in one call.
        record_payment(&conn, ids[0], 25.0, "2026-08-05", "ach", None).unwrap();
        record_payment(&conn, ids[0], 25.0, "2026-08-06", "ach", None).unwrap();
        record_payment(&conn, ids[1], 40.0, "2026-08-05", "ach", None).unwrap();
        record_payment(&conn, ids[1], 60.0, "2026-08-07", "other", None).unwrap();
        record_payment(&conn, ids[2], 300.0, "2026-08-05", "direct_deposit", None).unwrap();

        let rows = list_invoices(&conn, None, None).unwrap();
        let paid = |id: i64| rows.iter().find(|r| r.id == id).unwrap().paid;
        assert_eq!(paid(ids[0]), 50.0);
        assert_eq!(paid(ids[1]), 100.0);
        assert_eq!(paid(ids[2]), 300.0);
        assert_eq!(rows.iter().find(|r| r.id == ids[2]).unwrap().balance, 0.0);
    }

    #[test]
    fn list_invoices_keeps_an_invoice_whose_client_row_is_missing() {
        let (_d, conn) = test_conn();
        let (cid, ids) = seed_three(&conn);
        conn.execute("PRAGMA foreign_keys = OFF", []).unwrap();
        conn.execute("DELETE FROM clients WHERE id = ?1", [cid])
            .unwrap();

        // A list that hides invoices is worse than one that shows a dash.
        let rows = list_invoices(&conn, None, None).unwrap();
        assert_eq!(rows.len(), ids.len());
        assert!(rows.iter().all(|r| r.client_name.is_none()), "{rows:?}");
    }

    #[test]
    fn list_invoices_filters_by_status_word_and_by_open() {
        let (_d, conn) = test_conn();
        let (_cid, ids) = seed_three(&conn);
        mark_published(&conn, ids[0], "2026-08-05").unwrap();
        mark_published(&conn, ids[1], "2026-08-05").unwrap();
        record_payment(&conn, ids[1], 50.0, "2026-08-06", "ach", None).unwrap();

        let numbers = |status: &str| -> Vec<i64> {
            let mut ids: Vec<i64> = list_invoices(&conn, Some(status), None)
                .unwrap()
                .iter()
                .map(|r| r.id)
                .collect();
            ids.sort();
            ids
        };
        assert_eq!(numbers("draft"), vec![ids[2]]);
        assert_eq!(numbers("sent"), vec![ids[0]]);
        assert_eq!(numbers("partial"), vec![ids[1]]);
        assert_eq!(numbers("open"), vec![ids[0], ids[1]]);
        assert!(numbers("void").is_empty());
    }

    #[test]
    fn an_unknown_status_filter_is_invalid_and_names_the_legal_set() {
        let (_d, conn) = test_conn();
        let err = list_invoices(&conn, Some("archived"), None).unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
        let text = err.to_string();
        for word in ["draft", "overdue", "open"] {
            assert!(text.contains(word), "missing {word} in: {text}");
        }
    }

    #[test]
    fn list_invoices_filters_by_client() {
        let (_d, conn) = test_conn();
        let (cid, ids) = seed_three(&conn);
        let other = open_invoice(&conn, "Globex", "2026-01-05", None, 100.0);

        let mine: Vec<i64> = list_invoices(&conn, None, Some(cid))
            .unwrap()
            .iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(mine.len(), ids.len());
        assert!(!mine.contains(&other), "another client's invoice leaked in");
    }

    #[test]
    fn list_invoices_carries_the_due_date_when_there_is_one() {
        let (_d, conn) = test_conn();
        open_invoice(&conn, "Acme", "2026-01-05", Some("2026-02-05"), 100.0);
        open_invoice(&conn, "Globex", "2026-01-05", None, 100.0);

        let rows = list_invoices(&conn, None, None).unwrap();
        let dues: Vec<Option<String>> = rows.iter().map(|r| r.due_date.clone()).collect();
        assert!(dues.contains(&Some("2026-02-05".to_string())), "{dues:?}");
        assert!(dues.contains(&None), "{dues:?}");
    }

    #[test]
    fn list_invoices_on_an_empty_book_is_empty() {
        let (_d, conn) = test_conn();
        assert!(list_invoices(&conn, None, None).unwrap().is_empty());
    }

    #[test]
    fn an_unknown_payment_method_is_invalid_not_a_constraint_violation() {
        let (_d, conn) = test_conn();
        let (_cid, ids) = seed_three(&conn);

        let err = validate_payment_method("bitcoin").unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
        assert!(err.to_string().contains("direct_deposit"), "got: {err}");

        // record_payment refuses before the insert, so no CHECK ever fires.
        let err = record_payment(&conn, ids[0], 10.0, "2026-08-05", "bitcoin", None).unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
        assert!(payments(&conn, ids[0]).unwrap().is_empty());

        for method in PAYMENT_METHODS {
            assert!(validate_payment_method(method).is_ok(), "{method}");
        }
    }

    #[test]
    fn payments_come_back_oldest_first() {
        let (_d, conn) = test_conn();
        let (_cid, ids) = seed_three(&conn);
        record_payment(&conn, ids[0], 10.0, "2026-08-09", "ach", None).unwrap();
        record_payment(&conn, ids[0], 20.0, "2026-08-05", "stripe", None).unwrap();
        record_payment(&conn, ids[0], 30.0, "2026-08-05", "other", None).unwrap();

        let rows = payments(&conn, ids[0]).unwrap();
        let dates: Vec<&str> = rows.iter().map(|p| p.paid_date.as_str()).collect();
        assert_eq!(dates, ["2026-08-05", "2026-08-05", "2026-08-09"]);
        // Same date: insertion order breaks the tie.
        assert_eq!(rows[0].amount, 20.0);
        assert_eq!(rows[1].amount, 30.0);
        assert_eq!(rows[0].method, "stripe");
        assert_eq!(rows[0].invoice_id, ids[0]);
    }

    #[test]
    fn payments_for_an_invoice_with_none_is_empty() {
        let (_d, conn) = test_conn();
        let (_cid, ids) = seed_three(&conn);
        assert!(payments(&conn, ids[0]).unwrap().is_empty());
    }

    #[test]
    fn validate_date_normalizes_to_zero_padded_iso() {
        assert_eq!(validate_date("2026-8-7", "issue").unwrap(), "2026-08-07");
        assert_eq!(validate_date("2026-08-07", "issue").unwrap(), "2026-08-07");
        assert_eq!(validate_date("2026-12-31", "due").unwrap(), "2026-12-31");
        for bad in ["March", "2026-13-01", "2026-08-32", "", "2026-08-07extra"] {
            assert!(validate_date(bad, "issue").is_err(), "accepted {bad:?}");
        }
    }

    /// chrono's `%Y` reads "26" as the year 26 AD, so a two-digit-year typo would
    /// otherwise book a date two millennia off and take the status reference day
    /// with it. The error message promises `YYYY-MM-DD`; the rule keeps that promise.
    #[test]
    fn validate_date_requires_a_four_digit_year() {
        for bad in ["26-8-9", "26-08-09", "0-1-1", "126-08-09", "20267-08-09"] {
            let err = validate_date(bad, "issue").unwrap_err();
            assert!(matches!(err, NigelError::Invalid(_)), "{bad:?}: {err:?}");
            assert_eq!(
                err.to_string(),
                format!("Invalid issue date: {bad} (expected YYYY-MM-DD)")
            );
        }
        // A four-digit year that happens to be long ago is a real date, not a typo.
        assert_eq!(validate_date("0026-08-09", "issue").unwrap(), "0026-08-09");
    }

    #[test]
    fn a_two_digit_year_is_refused_on_new_edit_and_pay() {
        let (_d, conn) = test_conn();
        let cid = client_id(&conn, "Acme");
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];

        let err =
            create_invoice(&conn, cid, "26-8-9", None, "USD", &items, None, None).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Invalid issue date: 26-8-9 (expected YYYY-MM-DD)"
        );

        let id = seed_draft(&conn);
        let err = update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                due_date: Some(Some("26-8-9".into())),
                ..Default::default()
            },
            "2026-08-11",
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Invalid due date: 26-8-9 (expected YYYY-MM-DD)"
        );

        let err = record_payment(&conn, id, 50.0, "26-8-9", "ach", None).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Invalid payment date: 26-8-9 (expected YYYY-MM-DD)"
        );
        assert_eq!(paid_amount(&conn, id).unwrap(), 0.0);
    }

    #[test]
    fn an_unpadded_issue_or_due_date_is_stored_padded() {
        let (_d, conn) = test_conn();
        let cid = client_id(&conn, "Acme");
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        let id = create_invoice(
            &conn,
            cid,
            "2026-8-7",
            Some("2026-9-1"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();

        let inv = get_invoice(&conn, id).unwrap();
        assert_eq!(inv.issue_date, "2026-08-07");
        assert_eq!(inv.due_date.as_deref(), Some("2026-09-01"));
    }

    #[test]
    fn an_unpadded_date_edited_onto_a_draft_is_stored_padded() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                issue_date: Some("2026-8-7".into()),
                due_date: Some(Some("2026-9-1".into())),
                ..Default::default()
            },
            "2026-08-11",
        )
        .unwrap();

        let inv = get_invoice(&conn, id).unwrap();
        assert_eq!(inv.issue_date, "2026-08-07");
        assert_eq!(inv.due_date.as_deref(), Some("2026-09-01"));
    }

    /// `is_overdue` compares due dates as strings, where `"2026-08-20" > "2026-8-7"`
    /// is false. Both spellings must therefore land in the column the same way.
    #[test]
    fn overdue_derives_the_same_whether_the_due_date_was_typed_padded_or_not() {
        let (_d, conn) = test_conn();
        let padded = open_invoice(&conn, "Padded", "2026-07-01", Some("2026-07-05"), 100.0);
        let unpadded = open_invoice(&conn, "Unpadded", "2026-07-01", Some("2026-7-5"), 100.0);

        assert_eq!(
            refresh_status(&conn, padded, "2026-08-20").unwrap(),
            "overdue"
        );
        assert_eq!(
            refresh_status(&conn, unpadded, "2026-08-20").unwrap(),
            "overdue"
        );
    }

    /// Aging buckets from the parsed date and prints the stored one, so it is
    /// indifferent to the spelling in a way `is_overdue` is not. Both halves are
    /// pinned: the bucket a value lands in, and the string the report shows.
    #[test]
    fn aging_buckets_and_prints_the_same_whether_the_due_date_was_typed_padded_or_not() {
        let (_d, conn) = test_conn();
        open_invoice(&conn, "Padded", "2026-06-01", Some("2026-07-05"), 100.0);
        open_invoice(&conn, "Unpadded", "2026-06-01", Some("2026-7-5"), 100.0);

        let report = ar_aging_detail(&conn, AGING_TODAY).unwrap();
        let padded = report
            .invoices
            .iter()
            .find(|i| i.client == "Padded")
            .unwrap();
        let unpadded = report
            .invoices
            .iter()
            .find(|i| i.client == "Unpadded")
            .unwrap();
        assert_eq!(padded.bucket, unpadded.bucket);
        assert_eq!(padded.days_past_due, unpadded.days_past_due);
        assert_eq!(
            unpadded.due_date, "2026-07-05",
            "the stored date must already be padded"
        );
    }

    #[test]
    fn an_unpadded_as_of_date_is_reported_padded() {
        let (_d, conn) = test_conn();
        assert_eq!(
            ar_aging_detail(&conn, "2026-8-4").unwrap().as_of,
            "2026-08-04"
        );
    }

    #[test]
    fn record_payment_stores_an_unpadded_date_padded() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        record_payment(&conn, id, 50.0, "2026-8-9", "ach", None).unwrap();

        let dates: Vec<String> = payments(&conn, id)
            .unwrap()
            .into_iter()
            .map(|p| p.paid_date)
            .collect();
        assert_eq!(dates, vec!["2026-08-09".to_string()]);
    }

    /// The date reaches both the column and `refresh_status`'s reference day, so
    /// the check belongs in the data layer rather than in whichever front end
    /// remembers it.
    #[test]
    fn record_payment_refuses_a_malformed_date_instead_of_storing_it() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);

        let err = record_payment(&conn, id, 50.0, "March", "ach", None).unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "{err:?}");
        assert_eq!(
            err.to_string(),
            "Invalid payment date: March (expected YYYY-MM-DD)"
        );
        assert_eq!(
            paid_amount(&conn, id).unwrap(),
            0.0,
            "a refused payment writes no row"
        );
    }

    #[test]
    fn void_invoice_stores_an_unpadded_date_padded() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        void_invoice(&conn, id, "2026-8-6").unwrap();

        assert_eq!(
            get_invoice(&conn, id).unwrap().voided_at.as_deref(),
            Some("2026-08-06")
        );
    }

    /// An edit derives the status against the day passed in, not the invoice's own
    /// issue date.
    ///
    /// `published_at` is set by hand because this is the only shape where an
    /// *editable* invoice can reach the overdue branch at all: `mark_published`
    /// moves the status off `draft` and `ensure_editable` then refuses the edit.
    #[test]
    fn a_due_date_edit_derives_status_against_today_not_the_issue_date() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn); // issued 2026-08-04, still `draft`
        conn.execute(
            "UPDATE invoices SET published_at = '2026-08-05' WHERE id = ?1",
            [id],
        )
        .unwrap();

        update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                due_date: Some(Some("2026-08-06".into())),
                ..Default::default()
            },
            "2026-08-20",
        )
        .unwrap();

        assert_eq!(
            get_invoice(&conn, id).unwrap().status,
            "overdue",
            "derived against the issue date (2026-08-04), which is not past the due date"
        );
    }

    #[test]
    fn update_invoice_refuses_a_malformed_reference_day() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);

        let err = update_invoice(
            &conn,
            id,
            &InvoiceUpdate {
                notes: Some(Some("Thanks".into())),
                ..Default::default()
            },
            "March",
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Invalid reference date: March (expected YYYY-MM-DD)"
        );
        assert_eq!(
            get_invoice(&conn, id).unwrap().notes,
            None,
            "a refused reference day writes nothing"
        );
    }

    #[test]
    fn mark_published_stores_an_unpadded_date_padded_and_refuses_a_malformed_one() {
        let (_d, conn) = test_conn();
        let id = seed_draft(&conn);
        mark_published(&conn, id, "2026-8-5").unwrap();
        assert_eq!(
            get_invoice(&conn, id).unwrap().published_at.as_deref(),
            Some("2026-08-05")
        );

        let other = open_invoice(&conn, "Globex", "2026-08-04", None, 100.0);
        let err = mark_published(&conn, other, "March").unwrap_err();
        assert_eq!(
            err.to_string(),
            "Invalid published date: March (expected YYYY-MM-DD)"
        );
    }

    #[test]
    fn duplicating_copies_the_shape_and_regenerates_the_identity() {
        let (_d, conn) = test_conn();
        let client =
            add_client(&conn, "Cedar Systems", Some("ops@cedar.test"), None, None).unwrap();
        let items = vec![
            NewLineItem {
                description: "Retainer".into(),
                quantity: 1.0,
                unit_amount: 2_400.0,
            },
            NewLineItem {
                description: "Hosting".into(),
                quantity: 3.0,
                unit_amount: 45.0,
            },
        ];
        let source_id = create_invoice(
            &conn,
            client,
            "2026-06-01",
            Some("2026-06-15"),
            "EUR",
            &items,
            Some("Thanks for the quarter."),
            Some("Net 14."),
        )
        .unwrap();
        mark_published(&conn, source_id, "2026-06-02").unwrap();
        set_payment_link(&conn, source_id, "plink_1", "https://pay.example.test/1").unwrap();
        let source = get_invoice(&conn, source_id).unwrap();

        let copy_id = duplicate_invoice(&conn, source_id, "2026-09-01").unwrap();
        let copy = get_invoice(&conn, copy_id).unwrap();

        // Copied.
        assert_eq!(copy.client_id, source.client_id);
        assert_eq!(copy.currency, "EUR");
        assert_eq!(copy.notes.as_deref(), Some("Thanks for the quarter."));
        assert_eq!(copy.terms.as_deref(), Some("Net 14."));
        assert_eq!(copy.subtotal, source.subtotal);
        assert_eq!(copy.total, source.total);
        let copied: Vec<(String, f64, f64)> = line_items(&conn, copy_id)
            .unwrap()
            .into_iter()
            .map(|i| (i.description, i.quantity, i.unit_amount))
            .collect();
        assert_eq!(
            copied,
            vec![
                ("Retainer".to_string(), 1.0, 2_400.0),
                ("Hosting".to_string(), 3.0, 45.0),
            ]
        );

        // Regenerated.
        assert_eq!(copy.number, source.number + 1);
        assert_ne!(copy.token, source.token);
        assert_eq!(copy.status, "draft");
        assert_eq!(copy.published_at, None);
        assert_eq!(copy.voided_at, None);
        assert_eq!(copy.stripe_payment_link_id, None);
        assert_eq!(copy.stripe_payment_link_url, None);
    }

    #[test]
    fn duplicating_preserves_the_issue_to_due_offset_in_days() {
        let (_d, conn) = test_conn();
        let client = add_client(&conn, "Globex", Some("ap@globex.test"), None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Audit".into(),
            quantity: 1.0,
            unit_amount: 500.0,
        }];

        // Net 14 duplicates as Net 14, across a month boundary and a leap February.
        let net14 = create_invoice(
            &conn,
            client,
            "2026-01-20",
            Some("2026-02-03"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        let copy = get_invoice(
            &conn,
            duplicate_invoice(&conn, net14, "2028-02-20").unwrap(),
        )
        .unwrap();
        assert_eq!(copy.issue_date, "2028-02-20");
        assert_eq!(copy.due_date.as_deref(), Some("2028-03-05"));

        // No due date on the source means none on the copy.
        let open =
            create_invoice(&conn, client, "2026-01-20", None, "USD", &items, None, None).unwrap();
        let copy =
            get_invoice(&conn, duplicate_invoice(&conn, open, "2026-09-01").unwrap()).unwrap();
        assert_eq!(copy.due_date, None);

        // A same-day due date stays a same-day due date rather than becoming none.
        let same = create_invoice(
            &conn,
            client,
            "2026-01-20",
            Some("2026-01-20"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        let copy =
            get_invoice(&conn, duplicate_invoice(&conn, same, "2026-09-01").unwrap()).unwrap();
        assert_eq!(copy.due_date.as_deref(), Some("2026-09-01"));
    }

    #[test]
    fn any_source_state_duplicates_because_duplication_reads_a_shape() {
        let (_d, conn) = test_conn();
        let client =
            add_client(&conn, "Juniper Labs", Some("ap@juniper.test"), None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Workshop".into(),
            quantity: 1.0,
            unit_amount: 800.0,
        }];

        let draft = create_invoice(
            &conn,
            client,
            "2026-05-01",
            Some("2026-05-31"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();

        let sent = create_invoice(
            &conn,
            client,
            "2026-05-01",
            Some("2026-05-31"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        mark_published(&conn, sent, "2026-05-01").unwrap();
        refresh_status(&conn, sent, "2026-05-02").unwrap();

        let paid = create_invoice(
            &conn,
            client,
            "2026-05-01",
            Some("2026-05-31"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        mark_published(&conn, paid, "2026-05-01").unwrap();
        record_payment(&conn, paid, 800.0, "2026-05-10", "direct_deposit", None).unwrap();

        let voided = create_invoice(
            &conn,
            client,
            "2026-05-01",
            Some("2026-05-31"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        void_invoice(&conn, voided, "2026-05-04").unwrap();
        refresh_status(&conn, voided, "2026-05-05").unwrap();

        for (label, source) in [
            ("draft", draft),
            ("sent", sent),
            ("paid", paid),
            ("void", voided),
        ] {
            let copy_id = duplicate_invoice(&conn, source, "2026-08-20")
                .unwrap_or_else(|e| panic!("{label} source refused: {e}"));
            let copy = get_invoice(&conn, copy_id).unwrap();
            assert_eq!(copy.status, "draft", "{label} duplicated into a non-draft");
            assert_eq!(copy.total, 800.0, "{label}");
        }
    }

    #[test]
    fn duplicating_for_an_archived_client_refuses_the_way_create_invoice_does() {
        let (_d, conn) = test_conn();
        let client = add_client(
            &conn,
            "Harbor & Vale",
            Some("ap@harborvale.test"),
            None,
            None,
        )
        .unwrap();
        let items = vec![NewLineItem {
            description: "Retainer".into(),
            quantity: 1.0,
            unit_amount: 1_000.0,
        }];
        let source =
            create_invoice(&conn, client, "2026-05-01", None, "USD", &items, None, None).unwrap();

        crate::invoicing::clients::archive_client(&conn, client, "2026-06-01").unwrap();

        let err = duplicate_invoice(&conn, source, "2026-08-20").unwrap_err();
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
        assert!(err.to_string().contains("Harbor & Vale"), "got: {err}");
    }

    #[test]
    fn duplicating_a_missing_invoice_is_not_found_and_reserves_no_number() {
        let (_d, conn) = test_conn();
        let before = next_number(&conn).unwrap();
        let err = duplicate_invoice(&conn, 404, "2026-08-20").unwrap_err();
        assert!(matches!(err, NigelError::NotFound(_)), "got: {err:?}");
        assert_eq!(next_number(&conn).unwrap(), before);
    }
}
