use rusqlite::Connection;
use serde::Serialize;

use crate::db::AccountClass;
use crate::error::{DeleteBlock, NigelError, Result};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRow {
    pub id: i64,
    pub name: String,
    pub category_type: String,
    pub class: AccountClass,
    pub tax_line: Option<String>,
    pub form_line: Option<String>,
}

// ---------------------------------------------------------------------------
// Data-layer functions for TUI category management
// ---------------------------------------------------------------------------

pub fn list_categories(conn: &Connection) -> Result<Vec<CategoryRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, category_type, class, tax_line, form_line \
         FROM categories WHERE is_active = 1 \
         ORDER BY CASE category_type WHEN 'income' THEN 0 ELSE 1 END, name ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(|(id, name, category_type, class, tax_line, form_line)| {
            Ok(CategoryRow {
                id,
                name,
                category_type,
                class: AccountClass::from_db(&class)?,
                tax_line,
                form_line,
            })
        })
        .collect()
}

/// Fetch one active category by id.
pub fn get_category(conn: &Connection, id: i64) -> Result<CategoryRow> {
    let (id, name, category_type, class, tax_line, form_line) = conn
        .query_row(
            "SELECT id, name, category_type, class, tax_line, form_line \
             FROM categories WHERE id = ?1 AND is_active = 1",
            [id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                NigelError::NotFound(format!("Category not found: id {id}"))
            }
            other => NigelError::Db(other),
        })?;
    Ok(CategoryRow {
        id,
        name,
        category_type,
        class: AccountClass::from_db(&class)?,
        tax_line,
        form_line,
    })
}

/// Confirm a category id names a category the chart of accounts still offers.
/// Anything referring to a category by id — a rule, a transaction edit — has to
/// pass through here first.
pub fn ensure_category_exists(conn: &Connection, id: i64) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM categories WHERE id = ?1 AND is_active = 1)",
        [id],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(NigelError::NotFound(format!("Category not found: id {id}")))
    }
}

/// Insert a category and return its id.
pub fn add_category(
    conn: &Connection,
    name: &str,
    category_type: &str,
    class: Option<AccountClass>,
    tax_line: Option<&str>,
    form_line: Option<&str>,
) -> Result<i64> {
    validate_fields(name, category_type)?;
    let class = class.unwrap_or_else(|| crate::db::class_for_category_type(category_type));
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM categories WHERE name = ?1 AND is_active = 1)",
        [name],
        |row| row.get(0),
    )?;
    if exists {
        return Err(NigelError::DuplicateName {
            kind: "Category",
            name: name.to_string(),
        });
    }
    conn.execute(
        "INSERT INTO categories (name, category_type, class, tax_line, form_line) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![name, category_type, class.as_str(), tax_line, form_line],
    )?;
    Ok(conn.last_insert_rowid())
}

fn validate_fields(name: &str, category_type: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(NigelError::Invalid("Name is required".into()));
    }
    if category_type != "income" && category_type != "expense" {
        return Err(NigelError::Invalid(format!(
            "Invalid category type: {category_type} (must be 'income' or 'expense')"
        )));
    }
    Ok(())
}

pub fn rename_category(conn: &Connection, id: i64, new_name: &str) -> Result<()> {
    // Fetch the existing row so we can delegate to update_category with current values
    let (cat_type, class, tax_line, form_line): (String, String, Option<String>, Option<String>) =
        conn.query_row(
            "SELECT category_type, class, tax_line, form_line FROM categories WHERE id = ?1 AND is_active = 1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                NigelError::NotFound(format!("Category not found: id {id}"))
            }
            other => NigelError::Db(other),
        })?;
    update_category(
        conn,
        id,
        new_name,
        &cat_type,
        AccountClass::from_db(&class)?,
        tax_line.as_deref(),
        form_line.as_deref(),
    )
}

pub fn update_category(
    conn: &Connection,
    id: i64,
    name: &str,
    category_type: &str,
    class: AccountClass,
    tax_line: Option<&str>,
    form_line: Option<&str>,
) -> Result<()> {
    validate_fields(name, category_type)?;
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM categories WHERE name = ?1 AND is_active = 1 AND id != ?2)",
        rusqlite::params![name, id],
        |row| row.get(0),
    )?;
    if exists {
        return Err(NigelError::DuplicateName {
            kind: "Category",
            name: name.to_string(),
        });
    }
    let updated = conn.execute(
        "UPDATE categories SET name = ?1, category_type = ?2, class = ?3, tax_line = ?4, form_line = ?5 WHERE id = ?6 AND is_active = 1",
        rusqlite::params![name, category_type, class.as_str(), tax_line, form_line, id],
    )?;
    if updated == 0 {
        return Err(NigelError::NotFound(format!("Category not found: id {id}")));
    }
    Ok(())
}

/// Why this category cannot be deleted, or None when it can be. Transactions
/// outrank rules: a category with both reports the transactions.
pub fn delete_blocker(conn: &Connection, id: i64) -> Result<Option<DeleteBlock>> {
    let (txn_count, rule_count) = usage_count(conn, id)?;
    if txn_count > 0 {
        return Ok(Some(DeleteBlock::transactions("category", txn_count)));
    }
    if rule_count > 0 {
        return Ok(Some(DeleteBlock::active_rules("category", rule_count)));
    }
    Ok(None)
}

/// The blocking reason as a sentence, for the TUI status line.
pub fn blocking_reason(conn: &Connection, id: i64) -> Result<Option<String>> {
    Ok(delete_blocker(conn, id)?.map(|block| block.to_string()))
}

pub fn delete_category(conn: &Connection, id: i64) -> Result<()> {
    if let Some(block) = delete_blocker(conn, id)? {
        return Err(NigelError::Blocked(block));
    }
    let updated = conn.execute(
        "UPDATE categories SET is_active = 0 WHERE id = ?1 AND is_active = 1",
        [id],
    )?;
    if updated == 0 {
        return Err(NigelError::NotFound(format!("Category not found: id {id}")));
    }
    Ok(())
}

pub fn usage_count(conn: &Connection, id: i64) -> Result<(i64, i64)> {
    let txn_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM transactions WHERE category_id = ?1",
        [id],
        |row| row.get(0),
    )?;
    let rule_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM rules WHERE category_id = ?1 AND is_active = 1",
        [id],
        |row| row.get(0),
    )?;
    Ok((txn_count, rule_count))
}
