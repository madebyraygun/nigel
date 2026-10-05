use std::path::Path;

use comfy_table::{Cell, Table};

use nigel_core::db::get_connection;
use nigel_core::documents::kinds::{add_kind, deactivate_kind, list_kinds, rename_kind};
use nigel_core::documents::model::DocumentKind;
use nigel_core::documents::store::{file_document, get_document, latest_version, NewDocument};
use nigel_core::error::{NigelError, Result};
use nigel_core::settings::get_data_dir;

use crate::cli::DocumentKindsCommands;

pub fn add(client: i64, kind: &str, title: &str, file: &Path, today: &str) -> Result<()> {
    let pdf = std::fs::read(file)
        .map_err(|e| NigelError::Invalid(format!("Could not read {}: {e}", file.display())))?;
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let new = NewDocument {
        client_id: client,
        kind,
        title,
    };
    let id = file_document(&conn, &get_data_dir(), &new, &pdf, today)?;
    let document = get_document(&conn, id)?;
    let version = latest_version(&conn, id)?;
    println!(
        "Filed document #{id}: {} ({}, draft, {})",
        document.title, document.kind, version.checksum
    );
    Ok(())
}

pub fn kinds(command: Option<DocumentKindsCommands>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    match command.unwrap_or(DocumentKindsCommands::List) {
        DocumentKindsCommands::List => {
            println!("{}", format_kind_list(&list_kinds(&conn, true)?));
        }
        DocumentKindsCommands::Add { name } => {
            let id = add_kind(&conn, &name)?;
            println!("Added document kind #{id}: {}", name.trim());
        }
        DocumentKindsCommands::Rename { id, name } => {
            rename_kind(&conn, id, &name)?;
            println!("Renamed document kind #{id} to {}", name.trim());
        }
        DocumentKindsCommands::Deactivate { id } => {
            deactivate_kind(&conn, id)?;
            println!("Deactivated document kind #{id}");
        }
    }
    Ok(())
}

pub fn format_kind_list(kinds: &[DocumentKind]) -> String {
    let mut table = Table::new();
    table.set_header(vec!["ID", "Name", "State"]);
    for kind in kinds {
        table.add_row(vec![
            Cell::new(kind.id),
            Cell::new(&kind.name),
            Cell::new(if kind.active { "active" } else { "inactive" }),
        ]);
    }
    table.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kind_list_marks_inactive_rows() {
        let kinds = vec![
            DocumentKind {
                id: 1,
                name: "Proposal".into(),
                active: true,
                position: 0,
            },
            DocumentKind {
                id: 4,
                name: "SOW".into(),
                active: false,
                position: 3,
            },
        ];
        let out = format_kind_list(&kinds);
        assert!(out.contains("Proposal") && out.contains("SOW") && out.contains("inactive"));
    }
}
