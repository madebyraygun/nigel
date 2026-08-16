use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{DeleteBlock, NigelError, Result};
use crate::invoicing::mailgun::validate_header_value;
use crate::models::Client;

/// Is this name already taken by some other client?
///
/// The column has no UNIQUE constraint, so the check lives here rather than in
/// a caller — the `accounts::add_account` precedent, for the same reason: the
/// CLI, the TUI and the API all insert through this function.
fn name_taken(conn: &Connection, name: &str, except: Option<i64>) -> Result<bool> {
    let taken: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM clients WHERE name = ?1 AND id IS NOT ?2)",
        rusqlite::params![name, except],
        |row| row.get(0),
    )?;
    Ok(taken)
}

/// Add a client in its own transaction.
///
/// A caller that writes something else in the same breath — a whole contact
/// list, say — wants [`add_client_within`] and one transaction of its own, so
/// a refusal on the second half leaves no client row behind. This is the
/// `sync_all_report`/`sync_all_report_within` split, applied to writes.
pub fn add_client(
    conn: &Connection,
    name: &str,
    email: Option<&str>,
    billing_address: Option<&str>,
    notes: Option<&str>,
) -> Result<i64> {
    let tx = conn.unchecked_transaction()?;
    let id = add_client_within(&tx, name, billing_address, notes)?;
    set_billing_email(&tx, id, email)?;
    tx.commit()?;
    Ok(id)
}

/// The client row alone, inside the caller's transaction.
///
/// No email: an address is a `client_contacts` row, and a caller composing
/// this with [`set_contacts_within`] would otherwise write the billing contact
/// twice.
pub fn add_client_within(
    conn: &Connection,
    name: &str,
    billing_address: Option<&str>,
    notes: Option<&str>,
) -> Result<i64> {
    let name = name.trim();
    if name.is_empty() {
        return Err(NigelError::Invalid("Name is required".into()));
    }
    if name_taken(conn, name, None)? {
        return Err(NigelError::DuplicateName {
            kind: "Client",
            name: name.to_string(),
        });
    }
    conn.execute(
        "INSERT INTO clients (name, billing_address, notes) VALUES (?1, ?2, ?3)",
        rusqlite::params![name, billing_address, notes],
    )?;
    Ok(conn.last_insert_rowid())
}

/// One address a client can be reached at.
///
/// Exactly one row per client carries `is_billing`: a partial unique index
/// gives *at most* one, and the normalize step every write runs gives *at
/// least* one whenever there is any contact at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientContact {
    pub id: i64,
    pub client_id: i64,
    pub name: Option<String>,
    pub email: String,
    pub title: Option<String>,
    pub is_billing: bool,
    pub position: i64,
}

/// A contact as a caller supplies it — no id, because the write is a
/// whole-list replacement, the shape `update_invoice` uses for `items`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewContact {
    pub email: String,
    pub name: Option<String>,
    pub title: Option<String>,
    #[serde(default)]
    pub is_billing: bool,
}

const CONTACT_COLUMNS: &str = "id, client_id, name, email, title, is_billing, position";

fn contact_from_row(r: &rusqlite::Row) -> rusqlite::Result<ClientContact> {
    Ok(ClientContact {
        id: r.get(0)?,
        client_id: r.get(1)?,
        name: r.get(2)?,
        email: r.get(3)?,
        title: r.get(4)?,
        is_billing: r.get::<_, i64>(5)? != 0,
        position: r.get(6)?,
    })
}

/// A client's addresses, the billing one first and the rest in the order they
/// were written.
pub fn list_contacts(conn: &Connection, client_id: i64) -> Result<Vec<ClientContact>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {CONTACT_COLUMNS} FROM client_contacts
          WHERE client_id = ?1 ORDER BY is_billing DESC, position, id"
    ))?;
    let rows = stmt
        .query_map([client_id], contact_from_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// At least an address, no blank or duplicate one, at most one billing row.
///
/// It does **not** shape-check an address, for the reason the rest of the
/// codebase does not: `nigel client add --email` never has, and a form that
/// refused what the CLI accepts would make the surfaces disagree about what a
/// client is. What it does refuse is a line break, because these strings become
/// mail headers.
pub fn validate_contacts(contacts: &[NewContact]) -> Result<()> {
    let mut seen: Vec<String> = Vec::new();
    let mut billing = 0;
    for contact in contacts {
        let email = contact.email.trim();
        if email.is_empty() {
            return Err(NigelError::Invalid(
                "A contact needs an email address".into(),
            ));
        }
        validate_header_value(email, "contact email")?;
        if let Some(name) = &contact.name {
            validate_header_value(name, "contact name")?;
        }
        if let Some(title) = &contact.title {
            validate_header_value(title, "contact title")?;
        }
        let key = email.to_lowercase();
        if seen.contains(&key) {
            return Err(NigelError::Invalid(format!(
                "'{email}' is listed twice — one address per client"
            )));
        }
        seen.push(key);
        if contact.is_billing {
            billing += 1;
        }
    }
    if billing > 1 {
        return Err(NigelError::Invalid(
            "exactly one contact can be the billing recipient".into(),
        ));
    }
    Ok(())
}

/// Replace a client's whole contact list, in one transaction.
///
/// Validation runs before anything is deleted, and positions are the order the
/// contacts arrived in. Ids are not preserved — the same property `items` has
/// on `update_invoice`, and nothing references a contact id.
pub fn set_contacts(conn: &Connection, client_id: i64, contacts: &[NewContact]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    set_contacts_within(&tx, client_id, contacts)?;
    tx.commit()?;
    Ok(())
}

/// [`set_contacts`] inside the caller's transaction, for a surface writing the
/// client row and its addresses as one thing.
pub fn set_contacts_within(
    conn: &Connection,
    client_id: i64,
    contacts: &[NewContact],
) -> Result<()> {
    ensure_client_exists(conn, client_id)?;
    validate_contacts(contacts)?;

    // Normalization, after validation: a list that names no billing recipient
    // makes its first row one, so a client with contacts but no billing address
    // is not representable.
    let billing_index = contacts.iter().position(|c| c.is_billing).unwrap_or(0);

    conn.execute(
        "DELETE FROM client_contacts WHERE client_id = ?1",
        [client_id],
    )?;
    for (position, contact) in contacts.iter().enumerate() {
        conn.execute(
            "INSERT INTO client_contacts (client_id, name, email, title, is_billing, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                client_id,
                optional(contact.name.as_deref()),
                contact.email.trim(),
                optional(contact.title.as_deref()),
                i64::from(position == billing_index),
                position as i64,
            ],
        )?;
    }
    Ok(())
}

