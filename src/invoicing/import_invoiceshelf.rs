//! One-time importer for an InvoiceShelf SQLite database.
//!
//! InvoiceShelf stores money as integer cents; Nigel stores REAL dollars, so
//! every amount is divided by 100 at this boundary. The importer also seeds the
//! `next_invoice_number` metadata key so newly created invoices continue the
//! imported numbering sequence.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;

use crate::db::set_metadata;
use crate::error::{NigelError, Result};
use crate::invoicing::invoices::{gen_token, validate_date};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub clients: u32,
    pub invoices: u32,
    pub payments: u32,
    pub next_number: i64,
    /// Source dates copied verbatim because Nigel's own rule could not read them.
    pub unparsed_dates: u32,
    /// Source addresses copied verbatim because they carry a character no mail
    /// header may. They are imported all the same — see the writer below.
    pub unusable_emails: u32,
}

/// A source date in Nigel's stored form, or the source's own text when that rule
/// cannot read it.
///
/// The importer writes with raw SQL and is the one path into the invoicing tables
/// that bypasses a writer, so it applies the rule itself rather than letting
/// another system's shapes through. It never *rejects*: a one-time mirror of
/// somebody's old books must not fail on a date, and inventing a day is worse
/// than keeping theirs.
fn normalized_date(value: &str, counter: &mut u32) -> String {
    match validate_date(value, "imported") {
        Ok(padded) => padded,
        Err(_) => {
            *counter += 1;
            value.to_string()
        }
    }
}

/// Numbering never rewinds below the sequence Nigel already assumes
/// (`invoices::NEXT_NUMBER_DEFAULT`), so a source with fewer or no invoices
/// still leaves `next_invoice_number` at 1248.
const NEXT_NUMBER_FLOOR: i64 = 1247;

fn cents_to_dollars(cents: i64) -> f64 {
    cents as f64 / 100.0
}

