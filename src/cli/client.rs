use comfy_table::{Cell, Table};

use crate::db::get_connection;
use crate::error::{NigelError, Result};
use crate::invoicing::clients::{
    add_client_within, archive_client, client_summary, delete_blocker, delete_client, get_client,
    list_clients, set_billing_email, set_contacts_within, unarchive_client, update_client_within,
    ClientScope, ClientUpdate, NewContact,
};
use crate::models::Client;
use crate::settings::get_data_dir;

/// `--contact "email[:name[:title]]"`, the shape `--item "desc:qty:unit"` set.
///
/// `splitn(3, ':')` gives the last field the remainder, so a title containing a
/// colon survives. Each part is trimmed and a blank one is `None`.
pub(crate) fn parse_contact(spec: &str) -> Result<NewContact> {
    let mut parts = spec.splitn(3, ':');
    let email = parts.next().unwrap_or("").trim();
    if email.is_empty() {
        return Err(NigelError::Other(
            "bad --contact, want \"email[:name[:title]]\"".into(),
        ));
    }
    let optional = |value: Option<&str>| {
        value
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };
    Ok(NewContact {
        email: email.to_string(),
        name: optional(parts.next()),
        title: optional(parts.next()),
        is_billing: false,
    })
}

/// The whole list a `--contact` run replaces the client's addresses with. The
/// first spec is the billing recipient, which is what makes the order on the
/// command line mean something.
pub(crate) fn parse_contacts(specs: &[String]) -> Result<Vec<NewContact>> {
    let mut contacts: Vec<NewContact> = specs
        .iter()
        .map(|spec| parse_contact(spec))
        .collect::<Result<_>>()?;
    if let Some(first) = contacts.first_mut() {
        first.is_billing = true;
    }
    Ok(contacts)
}

pub fn add(
    name: &str,
    email: Option<&str>,
    address: Option<&str>,
    contacts: &[String],
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let parsed = parse_contacts(contacts)?;

    // One transaction over the row and its addresses: a refused contact list
    // must not leave a client behind that nobody asked for.
    let tx = conn.unchecked_transaction()?;
    let id = add_client_within(&tx, name, address, None)?;
    if parsed.is_empty() {
        set_billing_email(&tx, id, email)?;
    } else {
        set_contacts_within(&tx, id, &parsed)?;
    }
    tx.commit()?;

    println!("Added client {id}: {name}");
    Ok(())
}

pub fn show(id: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let summary = client_summary(&conn, id)?;
    let client = &summary.client;

    println!("Client #{}  {}", client.id, client.name);
    println!("Email:    {}", client.email.as_deref().unwrap_or("-"));
    println!(
        "Address:  {}",
        client.billing_address.as_deref().unwrap_or("-")
    );
    println!("Notes:    {}", client.notes.as_deref().unwrap_or("-"));
    if let Some(on) = &client.archived_at {
        println!("Archived: {on}");
    }

    if !summary.contacts.is_empty() {
        let mut table = Table::new();
        table.set_header(vec!["Email", "Name", "Title", ""]);
        for contact in &summary.contacts {
            table.add_row(vec![
                Cell::new(&contact.email),
                Cell::new(contact.name.as_deref().unwrap_or("-")),
                Cell::new(contact.title.as_deref().unwrap_or("-")),
                Cell::new(if contact.is_billing { "billing" } else { "" }),
            ]);
        }
        println!("{table}");
    }

    if summary.invoices.is_empty() {
        println!("No invoices.");
        return Ok(());
    }

    let mut table = Table::new();
    table.set_header(vec!["#", "Status", "Issued", "Total", "Paid"]);
    for row in &summary.invoices {
        table.add_row(vec![
            Cell::new(row.number),
            Cell::new(&row.status),
            Cell::new(&row.issue_date),
            Cell::new(format!("{:.2}", row.total)),
            Cell::new(format!("{:.2}", row.paid)),
        ]);
    }
    println!("{table}");
    println!("Outstanding: {:.2}", summary.outstanding);
    Ok(())
}

pub fn edit(
    id: i64,
    name: Option<String>,
    email: Option<String>,
    address: Option<String>,
    notes: Option<String>,
    contacts: &[String],
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let update = ClientUpdate {
        name,
        email: email.map(Some),
        billing_address: address.map(Some),
        notes: notes.map(Some),
    };
    // `--contact` alone is a legitimate edit, so the empty-update refusal has
    // to account for it — but an edit naming nothing at all is still an error
    // rather than a silent no-op.
    if update.is_empty() && contacts.is_empty() {
        return Err(NigelError::Invalid(
            "Nothing to update — provide at least one flag".to_string(),
        ));
    }
    let parsed = parse_contacts(contacts)?;

    // One transaction: a refused contact list leaves the rename unapplied too,
    // rather than half an edit nobody can see the shape of.
    let tx = conn.unchecked_transaction()?;
    if !update.is_empty() {
        update_client_within(&tx, id, &update)?;
    }
    if !parsed.is_empty() {
        set_contacts_within(&tx, id, &parsed)?;
    }
    tx.commit()?;

    let client = get_client(&conn, id)?;
    println!("Updated client {id}: {}", client.name);
    Ok(())
}

/// `nigel client list`, as text. Pure, so the parity fixtures can call it
/// without a terminal — the same shape `cli/report/text.rs` uses.
///
/// The Archived column appears only when the slice carries an archived client,
/// so the default list prints exactly the three columns it always has.
pub fn format_client_list(clients: &[Client]) -> String {
    let show_archived = clients.iter().any(|c| c.archived_at.is_some());

    let mut table = Table::new();
    let mut header = vec!["ID", "Name", "Email"];
    if show_archived {
        header.push("Archived");
    }
    table.set_header(header);
    for c in clients {
        let mut row = vec![
            Cell::new(c.id),
            Cell::new(&c.name),
            // A client with no email reads as an em dash, never an empty cell —
            // the missing address is the reason a send will refuse.
            Cell::new(c.email.as_deref().unwrap_or("\u{2014}")),
        ];
        if show_archived {
            row.push(Cell::new(c.archived_at.as_deref().unwrap_or("\u{2014}")));
        }
        table.add_row(row);
    }
    format!("Clients\n{table}")
}