/// A trimmed value, or `None` when it is blank.
fn optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

/// The one place a billing address is written, so `add_client`,
/// `update_client` and the InvoiceShelf importer cannot disagree about what
/// setting an email means.
///
/// `Some(addr)` promotes an address the client already has, or updates the
/// billing row, or adds one; `None` deletes the billing row and promotes the
/// next contact by position, so clearing the email of a client with cc rows
/// leaves them reachable rather than orphaned.
pub(crate) fn set_billing_email(
    conn: &Connection,
    client_id: i64,
    email: Option<&str>,
) -> Result<()> {
    if let Some(address) = email.map(str::trim).filter(|a| !a.is_empty()) {
        validate_header_value(address, "email")?;
    }
    set_billing_email_unchecked(conn, client_id, email)
}

/// [`set_billing_email`] without the header check, for data that already
/// exists somewhere else and is only being copied.
///
/// The InvoiceShelf importer is the one caller: refusing somebody's years-old
/// address would abort a whole migration over a value they cannot edit until
/// it is imported. It counts what it copies instead — v8's own posture toward
/// the column it backfills.
pub(crate) fn set_billing_email_unchecked(
    conn: &Connection,
    client_id: i64,
    email: Option<&str>,
) -> Result<()> {
    let address = email.map(str::trim).filter(|a| !a.is_empty());

    let Some(address) = address else {
        conn.execute(
            "DELETE FROM client_contacts WHERE client_id = ?1 AND is_billing = 1",
            [client_id],
        )?;
        promote_first_contact(conn, client_id)?;
        return Ok(());
    };

    // `.optional()`, not `.ok()`: only "no such row" means the client does not
    // already hold this address. Swallowing a real database error here would
    // insert a duplicate the unique index then refuses, reporting a constraint
    // violation in place of whatever actually went wrong.
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM client_contacts
              WHERE client_id = ?1 AND lower(email) = lower(?2)",
            rusqlite::params![client_id, address],
            |r| r.get(0),
        )
        .optional()?;

    if let Some(id) = existing {
        // The address is already on the client, as a cc. Moving the flag beats
        // inserting a duplicate the unique index would refuse.
        conn.execute(
            "UPDATE client_contacts SET is_billing = 0 WHERE client_id = ?1",
            [client_id],
        )?;
        conn.execute(
            "UPDATE client_contacts SET is_billing = 1 WHERE id = ?1",
            [id],
        )?;
        return Ok(());
    }

    let changed = conn.execute(
        "UPDATE client_contacts SET email = ?2 WHERE client_id = ?1 AND is_billing = 1",
        rusqlite::params![client_id, address],
    )?;
    if changed == 0 {
        let next: i64 = conn.query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM client_contacts WHERE client_id = ?1",
            [client_id],
            |r| r.get(0),
        )?;
        conn.execute(
            "INSERT INTO client_contacts (client_id, email, is_billing, position)
             VALUES (?1, ?2, 1, ?3)",
            rusqlite::params![client_id, address, next],
        )?;
    }
    Ok(())
}

/// Give the billing flag to the lowest-position contact, if any remain.
fn promote_first_contact(conn: &Connection, client_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE client_contacts SET is_billing = 1
          WHERE id = (SELECT id FROM client_contacts WHERE client_id = ?1
                       ORDER BY position, id LIMIT 1)",
        [client_id],
    )?;
    Ok(())
}

/// Which clients a list wants.
///
/// An enum rather than a bool so a call site says what it means:
/// `list_clients(&conn, ClientScope::Active)`. There is no default — every
/// surface states the scope it is asking for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientScope {
    Active,
    All,
}

impl ClientScope {
    fn where_clause(self) -> &'static str {
        match self {
            ClientScope::Active => "WHERE archived_at IS NULL",
            ClientScope::All => "",
        }
    }
}