pub fn import(dest: &Connection, invoiceshelf_db: &Path) -> Result<ImportSummary> {
    let src = Connection::open_with_flags(invoiceshelf_db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut summary = ImportSummary {
        clients: 0,
        invoices: 0,
        payments: 0,
        next_number: NEXT_NUMBER_FLOOR + 1,
        unparsed_dates: 0,
        unusable_emails: 0,
    };
    let mut max_number: i64 = NEXT_NUMBER_FLOOR;

    let tx = dest.unchecked_transaction()?;

    // Customers -> clients. Keep a source_id -> dest_id map.
    let mut customer_map = HashMap::new();
    {
        let mut stmt = src.prepare("SELECT id, name, email FROM customers")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (src_id, name, email) in rows {
            // A raw insert rather than `add_client`, because import data
            // legitimately carries the blank and duplicate names `add_client`
            // refuses and this importer's job is to take what is there.
            dest.execute(
                "INSERT INTO clients (name) VALUES (?1)",
                rusqlite::params![name],
            )?;
            let dest_id = dest.last_insert_rowid();

            // Copied verbatim and counted, never refused: an address written
            // years ago in another program is not something the operator can
            // correct before importing it, and one bad character must not
            // abort a whole migration. The same posture this importer already
            // takes toward a date it cannot parse, and the one v8 takes toward
            // the column it backfills. A send will refuse it later, by name.
            if let Some(address) = email.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
                if crate::invoicing::mailgun::validate_header_value(address, "email").is_err() {
                    summary.unusable_emails += 1;
                }
            }
            crate::invoicing::clients::set_billing_email_unchecked(
                dest,
                dest_id,
                email.as_deref(),
            )?;

            customer_map.insert(src_id, dest_id);
            summary.clients += 1;
        }
    }

    // Invoices + their items. `invoice_date` is a datetime column, so truncate
    // it to a plain date; the currency code lives in a separate table.
    let mut invoice_map = HashMap::new();
    {
        let mut stmt = src.prepare(
            "SELECT i.id, i.invoice_number, i.customer_id, date(i.invoice_date), i.due_date,
                    i.total, i.paid_status, c.code
             FROM invoices i LEFT JOIN currencies c ON c.id = i.currency_id",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, Option<String>>(7)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut istmt = src.prepare(
            "SELECT name, quantity, price, total FROM invoice_items
             WHERE invoice_id = ?1 ORDER BY id",
        )?;

        for (src_id, number_str, cust, issue, due, total_cents, paid_status, currency) in rows {
            let number: i64 = number_str.trim().parse().map_err(|_| {
                NigelError::Other(format!("non-numeric invoice number '{number_str}'"))
            })?;
            max_number = max_number.max(number);
            let client_id = *customer_map.get(&cust).ok_or_else(|| {
                NigelError::Other(format!(
                    "invoice {number} references missing customer {cust}"
                ))
            })?;
            let total = cents_to_dollars(total_cents);
            let due = due.map(|d| normalized_date(&d, &mut summary.unparsed_dates));
            let status = if paid_status.eq_ignore_ascii_case("PAID") {
                "paid"
            } else {
                "sent"
            };
            dest.execute(
                "INSERT INTO invoices (number, client_id, issue_date, due_date, status, currency,
                    subtotal, tax, total, token, published_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9, ?3)",
                rusqlite::params![
                    number,
                    client_id,
                    issue,
                    due,
                    status,
                    currency.unwrap_or_else(|| "USD".into()),
                    total,
                    total,
                    gen_token()
                ],
            )?;
            let dest_invoice_id = dest.last_insert_rowid();
            invoice_map.insert(src_id, dest_invoice_id);
            summary.invoices += 1;

            let items = istmt
                .query_map([src_id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, f64>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, i64>(3)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for (pos, (name, qty, price, line_total)) in items.into_iter().enumerate() {
                dest.execute(
                    "INSERT INTO invoice_line_items (invoice_id, description, quantity, unit_amount, line_total, position)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![
                        dest_invoice_id,
                        name,
                        qty,
                        cents_to_dollars(price),
                        cents_to_dollars(line_total),
                        pos as i64
                    ],
                )?;
            }
        }
    }

    // Payments.
    {
        let mut stmt = src.prepare("SELECT invoice_id, amount, payment_date FROM payments")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (src_invoice, amount_cents, date) in rows {
            if let Some(dest_id) = invoice_map.get(&src_invoice) {
                let date = normalized_date(&date, &mut summary.unparsed_dates);
                dest.execute(
                    "INSERT INTO invoice_payments (invoice_id, amount, paid_date, method)
                     VALUES (?1, ?2, ?3, 'other')",
                    rusqlite::params![dest_id, cents_to_dollars(amount_cents), date],
                )?;
                summary.payments += 1;
            }
        }
    }

    summary.next_number = max_number + 1;
    set_metadata(
        dest,
        "next_invoice_number",
        &summary.next_number.to_string(),
    )?;
    tx.commit()?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, get_metadata, init_db};
    use crate::migrations::run_migrations;

    fn dest_conn() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("nigel.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    /// The subset of the live InvoiceShelf schema this importer reads.
    const SOURCE_DDL: &str =
        "CREATE TABLE customers (id INTEGER PRIMARY KEY, name TEXT, email TEXT);
         CREATE TABLE currencies (id INTEGER PRIMARY KEY, code TEXT);
         CREATE TABLE invoices (id INTEGER PRIMARY KEY, invoice_number TEXT, customer_id INTEGER,
            invoice_date DATETIME, due_date TEXT, total INTEGER, paid_status TEXT, currency_id INTEGER);
         CREATE TABLE invoice_items (id INTEGER PRIMARY KEY, invoice_id INTEGER, name TEXT,
            quantity REAL, price INTEGER, total INTEGER);
         CREATE TABLE payments (id INTEGER PRIMARY KEY, invoice_id INTEGER, amount INTEGER, payment_date TEXT);";

    fn empty_source(path: &std::path::Path) -> rusqlite::Connection {
        let c = rusqlite::Connection::open(path).unwrap();
        c.execute_batch(SOURCE_DDL).unwrap();
        c
    }

    fn fixture_invoiceshelf(path: &std::path::Path) {
        let c = empty_source(path);
        c.execute_batch(
            "INSERT INTO customers VALUES (1,'Acme','a@b.test');
             INSERT INTO currencies VALUES (4,'USD'),(5,'CAD');
             INSERT INTO invoices VALUES (1,'1247',1,'2026-07-01','2026-07-31',66000,'PAID',4);
             INSERT INTO invoices VALUES (2,'1246',1,'2026-06-01 00:00:00',NULL,101250,'UNPAID',5);
             INSERT INTO invoices VALUES (3,'1245',1,'2026-05-01',NULL,1000,'UNPAID',NULL);
             INSERT INTO invoice_items VALUES (1,1,'Consulting',1,66000,66000);
             INSERT INTO invoice_items VALUES (2,2,'Design',6.75,15000,101250);
             INSERT INTO payments VALUES (1,1,66000,'2026-07-15');
             INSERT INTO payments VALUES (2,999,5000,'2026-07-20');",
        )
        .unwrap();
    }

    /// AC #7: the one address the source has lands as a billing contact.
    #[test]
    fn an_imported_customer_email_becomes_its_billing_contact() {
        use crate::invoicing::clients::{get_client, list_contacts};

        let (_d, dest) = dest_conn();
        let src_dir = tempfile::tempdir().unwrap();
        let src_path = src_dir.path().join("invoiceshelf.sqlite");
        fixture_invoiceshelf(&src_path);

        import(&dest, &src_path).unwrap();

        let id: i64 = dest
            .query_row("SELECT id FROM clients WHERE name = 'Acme'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            get_client(&dest, id).unwrap().email.as_deref(),
            Some("a@b.test")
        );

        let contacts = list_contacts(&dest, id).unwrap();
        assert_eq!(contacts.len(), 1);
        assert!(contacts[0].is_billing);
        assert_eq!(contacts[0].position, 0);
    }

    /// Somebody's years-old address is not something they can correct before
    /// importing it, so it is copied and counted rather than refused.
    #[test]
    fn an_address_a_mail_header_could_not_carry_is_imported_and_counted() {
        use crate::invoicing::clients::list_contacts;

        let (_d, dest) = dest_conn();
        let src_dir = tempfile::tempdir().unwrap();
        let src_path = src_dir.path().join("invoiceshelf.sqlite");
        let c = empty_source(&src_path);
        c.execute(
            "INSERT INTO customers VALUES (1,'Acme',?1)",
            ["ap@acme.test\r\nBcc: x@y.test"],
        )
        .unwrap();
        c.execute(
            "INSERT INTO customers VALUES (2,'Globex','ok@globex.test')",
            [],
        )
        .unwrap();
        drop(c);

        let summary = import(&dest, &src_path).unwrap();

        assert_eq!(summary.clients, 2, "the import ran to completion");
        assert_eq!(summary.unusable_emails, 1);

        let id: i64 = dest
            .query_row("SELECT id FROM clients WHERE name = 'Acme'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let contacts = list_contacts(&dest, id).unwrap();
        assert_eq!(contacts.len(), 1);
        assert_eq!(
            contacts[0].email, "ap@acme.test\r\nBcc: x@y.test",
            "copied verbatim, the way an unparseable date is"
        );
    }

    #[test]
    fn an_imported_customer_with_no_email_gets_no_contact() {
        use crate::invoicing::clients::list_contacts;

        let (_d, dest) = dest_conn();
        let src_dir = tempfile::tempdir().unwrap();
        let src_path = src_dir.path().join("invoiceshelf.sqlite");
        let c = empty_source(&src_path);
        c.execute_batch("INSERT INTO customers VALUES (1,'Globex',NULL);")
            .unwrap();
        drop(c);

        import(&dest, &src_path).unwrap();

        let id: i64 = dest
            .query_row("SELECT id FROM clients WHERE name = 'Globex'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(list_contacts(&dest, id).unwrap().is_empty());
    }

    #[test]
    fn imports_customers_invoices_items_payments_and_sets_next_number() {
        let (_d, dest) = dest_conn();
        let src_dir = tempfile::tempdir().unwrap();
        let src_path = src_dir.path().join("invoiceshelf.sqlite");
        fixture_invoiceshelf(&src_path);

        let summary = import(&dest, &src_path).unwrap();
        assert_eq!(summary.clients, 1);
        assert_eq!(summary.invoices, 3);
        // The payment pointing at a nonexistent invoice is skipped.
        assert_eq!(summary.payments, 1);
        assert_eq!(summary.next_number, 1248);

        // cents -> dollars
        let total: f64 = dest
            .query_row("SELECT total FROM invoices WHERE number=1247", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(total, 660.0);
        let paid: f64 = dest
            .query_row("SELECT amount FROM invoice_payments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(paid, 660.0);
        assert_eq!(get_metadata(&dest, "next_invoice_number").unwrap(), "1248");

        // paid_status mapping and currency resolved through the currencies join.
        let (status, currency): (String, String) = dest
            .query_row(
                "SELECT status, currency FROM invoices WHERE number=1247",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "paid");
        assert_eq!(currency, "USD");

        // datetime invoice_date is truncated; NULL due_date and UNPAID carry over.
        let (issue, due, status, currency): (String, Option<String>, String, String) = dest
            .query_row(
                "SELECT issue_date, due_date, status, currency FROM invoices WHERE number=1246",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(issue, "2026-06-01");
        assert_eq!(due, None);
        assert_eq!(status, "sent");
        assert_eq!(currency, "CAD");

        // NULL currency_id falls back to USD.
        let currency: String = dest
            .query_row("SELECT currency FROM invoices WHERE number=1245", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(currency, "USD");

        // Line items: fractional quantity preserved, amounts in dollars.
        let (desc, qty, unit, line_total): (String, f64, f64, f64) = dest
            .query_row(
                "SELECT li.description, li.quantity, li.unit_amount, li.line_total
                 FROM invoice_line_items li JOIN invoices i ON i.id = li.invoice_id
                 WHERE i.number = 1246",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(desc, "Design");
        assert_eq!(qty, 6.75);
        assert_eq!(unit, 150.0);
        assert_eq!(line_total, 1012.50);
    }

    #[test]
    fn empty_source_leaves_numbering_at_the_default() {
        let (_d, dest) = dest_conn();
        let src_dir = tempfile::tempdir().unwrap();
        let src_path = src_dir.path().join("empty.sqlite");
        empty_source(&src_path);

        let summary = import(&dest, &src_path).unwrap();
        assert_eq!(summary.invoices, 0);
        assert_eq!(summary.next_number, 1248);
        assert_eq!(get_metadata(&dest, "next_invoice_number").unwrap(), "1248");
    }

    /// The InvoiceShelf import inserts customers with raw SQL, deliberately
    /// bypassing `add_client`'s duplicate-name check: it is a faithful copy of
    /// another system's customer table, and that system does not guarantee unique
    /// names. This is the test a `UNIQUE` index on `clients.name` would break,
    /// which is why it is here — see the `clients.name` note in CLAUDE.md.
    #[test]
    fn two_source_customers_with_the_same_name_import_as_two_clients() {
        let (_d, dest) = dest_conn();
        let src_dir = tempfile::tempdir().unwrap();
        let src_path = src_dir.path().join("invoiceshelf.sqlite");
        {
            let c = empty_source(&src_path);
            c.execute_batch(
                "INSERT INTO customers VALUES (1,'Acme Co','ap@acme.test');
                 INSERT INTO customers VALUES (2,'Acme Co','billing@acme.test');",
            )
            .unwrap();
        }

        let summary = import(&dest, &src_path).unwrap();
        assert_eq!(summary.clients, 2);
        let same_name: i64 = dest
            .query_row(
                "SELECT COUNT(*) FROM clients WHERE name = 'Acme Co'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            same_name, 2,
            "the import mirrors its source; it does not merge or rename"
        );
    }

    /// The importer writes with raw SQL, so it is the one path into the invoicing
    /// tables that does not go through a writer. Without normalizing here it would
    /// reintroduce exactly the unpadded dates v6 exists to remove.
    #[test]
    fn imported_due_and_payment_dates_are_stored_padded() {
        let (_d, dest) = dest_conn();
        let src_dir = tempfile::tempdir().unwrap();
        let src_path = src_dir.path().join("invoiceshelf.sqlite");
        {
            let c = empty_source(&src_path);
            c.execute_batch(
                "INSERT INTO customers VALUES (1,'Acme','a@b.test');
                 INSERT INTO currencies VALUES (4,'USD');
                 INSERT INTO invoices VALUES (1,'1247',1,'2026-07-01','2026-8-3',10000,'UNPAID',4);
                 INSERT INTO payments VALUES (1,1,5000,'2026-7-9');",
            )
            .unwrap();
        }

        let summary = import(&dest, &src_path).unwrap();
        assert_eq!(summary.unparsed_dates, 0);

        let due: Option<String> = dest
            .query_row(
                "SELECT due_date FROM invoices WHERE number = 1247",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(due.as_deref(), Some("2026-08-03"));

        let paid: String = dest
            .query_row("SELECT paid_date FROM invoice_payments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(paid, "2026-07-09");
    }

    /// A source date Nigel's rule cannot read is copied as it stands — the import
    /// is a faithful mirror and refuses to invent a day — but it is counted so the
    /// operator hears about it rather than discovering it in a report.
    #[test]
    fn an_unreadable_source_date_is_kept_verbatim_and_counted() {
        let (_d, dest) = dest_conn();
        let src_dir = tempfile::tempdir().unwrap();
        let src_path = src_dir.path().join("invoiceshelf.sqlite");
        {
            let c = empty_source(&src_path);
            c.execute_batch(
                "INSERT INTO customers VALUES (1,'Acme','a@b.test');
                 INSERT INTO currencies VALUES (4,'USD');
                 INSERT INTO invoices VALUES (1,'1247',1,'2026-07-01','whenever',10000,'UNPAID',4);
                 INSERT INTO payments VALUES (1,1,5000,'2026-07-09 00:00:00');",
            )
            .unwrap();
        }

        let summary = import(&dest, &src_path).unwrap();
        assert_eq!(summary.unparsed_dates, 2);

        let due: Option<String> = dest
            .query_row(
                "SELECT due_date FROM invoices WHERE number = 1247",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(due.as_deref(), Some("whenever"));

        let paid: String = dest
            .query_row("SELECT paid_date FROM invoice_payments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(paid, "2026-07-09 00:00:00");
    }
}
