use rusqlite::{params, Connection};

use crate::documents::model::DocumentKind;
use crate::error::{NigelError, Result};

fn kind_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DocumentKind> {
    Ok(DocumentKind {
        id: row.get(0)?,
        name: row.get(1)?,
        active: row.get::<_, i64>(2)? != 0,
        position: row.get(3)?,
    })
}

fn clean_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(NigelError::Invalid("A document kind needs a name.".into()));
    }
    Ok(name.to_string())
}

fn name_taken(conn: &Connection, name: &str, except: Option<i64>) -> Result<bool> {
    let taken: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM document_kinds
          WHERE name = ?1 COLLATE NOCASE AND id IS NOT ?2)",
        params![name, except],
        |row| row.get(0),
    )?;
    Ok(taken)
}

fn duplicate(name: String) -> NigelError {
    NigelError::DuplicateName {
        kind: "Document kind",
        name,
    }
}

fn not_found(id: i64) -> NigelError {
    NigelError::NotFound(format!("Document kind not found: id {id}"))
}

pub fn list_kinds(conn: &Connection, include_inactive: bool) -> Result<Vec<DocumentKind>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, active, position FROM document_kinds
          WHERE (?1 OR active = 1) ORDER BY position, id",
    )?;
    let kinds = stmt
        .query_map(params![include_inactive], kind_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(kinds)
}

pub fn add_kind(conn: &Connection, name: &str) -> Result<i64> {
    let name = clean_name(name)?;
    if name_taken(conn, &name, None)? {
        return Err(duplicate(name));
    }
    conn.execute(
        "INSERT INTO document_kinds (name, position)
         VALUES (?1, (SELECT COALESCE(MAX(position) + 1, 0) FROM document_kinds))",
        params![name],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn rename_kind(conn: &Connection, id: i64, name: &str) -> Result<()> {
    let name = clean_name(name)?;
    if name_taken(conn, &name, Some(id))? {
        return Err(duplicate(name));
    }
    let changed = conn.execute(
        "UPDATE document_kinds SET name = ?1 WHERE id = ?2",
        params![name, id],
    )?;
    if changed == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

pub fn deactivate_kind(conn: &Connection, id: i64) -> Result<()> {
    let active: Option<i64> = conn
        .query_row(
            "SELECT active FROM document_kinds WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .ok();
    match active {
        None => Err(not_found(id)),
        Some(0) => Err(NigelError::Conflict {
            code: "already_inactive",
            message: "That document kind is already inactive.".into(),
        }),
        Some(_) => {
            conn.execute(
                "UPDATE document_kinds SET active = 0 WHERE id = ?1",
                params![id],
            )?;
            Ok(())
        }
    }
}

pub fn active_kind_by_name(conn: &Connection, name: &str) -> Result<DocumentKind> {
    let name = name.trim();
    let kind = conn
        .query_row(
            "SELECT id, name, active, position FROM document_kinds
              WHERE name = ?1 COLLATE NOCASE",
            params![name],
            kind_from_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                NigelError::NotFound(format!("Document kind not found: {name}"))
            }
            other => other.into(),
        })?;
    if !kind.active {
        return Err(NigelError::Conflict {
            code: "kind_inactive",
            message: format!("The document kind \"{}\" is no longer in use.", kind.name),
        });
    }
    Ok(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_are_rows_that_can_be_added_renamed_and_deactivated() {
        let (_d, conn) = crate::documents::testing::test_conn();
        let id = add_kind(&conn, "Statement of work").unwrap();
        rename_kind(&conn, id, "SOW").unwrap();
        deactivate_kind(&conn, id).unwrap();
        let active: Vec<String> = list_kinds(&conn, false)
            .unwrap()
            .into_iter()
            .map(|k| k.name)
            .collect();
        assert_eq!(active, ["Proposal", "Estimate", "Agreement"]);
        let all = list_kinds(&conn, true).unwrap();
        assert!(all.iter().any(|k| k.name == "SOW" && !k.active));
        assert!(matches!(
            active_kind_by_name(&conn, "sow").unwrap_err(),
            NigelError::Conflict {
                code: "kind_inactive",
                ..
            }
        ));
        assert!(matches!(
            add_kind(&conn, "proposal"),
            Err(NigelError::DuplicateName { .. })
        ));
        assert!(matches!(
            active_kind_by_name(&conn, "Memo"),
            Err(NigelError::NotFound(_))
        ));
    }

    #[test]
    fn deactivating_twice_conflicts_and_unknown_ids_are_not_found() {
        let (_d, conn) = crate::documents::testing::test_conn();
        let id = add_kind(&conn, "Memo").unwrap();
        deactivate_kind(&conn, id).unwrap();
        assert!(matches!(
            deactivate_kind(&conn, id),
            Err(NigelError::Conflict {
                code: "already_inactive",
                ..
            })
        ));
        assert!(matches!(
            rename_kind(&conn, 9999, "X"),
            Err(NigelError::NotFound(_))
        ));
        assert!(matches!(
            deactivate_kind(&conn, 9999),
            Err(NigelError::NotFound(_))
        ));
        assert!(matches!(add_kind(&conn, "  "), Err(NigelError::Invalid(_))));
    }

    #[test]
    fn no_rust_enum_mirrors_the_kind_list() {
        let sources = [include_str!("model.rs"), include_str!("kinds.rs")];
        for source in sources {
            assert!(
                !source.contains(concat!("enum ", "DocumentKind")),
                "a compiled-in kind list"
            );
            assert!(
                !source.contains(concat!("Proposal", " =>")),
                "a compiled-in kind name"
            );
        }
    }
}