/// `email` is a projection of the billing contact rather than a column: the
/// field kept its meaning — the address an invoice is sent to — which is what
/// keeps `require_email`, `{{CLIENT_EMAIL}}`, `format_client_list` and the wire
/// shape working unchanged. The subquery is correlated, not an N+1.
const CLIENT_COLUMNS: &str = "id, name,
     (SELECT c.email FROM client_contacts c
       WHERE c.client_id = clients.id AND c.is_billing = 1) AS email,
     billing_address, notes, archived_at";

fn client_from_row(r: &rusqlite::Row) -> rusqlite::Result<Client> {
    Ok(Client {
        id: r.get(0)?,
        name: r.get(1)?,
        email: r.get(2)?,
        billing_address: r.get(3)?,
        notes: r.get(4)?,
        archived_at: r.get(5)?,
    })
}

pub fn get_client(conn: &Connection, id: i64) -> Result<Client> {
    conn.query_row(
        &format!("SELECT {CLIENT_COLUMNS} FROM clients WHERE id = ?1"),
        [id],
        client_from_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            NigelError::NotFound(format!("Client not found: id {id}"))
        }
        other => NigelError::Db(other),
    })
}

/// Cheap existence probe for callers that only need the id to be real.
pub fn ensure_client_exists(conn: &Connection, id: i64) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM clients WHERE id = ?1)",
        [id],
        |r| r.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(NigelError::NotFound(format!("Client not found: id {id}")))
    }
}

/// Fields to change on a client. `None` leaves a field alone; `Some(None)`
/// clears it — the convention `cli::rules::RuleUpdate` uses for `vendor`.
#[derive(Debug, Default, Clone)]
pub struct ClientUpdate {
    /// NOT NULL in the schema, so it can be renamed but never cleared.
    pub name: Option<String>,
    pub email: Option<Option<String>>,
    pub billing_address: Option<Option<String>>,
    pub notes: Option<Option<String>>,
}

impl ClientUpdate {
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.email.is_none()
            && self.billing_address.is_none()
            && self.notes.is_none()
    }
}

/// Apply a partial update to a client, in its own transaction.
///
/// [`update_client_within`] is the same work inside the caller's, for a
/// surface that also replaces the contact list and wants both or neither.
pub fn update_client(conn: &Connection, id: i64, update: &ClientUpdate) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    update_client_within(&tx, id, update)?;
    tx.commit()?;
    Ok(())
}

pub fn update_client_within(conn: &Connection, id: i64, update: &ClientUpdate) -> Result<()> {
    if update.is_empty() {
        return Err(NigelError::Invalid(
            "Nothing to update — provide at least one flag".to_string(),
        ));
    }
    if let Some(ref name) = update.name {
        let name = name.trim();
        if name.is_empty() {
            return Err(NigelError::Invalid("Name is required".into()));
        }
        // Excluding this client, so a form that resends an unchanged name does
        // not collide with itself.
        if name_taken(conn, name, Some(id))? {
            return Err(NigelError::DuplicateName {
                kind: "Client",
                name: name.to_string(),
            });
        }
    }

    // Checked up front rather than read off the row count, because `email` is
    // no longer a column on `clients` and an email-only update touches none.
    ensure_client_exists(conn, id)?;

    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref name) = update.name {
        params.push(Box::new(name.trim().to_string()));
        updates.push(format!("name = ?{}", params.len()));
    }
    if let Some(ref address) = update.billing_address {
        params.push(Box::new(address.clone()));
        updates.push(format!("billing_address = ?{}", params.len()));
    }
    if let Some(ref notes) = update.notes {
        params.push(Box::new(notes.clone()));
        updates.push(format!("notes = ?{}", params.len()));
    }

    if !updates.is_empty() {
        params.push(Box::new(id));
        let sql = format!(
            "UPDATE clients SET {} WHERE id = ?{}",
            updates.join(", "),
            params.len()
        );
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, param_refs.as_slice())?;
    }
    if let Some(ref email) = update.email {
        set_billing_email(conn, id, email.as_deref())?;
    }
    Ok(())
}

/// Why this client cannot be deleted, or `None` when it can — the shape
/// `accounts::delete_blocker` and `categories::delete_blocker` return, so the
/// API answers all three with one mapping and the TUI prints one sentence.
///
/// Every status counts, including `void` and `paid`: the invoice names this
/// client on a page that has already been sent, and an invoice whose client row
/// is gone is a state the rest of the system only tolerates because nothing is
/// allowed to create it.
pub fn delete_blocker(conn: &Connection, id: i64) -> Result<Option<DeleteBlock>> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM invoices WHERE client_id = ?1",
        [id],
        |r| r.get(0),
    )?;
    if count > 0 {
        return Ok(Some(DeleteBlock::invoices("client", count)));
    }
    Ok(None)
}

pub fn delete_client(conn: &Connection, id: i64) -> Result<()> {
    if let Some(block) = delete_blocker(conn, id)? {
        return Err(NigelError::Blocked(block));
    }
    if conn.execute("DELETE FROM clients WHERE id = ?1", [id])? == 0 {
        return Err(NigelError::NotFound(format!("Client not found: id {id}")));
    }
    Ok(())
}