pub fn list(all: bool) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let scope = if all {
        ClientScope::All
    } else {
        ClientScope::Active
    };
    println!("{}", format_client_list(&list_clients(&conn, scope)?));
    Ok(())
}

pub fn delete(id: i64, yes: bool) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let client = get_client(&conn, id)?;

    // Asked before the prompt: a client that cannot be deleted is never offered
    // a confirmation, because there is nothing to confirm.
    //
    // Returned rather than printed, so `main` writes it once — the block's own
    // sentence, with the pointer on the line below it.
    if let Some(block) = delete_blocker(&conn, id)? {
        return Err(NigelError::Other(format!(
            "{}\nRun `nigel client show {id}` to see them.",
            NigelError::Blocked(block)
        )));
    }

    println!(
        "Delete client #{id} {}? This cannot be undone.",
        client.name
    );
    if !crate::cli::confirm_or_refuse(
        "Delete it? [y/N]",
        &format!("Refusing to delete client #{id} without confirmation. Pass --yes."),
        yes,
    )? {
        println!("Aborted.");
        return Ok(());
    }
    delete_client(&conn, id)?;
    println!("Deleted client {id}: {}", client.name);
    Ok(())
}

pub fn archive(id: i64, today: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    archive_client(&conn, id, today)?;
    let client = get_client(&conn, id)?;
    println!("Archived client {id}: {}", client.name);
    Ok(())
}

pub fn unarchive(id: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    unarchive_client(&conn, id)?;
    let client = get_client(&conn, id)?;
    println!("Restored client {id}: {}", client.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(id: i64, name: &str, email: Option<&str>) -> Client {
        Client {
            id,
            name: name.into(),
            email: email.map(str::to_string),
            billing_address: None,
            notes: None,
            archived_at: None,
        }
    }

    fn archived(id: i64, name: &str, on: &str) -> Client {
        Client {
            archived_at: Some(on.into()),
            ..client(id, name, None)
        }
    }

    #[test]
    fn a_contact_spec_parses_email_name_and_title() {
        assert_eq!(
            parse_contact("ap@acme.test").unwrap(),
            NewContact {
                email: "ap@acme.test".into(),
                ..Default::default()
            }
        );
        let c = parse_contact("ap@acme.test:Ada Payne:AP Manager").unwrap();
        assert_eq!(
            (c.email.as_str(), c.name.as_deref(), c.title.as_deref()),
            ("ap@acme.test", Some("Ada Payne"), Some("AP Manager"))
        );
    }

    #[test]
    fn a_contact_spec_with_a_colon_in_the_title_keeps_the_remainder() {
        let c = parse_contact("a@x.test:Ada:Head: Billing").unwrap();
        assert_eq!(c.title.as_deref(), Some("Head: Billing"));
    }

    #[test]
    fn an_empty_contact_spec_is_refused_with_the_flag_in_the_message() {
        for bad in ["", "   ", ":Ada:AP"] {
            let err = parse_contact(bad).map(|_| ()).unwrap_err().to_string();
            assert!(err.contains("--contact"), "{bad:?} got: {err}");
        }
    }

    #[test]
    fn the_first_contact_is_the_billing_recipient() {
        let contacts =
            parse_contacts(&["ap@acme.test:Ada".to_string(), "dana@acme.test".to_string()])
                .unwrap();
        assert!(contacts[0].is_billing);
        assert!(!contacts[1].is_billing);
    }

    /// Byte-for-byte what `nigel client list` prints.
    #[test]
    fn format_client_list_prints_the_columns_it_always_has() {
        let out = format_client_list(&[
            client(1, "Acme Co", Some("ap@acme.test")),
            client(2, "Globex", None),
        ]);
        assert_eq!(
            out,
            concat!(
                "Clients\n",
                "+----+---------+--------------+\n",
                "| ID | Name    | Email        |\n",
                "+=============================+\n",
                "| 1  | Acme Co | ap@acme.test |\n",
                "|----+---------+--------------|\n",
                "| 2  | Globex  | \u{2014}            |\n",
                "+----+---------+--------------+",
            )
        );
    }

    #[test]
    fn format_client_list_grows_an_archived_column_when_a_row_is_archived() {
        let out = format_client_list(&[
            client(1, "Acme Co", Some("ap@acme.test")),
            archived(2, "Globex", "2026-08-11"),
        ]);
        assert!(out.contains("Archived"), "got:\n{out}");
        assert!(out.contains("2026-08-11"), "got:\n{out}");
    }

    #[test]
    fn format_client_list_prints_an_em_dash_for_a_client_with_no_email() {
        let out = format_client_list(&[client(2, "Globex", None)]);
        assert!(out.contains('\u{2014}'), "want an em dash, got:\n{out}");

        let out = format_client_list(&[client(1, "Acme Co", Some("ap@acme.test"))]);
        assert!(
            !out.contains('\u{2014}'),
            "a client with an email gets no dash, got:\n{out}"
        );
    }

    #[test]
    fn format_client_list_with_no_clients_is_the_bare_heading_and_header() {
        let out = format_client_list(&[]);
        assert!(out.starts_with("Clients\n"), "got: {out}");
        assert!(out.contains("Email"), "got: {out}");
    }
}
