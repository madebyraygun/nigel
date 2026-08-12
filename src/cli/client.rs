use comfy_table::{Cell, Table};

use crate::db::get_connection;
use crate::error::{NigelError, Result};
use crate::invoicing::clients::{
    add_client, archive_client, client_summary, delete_blocker, delete_client, get_client,
    list_clients, unarchive_client, update_client, ClientScope, ClientUpdate,
};
use crate::models::Client;
use crate::settings::get_data_dir;

pub fn add(name: &str, email: Option<&str>, address: Option<&str>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let id = add_client(&conn, name, email, address, None)?;
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
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let update = ClientUpdate {
        name,
        email: email.map(Some),
        billing_address: address.map(Some),
        notes: notes.map(Some),
    };
    update_client(&conn, id, &update)?;
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