/// Every client in scope, by name.
///
/// The order is `name` in both scopes and an archived row does not sink to the
/// bottom: every surface marks them instead, because a list that re-sorts
/// itself depending on a filter feels unstable.
pub fn list_clients(conn: &Connection, scope: ClientScope) -> Result<Vec<Client>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {CLIENT_COLUMNS} FROM clients {} ORDER BY name",
        scope.where_clause()
    ))?;
    let rows = stmt
        .query_map([], client_from_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Take a client out of the working list without touching a single invoice.
///
/// Idempotent by `AND archived_at IS NULL`: archiving twice keeps the date the
/// client actually stopped being billed rather than the date somebody pressed
/// the key again. `on` is the caller's today — the data layer never reads the
/// clock, the way `void_invoice` does not.
pub fn archive_client(conn: &Connection, id: i64, on: &str) -> Result<()> {
    ensure_client_exists(conn, id)?;
    conn.execute(
        "UPDATE clients SET archived_at = ?2 WHERE id = ?1 AND archived_at IS NULL",
        rusqlite::params![id, on],
    )?;
    Ok(())
}

/// Bring an archived client back to the working list.
pub fn unarchive_client(conn: &Connection, id: i64) -> Result<()> {
    ensure_client_exists(conn, id)?;
    conn.execute("UPDATE clients SET archived_at = NULL WHERE id = ?1", [id])?;
    Ok(())
}

/// A client that is not archived, or the refusal a new invoice gets.
///
/// A `Conflict` rather than an `Invalid`, for `client_missing_email`'s reason:
/// it is a fact about the client record a screen can act on, so over HTTP it is
/// a 409 naming the client and carrying a reason a button can be built from.
pub fn ensure_client_active(conn: &Connection, id: i64) -> Result<()> {
    let client = get_client(conn, id)?;
    if client.archived_at.is_none() {
        return Ok(());
    }
    Err(NigelError::Conflict {
        code: "client_archived",
        message: format!(
            "client '{}' is archived — unarchive it before invoicing",
            client.name
        ),
    })
}

/// One row of a client's invoice history, for `client show`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInvoiceRow {
    pub number: i64,
    pub status: String,
    pub issue_date: String,
    pub due_date: Option<String>,
    pub total: f64,
    pub paid: f64,
}

/// A client plus everything `client show` prints, in one round trip.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientSummary {
    pub client: Client,
    /// Every address, billing first. On the summary rather than on `Client`,
    /// so the list stays one query and one row shape.
    pub contacts: Vec<ClientContact>,
    /// Newest invoice number first.
    pub invoices: Vec<ClientInvoiceRow>,
    /// Open invoices only, so a paid or voided one contributes nothing.
    pub outstanding: f64,
}

pub fn client_summary(conn: &Connection, id: i64) -> Result<ClientSummary> {
    let client = get_client(conn, id)?;
    let contacts = list_contacts(conn, id)?;

    let mut stmt = conn.prepare(
        "SELECT i.number, i.status, i.issue_date, i.due_date, i.total,
                COALESCE((SELECT SUM(p.amount) FROM invoice_payments p
                          WHERE p.invoice_id = i.id), 0)
         FROM invoices i WHERE i.client_id = ?1 ORDER BY i.number DESC",
    )?;
    let invoices = stmt
        .query_map([id], |r| {
            Ok(ClientInvoiceRow {
                number: r.get(0)?,
                status: r.get(1)?,
                issue_date: r.get(2)?,
                due_date: r.get(3)?,
                total: r.get(4)?,
                paid: r.get(5)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // The same open-status filter `ar_aging` uses, clamped per row so an
    // overpayment on one invoice cannot pay down another's balance.
    let outstanding = invoices
        .iter()
        .filter(|i| matches!(i.status.as_str(), "sent" | "partial" | "overdue"))
        .map(|i| (i.total - i.paid).max(0.0))
        .sum();

    Ok(ClientSummary {
        client,
        contacts,
        invoices,
        outstanding,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::error::NigelError;
    use crate::migrations::run_migrations;

    fn test_conn() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    #[test]
    fn unknown_client_id_is_not_found() {
        let (_d, conn) = test_conn();
        let err = get_client(&conn, 99).map(|_| ()).unwrap_err();
        assert!(matches!(err, NigelError::NotFound(_)), "got: {err:?}");
        assert_eq!(err.to_string(), "Client not found: id 99");
    }

    #[test]
    fn ensure_client_exists_passes_for_a_real_client_and_fails_otherwise() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", None, None, None).unwrap();
        assert!(ensure_client_exists(&conn, id).is_ok());

        let err = ensure_client_exists(&conn, 99).unwrap_err();
        assert!(matches!(err, NigelError::NotFound(_)), "got: {err:?}");
        assert_eq!(err.to_string(), "Client not found: id 99");
    }

    #[test]
    fn add_and_get_client() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", Some("ap@acme.test"), None, None).unwrap();
        let c = get_client(&conn, id).unwrap();
        assert_eq!(c.name, "Acme Co");
        assert_eq!(c.email.as_deref(), Some("ap@acme.test"));
        assert_eq!(list_clients(&conn, ClientScope::Active).unwrap().len(), 1);
    }

    fn seed_client(conn: &Connection) -> i64 {
        add_client(
            conn,
            "Acme Co",
            Some("ap@acme.test"),
            Some("123 Main St"),
            Some("pays late"),
        )
        .unwrap()
    }

    #[test]
    fn updating_one_field_leaves_the_others_alone() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);

        update_client(
            &conn,
            id,
            &ClientUpdate {
                email: Some(Some("billing@acme.test".into())),
                ..Default::default()
            },
        )
        .unwrap();

        let c = get_client(&conn, id).unwrap();
        assert_eq!(c.email.as_deref(), Some("billing@acme.test"));
        assert_eq!(c.name, "Acme Co");
        assert_eq!(c.billing_address.as_deref(), Some("123 Main St"));
        assert_eq!(c.notes.as_deref(), Some("pays late"));
    }

    #[test]
    fn some_none_clears_a_nullable_field() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);

        update_client(
            &conn,
            id,
            &ClientUpdate {
                email: Some(None),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(get_client(&conn, id).unwrap().email, None);
    }

    #[test]
    fn an_empty_client_update_is_rejected() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);

        let err = update_client(&conn, id, &ClientUpdate::default()).unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
        assert_eq!(
            err.to_string(),
            "Nothing to update — provide at least one flag"
        );
    }

    #[test]
    fn a_blank_client_name_is_rejected() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);

        let err = update_client(
            &conn,
            id,
            &ClientUpdate {
                name: Some("   ".into()),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
        assert_eq!(err.to_string(), "Name is required");
        assert_eq!(get_client(&conn, id).unwrap().name, "Acme Co");
    }

    /// One invoice for `client_id` at `total`, left as a draft.
    fn seed_invoice(conn: &Connection, client_id: i64, issue_date: &str, total: f64) -> i64 {
        let items = vec![crate::invoicing::invoices::NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: total,
        }];
        crate::invoicing::invoices::create_invoice(
            conn, client_id, issue_date, None, "USD", &items, None, None,
        )
        .unwrap()
    }

    fn publish(conn: &Connection, invoice_id: i64, on: &str) {
        crate::invoicing::invoices::mark_published(conn, invoice_id, on).unwrap();
    }

    #[test]
    fn summary_lists_a_clients_invoices_newest_first() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);
        seed_invoice(&conn, id, "2026-06-01", 100.0);
        seed_invoice(&conn, id, "2026-07-01", 200.0);
        seed_invoice(&conn, id, "2026-08-01", 300.0);

        let summary = client_summary(&conn, id).unwrap();
        let numbers: Vec<i64> = summary.invoices.iter().map(|i| i.number).collect();
        assert_eq!(numbers, vec![1250, 1249, 1248]);
        assert_eq!(summary.client.name, "Acme Co");
    }

    #[test]
    fn summary_outstanding_counts_only_open_invoices() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);

        let open = seed_invoice(&conn, id, "2026-06-01", 100.0);
        publish(&conn, open, "2026-06-01");
        crate::invoicing::invoices::record_payment(&conn, open, 30.0, "2026-06-10", "ach", None)
            .unwrap();

        let settled = seed_invoice(&conn, id, "2026-07-01", 200.0);
        publish(&conn, settled, "2026-07-01");
        crate::invoicing::invoices::record_payment(
            &conn,
            settled,
            200.0,
            "2026-07-10",
            "ach",
            None,
        )
        .unwrap();

        let cancelled = seed_invoice(&conn, id, "2026-08-01", 500.0);
        publish(&conn, cancelled, "2026-08-01");
        conn.execute(
            "UPDATE invoices SET voided_at = '2026-08-02' WHERE id = ?1",
            [cancelled],
        )
        .unwrap();
        crate::invoicing::invoices::refresh_status(&conn, cancelled, "2026-08-02").unwrap();

        let summary = client_summary(&conn, id).unwrap();
        assert_eq!(summary.outstanding, 70.0);
        assert_eq!(summary.invoices.len(), 3);
    }

    #[test]
    fn summary_for_a_client_with_no_invoices_is_empty_not_an_error() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);

        let summary = client_summary(&conn, id).unwrap();
        assert!(summary.invoices.is_empty());
        assert_eq!(summary.outstanding, 0.0);
    }

    #[test]
    fn summary_for_a_missing_client_is_not_found() {
        let (_d, conn) = test_conn();
        let err = client_summary(&conn, 99).map(|_| ()).unwrap_err();
        assert!(matches!(err, NigelError::NotFound(_)), "got: {err:?}");
        assert_eq!(err.to_string(), "Client not found: id 99");
    }

    #[test]
    fn a_client_with_no_invoices_can_be_deleted() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);

        assert!(delete_blocker(&conn, id).unwrap().is_none());
        delete_client(&conn, id).unwrap();
        assert!(list_clients(&conn, ClientScope::Active).unwrap().is_empty());
    }

    #[test]
    fn deleting_a_client_with_invoices_is_blocked_with_the_count() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);
        seed_invoice(&conn, id, "2026-06-01", 100.0);
        seed_invoice(&conn, id, "2026-07-01", 200.0);

        let block = delete_blocker(&conn, id).unwrap().expect("blocked");
        assert_eq!(block.reason_code(), "has_invoices");
        assert_eq!(block.count(), Some(2));

        let err = delete_client(&conn, id).unwrap_err();
        assert!(matches!(err, NigelError::Blocked(_)), "got: {err:?}");
        assert_eq!(err.to_string(), "Cannot delete: client has 2 invoices");
        // Refused means refused: the client is still there.
        assert_eq!(list_clients(&conn, ClientScope::Active).unwrap().len(), 1);
    }

    /// Every status counts, not just the open ones. A void or settled invoice
    /// still names its client on the page that was sent out.
    #[test]
    fn a_void_or_paid_invoice_still_blocks_the_delete() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);

        let settled = seed_invoice(&conn, id, "2026-06-01", 100.0);
        publish(&conn, settled, "2026-06-01");
        crate::invoicing::invoices::record_payment(
            &conn,
            settled,
            100.0,
            "2026-06-10",
            "ach",
            None,
        )
        .unwrap();

        let cancelled = seed_invoice(&conn, id, "2026-07-01", 500.0);
        crate::invoicing::invoices::void_invoice(&conn, cancelled, "2026-07-02").unwrap();

        let block = delete_blocker(&conn, id).unwrap().expect("blocked");
        assert_eq!(block.count(), Some(2));
        assert!(matches!(
            delete_client(&conn, id).unwrap_err(),
            NigelError::Blocked(_)
        ));
    }

    #[test]
    fn deleting_a_missing_client_is_not_found() {
        let (_d, conn) = test_conn();
        let err = delete_client(&conn, 99).unwrap_err();
        assert!(matches!(err, NigelError::NotFound(_)), "got: {err:?}");
        assert_eq!(err.to_string(), "Client not found: id 99");
    }

    #[test]
    fn a_duplicate_client_name_is_refused() {
        let (_d, conn) = test_conn();
        add_client(&conn, "Acme Co", None, None, None).unwrap();

        let err = add_client(&conn, "Acme Co", None, None, None).unwrap_err();
        assert!(
            matches!(err, NigelError::DuplicateName { kind: "Client", .. }),
            "got: {err:?}"
        );
        assert_eq!(err.to_string(), "Client name already exists: Acme Co");
        assert_eq!(list_clients(&conn, ClientScope::Active).unwrap().len(), 1);
    }

    #[test]
    fn an_empty_client_name_is_refused() {
        let (_d, conn) = test_conn();
        let err = add_client(&conn, "   ", None, None, None).unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
        assert_eq!(err.to_string(), "Name is required");
    }

    #[test]
    fn renaming_onto_another_clients_name_is_refused_but_a_no_op_rename_is_not() {
        let (_d, conn) = test_conn();
        let acme = add_client(&conn, "Acme Co", None, None, None).unwrap();
        let globex = add_client(&conn, "Globex", None, None, None).unwrap();

        let err = update_client(
            &conn,
            globex,
            &ClientUpdate {
                name: Some("Acme Co".into()),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(
            matches!(err, NigelError::DuplicateName { kind: "Client", .. }),
            "got: {err:?}"
        );

        // The client manager sends every field on every edit, so a name that
        // has not changed must not collide with itself.
        update_client(
            &conn,
            acme,
            &ClientUpdate {
                name: Some("Acme Co".into()),
                email: Some(Some("ap@acme.test".into())),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            get_client(&conn, acme).unwrap().email.as_deref(),
            Some("ap@acme.test")
        );
    }

    #[test]
    fn the_default_scope_hides_archived_clients_and_all_shows_them() {
        let (_d, conn) = test_conn();
        let acme = add_client(&conn, "Acme Co", None, None, None).unwrap();
        add_client(&conn, "Globex", None, None, None).unwrap();
        archive_client(&conn, acme, "2026-08-11").unwrap();

        let active = list_clients(&conn, ClientScope::Active).unwrap();
        assert_eq!(
            active.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            vec!["Globex"]
        );

        let all = list_clients(&conn, ClientScope::All).unwrap();
        assert_eq!(all.len(), 2);
        // Order is by name in both scopes: an archived row does not move.
        assert_eq!(all[0].name, "Acme Co");
        assert_eq!(all[0].archived_at.as_deref(), Some("2026-08-11"));
        assert!(all[1].archived_at.is_none());
    }

    /// Archive is not a soft delete: nothing stops reading the row.
    #[test]
    fn get_client_answers_an_archived_client_normally() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);
        archive_client(&conn, id, "2026-08-11").unwrap();

        let c = get_client(&conn, id).unwrap();
        assert_eq!(c.name, "Acme Co");
        assert_eq!(c.archived_at.as_deref(), Some("2026-08-11"));
    }

    #[test]
    fn archiving_is_idempotent_and_keeps_the_first_timestamp() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);
        archive_client(&conn, id, "2026-08-11").unwrap();
        archive_client(&conn, id, "2026-09-01").unwrap();

        assert_eq!(
            get_client(&conn, id).unwrap().archived_at.as_deref(),
            Some("2026-08-11")
        );
    }

    #[test]
    fn unarchiving_clears_the_timestamp() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);
        archive_client(&conn, id, "2026-08-11").unwrap();
        unarchive_client(&conn, id).unwrap();

        assert_eq!(get_client(&conn, id).unwrap().archived_at, None);
        assert_eq!(list_clients(&conn, ClientScope::Active).unwrap().len(), 1);
    }

    /// AC #4 in one test: archiving is the one `UPDATE`, and nothing else moves.
    #[test]
    fn archiving_touches_nothing_but_the_flag() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);
        let open = seed_invoice(&conn, id, "2026-06-01", 100.0);
        publish(&conn, open, "2026-06-01");
        crate::invoicing::invoices::record_payment(&conn, open, 30.0, "2026-06-10", "ach", None)
            .unwrap();
        seed_invoice(&conn, id, "2026-07-01", 200.0);

        let before = client_summary(&conn, id).unwrap();
        archive_client(&conn, id, "2026-08-11").unwrap();
        let after = client_summary(&conn, id).unwrap();

        assert_eq!(after.outstanding, before.outstanding);
        assert_eq!(after.invoices.len(), before.invoices.len());
        assert_eq!(
            crate::invoicing::invoices::list_invoices(&conn, None, None)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            delete_blocker(&conn, id).unwrap().expect("blocked").count(),
            Some(2)
        );
    }

    #[test]
    fn archiving_a_missing_client_is_not_found() {
        let (_d, conn) = test_conn();
        assert_eq!(
            archive_client(&conn, 99, "2026-08-11")
                .unwrap_err()
                .to_string(),
            "Client not found: id 99"
        );
        assert_eq!(
            unarchive_client(&conn, 99).unwrap_err().to_string(),
            "Client not found: id 99"
        );
    }

    #[test]
    fn ensure_client_active_refuses_an_archived_client_by_name() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);
        assert!(ensure_client_active(&conn, id).is_ok());

        archive_client(&conn, id, "2026-08-11").unwrap();
        let err = ensure_client_active(&conn, id).unwrap_err();
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
        assert!(err.to_string().contains("Acme Co"), "got: {err}");
    }

    fn contact(email: &str) -> NewContact {
        NewContact {
            email: email.into(),
            ..Default::default()
        }
    }

    fn billing(email: &str) -> NewContact {
        NewContact {
            email: email.into(),
            is_billing: true,
            ..Default::default()
        }
    }

    /// The whole point of keeping `Client.email` as a projection.
    #[test]
    fn add_client_still_stores_and_answers_one_email() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", Some("ap@acme.test"), None, None).unwrap();

        assert_eq!(
            get_client(&conn, id).unwrap().email.as_deref(),
            Some("ap@acme.test")
        );
        let contacts = list_contacts(&conn, id).unwrap();
        assert_eq!(contacts.len(), 1);
        assert!(contacts[0].is_billing);
        assert_eq!(contacts[0].position, 0);
    }

    #[test]
    fn a_client_added_with_no_email_has_no_contacts_at_all() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Globex", None, None, None).unwrap();
        assert_eq!(get_client(&conn, id).unwrap().email, None);
        assert!(list_contacts(&conn, id).unwrap().is_empty());
    }

    #[test]
    fn the_projected_email_is_the_billing_contacts_address() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", Some("ap@acme.test"), None, None).unwrap();
        set_contacts(
            &conn,
            id,
            &[
                NewContact {
                    email: "dana@acme.test".into(),
                    name: Some("Dana".into()),
                    is_billing: true,
                    ..Default::default()
                },
                contact("ap@acme.test"),
            ],
        )
        .unwrap();

        assert_eq!(
            get_client(&conn, id).unwrap().email.as_deref(),
            Some("dana@acme.test")
        );
    }

    #[test]
    fn setting_the_email_upserts_the_billing_contact_and_leaves_the_cc_rows_alone() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", None, None, None).unwrap();
        set_contacts(
            &conn,
            id,
            &[billing("ap@acme.test"), contact("dana@acme.test")],
        )
        .unwrap();

        update_client(
            &conn,
            id,
            &ClientUpdate {
                email: Some(Some("new@acme.test".into())),
                ..Default::default()
            },
        )
        .unwrap();

        let contacts = list_contacts(&conn, id).unwrap();
        assert_eq!(contacts.len(), 2);
        assert_eq!(contacts[0].email, "new@acme.test");
        assert!(contacts[0].is_billing);
        assert_eq!(
            contacts[1].email, "dana@acme.test",
            "the cc row is untouched"
        );
    }

    #[test]
    fn clearing_the_email_promotes_the_next_contact() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", None, None, None).unwrap();
        set_contacts(
            &conn,
            id,
            &[billing("ap@acme.test"), contact("dana@acme.test")],
        )
        .unwrap();

        update_client(
            &conn,
            id,
            &ClientUpdate {
                email: Some(None),
                ..Default::default()
            },
        )
        .unwrap();

        // A client with contacts but no billing recipient is not representable.
        assert_eq!(
            get_client(&conn, id).unwrap().email.as_deref(),
            Some("dana@acme.test")
        );
        let contacts = list_contacts(&conn, id).unwrap();
        assert_eq!(contacts.len(), 1);
        assert!(contacts[0].is_billing);
    }

    #[test]
    fn clearing_the_email_of_a_single_contact_client_leaves_no_contacts() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", Some("ap@acme.test"), None, None).unwrap();

        update_client(
            &conn,
            id,
            &ClientUpdate {
                email: Some(None),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(get_client(&conn, id).unwrap().email, None);
        assert!(list_contacts(&conn, id).unwrap().is_empty());
    }

    /// Setting the billing address to one the client already has as a cc moves
    /// the flag rather than colliding with the unique index.
    #[test]
    fn setting_the_email_to_an_existing_cc_promotes_it() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", None, None, None).unwrap();
        set_contacts(
            &conn,
            id,
            &[billing("ap@acme.test"), contact("dana@acme.test")],
        )
        .unwrap();

        update_client(
            &conn,
            id,
            &ClientUpdate {
                email: Some(Some("DANA@acme.test".into())),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(
            get_client(&conn, id).unwrap().email.as_deref(),
            Some("dana@acme.test")
        );
        assert_eq!(list_contacts(&conn, id).unwrap().len(), 2);
    }

    #[test]
    fn a_list_with_no_billing_flag_makes_the_first_row_billing() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", None, None, None).unwrap();
        set_contacts(&conn, id, &[contact("a@x.test"), contact("b@x.test")]).unwrap();

        let contacts = list_contacts(&conn, id).unwrap();
        assert_eq!(contacts[0].email, "a@x.test");
        assert!(contacts[0].is_billing);
        assert!(!contacts[1].is_billing);
    }

    #[test]
    fn a_list_with_two_billing_flags_is_refused_before_anything_is_deleted() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", Some("ap@acme.test"), None, None).unwrap();

        let err = set_contacts(&conn, id, &[billing("a@x.test"), billing("b@x.test")]).unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
        assert_eq!(
            list_contacts(&conn, id).unwrap().len(),
            1,
            "validation runs before the delete"
        );
    }

    #[test]
    fn a_blank_or_duplicate_address_is_refused() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", None, None, None).unwrap();

        for bad in [vec![contact("")], vec![contact("   ")]] {
            let err = set_contacts(&conn, id, &bad).unwrap_err();
            assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
        }

        let err = set_contacts(&conn, id, &[contact("A@x.test"), contact("a@x.test")]).unwrap_err();
        assert!(err.to_string().contains("twice"), "got: {err}");
    }

    /// These strings become mail headers.
    #[test]
    fn a_contact_carrying_a_line_break_is_refused() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", None, None, None).unwrap();

        let err = set_contacts(
            &conn,
            id,
            &[NewContact {
                email: "a@x.test".into(),
                name: Some("Ada\r\nBcc: x@y.test".into()),
                ..Default::default()
            }],
        )
        .unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
        assert!(list_contacts(&conn, id).unwrap().is_empty());
    }

    #[test]
    fn set_contacts_replaces_the_whole_list_in_one_transaction() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", None, None, None).unwrap();
        set_contacts(
            &conn,
            id,
            &[
                contact("a@x.test"),
                contact("b@x.test"),
                contact("c@x.test"),
            ],
        )
        .unwrap();

        set_contacts(&conn, id, &[contact("d@x.test"), contact("e@x.test")]).unwrap();

        let contacts = list_contacts(&conn, id).unwrap();
        assert_eq!(
            contacts
                .iter()
                .map(|c| c.email.as_str())
                .collect::<Vec<_>>(),
            vec!["d@x.test", "e@x.test"]
        );
        assert_eq!(contacts[0].position, 0);
        assert_eq!(contacts[1].position, 1);
    }

    #[test]
    fn an_empty_contact_list_clears_every_address() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", Some("ap@acme.test"), None, None).unwrap();
        set_contacts(&conn, id, &[]).unwrap();
        assert!(list_contacts(&conn, id).unwrap().is_empty());
        assert_eq!(get_client(&conn, id).unwrap().email, None);
    }

    #[test]
    fn client_summary_carries_the_contacts() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", None, None, None).unwrap();
        set_contacts(&conn, id, &[contact("a@x.test"), billing("b@x.test")]).unwrap();

        let summary = client_summary(&conn, id).unwrap();
        assert_eq!(summary.contacts.len(), 2);
        assert_eq!(summary.contacts[0].email, "b@x.test", "billing first");
        assert!(summary.contacts[0].is_billing);
    }

    #[test]
    fn list_clients_answers_every_billing_email_in_one_query() {
        let (_d, conn) = test_conn();
        for (name, email) in [
            ("Acme Co", Some("ap@acme.test")),
            ("Globex", None),
            ("Northwind", Some("billing@nw.test")),
        ] {
            add_client(&conn, name, email, None, None).unwrap();
        }

        let rows = list_clients(&conn, ClientScope::Active).unwrap();
        assert_eq!(
            rows.iter().map(|c| c.email.as_deref()).collect::<Vec<_>>(),
            vec![Some("ap@acme.test"), None, Some("billing@nw.test")]
        );
    }

    #[test]
    fn deleting_a_client_takes_its_contacts_with_it() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", Some("ap@acme.test"), None, None).unwrap();
        delete_client(&conn, id).unwrap();

        let left: i64 = conn
            .query_row("SELECT COUNT(*) FROM client_contacts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0, "the FK cascade does it; no new statement needed");
    }

    #[test]
    fn setting_contacts_on_a_missing_client_is_not_found() {
        let (_d, conn) = test_conn();
        let err = set_contacts(&conn, 99, &[contact("a@x.test")]).unwrap_err();
        assert_eq!(err.to_string(), "Client not found: id 99");
    }

    #[test]
    fn updating_a_missing_client_is_not_found() {
        let (_d, conn) = test_conn();
        let err = update_client(
            &conn,
            99,
            &ClientUpdate {
                name: Some("Ghost".into()),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(matches!(err, NigelError::NotFound(_)), "got: {err:?}");
        assert_eq!(err.to_string(), "Client not found: id 99");
    }
}
