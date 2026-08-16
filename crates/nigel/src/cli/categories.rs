use comfy_table::{Cell, Table};

use nigel_core::db::get_connection;
use nigel_core::error::Result;
use nigel_core::settings::get_data_dir;

pub use nigel_core::categories::*;

pub fn add(
    name: &str,
    category_type: &str,
    tax_line: Option<&str>,
    form_line: Option<&str>,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    add_category(&conn, name, category_type, tax_line, form_line)?;
    println!("Added category: {name}");
    Ok(())
}

pub fn list() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let categories = list_categories(&conn)?;

    let mut table = Table::new();
    table.set_header(vec!["ID", "Name", "Type", "Tax Line", "Form Line"]);
    for cat in categories {
        table.add_row(vec![
            Cell::new(cat.id),
            Cell::new(cat.name),
            Cell::new(cat.category_type),
            Cell::new(cat.tax_line.unwrap_or_default()),
            Cell::new(cat.form_line.unwrap_or_default()),
        ]);
    }
    println!("Categories\n{table}");
    Ok(())
}

pub fn rename(id: i64, new_name: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    rename_category(&conn, id, new_name)?;
    println!("Renamed category {id} to: {new_name}");
    Ok(())
}

pub fn update(
    id: i64,
    name: &str,
    category_type: &str,
    tax_line: Option<&str>,
    form_line: Option<&str>,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    update_category(&conn, id, name, category_type, tax_line, form_line)?;
    println!("Updated category {id}: {name}");
    Ok(())
}

pub fn delete(id: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    delete_category(&conn, id)?;
    println!("Deleted category {id}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nigel_core::db::init_db;
    use nigel_core::error::NigelError;
    use rusqlite::Connection;

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        init_db(&conn).unwrap();
        (dir, conn)
    }

    #[test]
    fn test_list_categories_returns_seeded() {
        let (_dir, conn) = test_conn();
        let categories = list_categories(&conn).unwrap();
        assert!(!categories.is_empty(), "should have seeded categories");
        // Spot-check known seeded categories
        assert!(categories
            .iter()
            .any(|c| c.name == "Client Services" && c.category_type == "income"));
        assert!(categories
            .iter()
            .any(|c| c.name == "Office Expense" && c.category_type == "expense"));
        // Income categories come first (explicit CASE ordering)
        let first_expense_idx = categories.iter().position(|c| c.category_type == "expense");
        let last_income_idx = categories.iter().rposition(|c| c.category_type == "income");
        if let (Some(last_inc), Some(first_exp)) = (last_income_idx, first_expense_idx) {
            assert!(
                last_inc < first_exp,
                "income categories should come before expense categories"
            );
        }
    }

    #[test]
    fn test_add_category_and_list() {
        let (_dir, conn) = test_conn();
        add_category(&conn, "New Category", "income", Some("Line 1"), None).unwrap();
        let categories = list_categories(&conn).unwrap();
        let found = categories.iter().find(|c| c.name == "New Category");
        assert!(
            found.is_some(),
            "newly added category should appear in list"
        );
        let cat = found.unwrap();
        assert_eq!(cat.category_type, "income");
        assert_eq!(cat.tax_line.as_deref(), Some("Line 1"));
        assert!(cat.form_line.is_none());
    }

    #[test]
    fn test_add_duplicate_name_rejected() {
        let (_dir, conn) = test_conn();
        add_category(&conn, "Test Cat", "expense", None, None).unwrap();
        let err = add_category(&conn, "Test Cat", "income", None, None).unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn test_add_invalid_type_rejected() {
        let (_dir, conn) = test_conn();
        let err = add_category(&conn, "Bad Type", "revenue", None, None).unwrap_err();
        assert!(err.to_string().contains("Invalid category type"));
    }

    #[test]
    fn test_add_empty_name_rejected() {
        let (_dir, conn) = test_conn();
        let err = add_category(&conn, "  ", "income", None, None).unwrap_err();
        assert!(err.to_string().contains("Name is required"));
    }

    #[test]
    fn test_update_category_changes_all_fields() {
        let (_dir, conn) = test_conn();
        add_category(
            &conn,
            "Original",
            "expense",
            Some("Line 8"),
            Some("1120S-16"),
        )
        .unwrap();
        let id = list_categories(&conn)
            .unwrap()
            .iter()
            .find(|c| c.name == "Original")
            .unwrap()
            .id;

        update_category(
            &conn,
            id,
            "Updated",
            "income",
            Some("Gross receipts"),
            Some("K-4"),
        )
        .unwrap();

        let cat = list_categories(&conn)
            .unwrap()
            .into_iter()
            .find(|c| c.id == id)
            .unwrap();
        assert_eq!(cat.name, "Updated");
        assert_eq!(cat.category_type, "income");
        assert_eq!(cat.tax_line.as_deref(), Some("Gross receipts"));
        assert_eq!(cat.form_line.as_deref(), Some("K-4"));
    }

    #[test]
    fn test_rename_category_changes_only_name() {
        let (_dir, conn) = test_conn();
        add_category(&conn, "Before", "expense", Some("Line 8"), Some("1120S-16")).unwrap();
        let id = list_categories(&conn)
            .unwrap()
            .iter()
            .find(|c| c.name == "Before")
            .unwrap()
            .id;

        rename_category(&conn, id, "After").unwrap();

        let cat = list_categories(&conn)
            .unwrap()
            .into_iter()
            .find(|c| c.id == id)
            .unwrap();
        assert_eq!(cat.name, "After");
        assert_eq!(cat.category_type, "expense");
        assert_eq!(cat.tax_line.as_deref(), Some("Line 8"));
        assert_eq!(cat.form_line.as_deref(), Some("1120S-16"));
    }

    #[test]
    fn test_name_uniqueness_excludes_self_on_update() {
        let (_dir, conn) = test_conn();
        add_category(&conn, "Self Update", "expense", None, None).unwrap();
        let id = list_categories(&conn)
            .unwrap()
            .iter()
            .find(|c| c.name == "Self Update")
            .unwrap()
            .id;

        // Updating to the same name should succeed
        update_category(&conn, id, "Self Update", "income", None, None).unwrap();
    }

    #[test]
    fn test_update_duplicate_name_rejected() {
        let (_dir, conn) = test_conn();
        add_category(&conn, "Cat A", "expense", None, None).unwrap();
        add_category(&conn, "Cat B", "expense", None, None).unwrap();
        let id_b = list_categories(&conn)
            .unwrap()
            .iter()
            .find(|c| c.name == "Cat B")
            .unwrap()
            .id;

        // Updating Cat B to Cat A's name should fail
        let err = update_category(&conn, id_b, "Cat A", "expense", None, None).unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn test_delete_unused_category() {
        let (_dir, conn) = test_conn();
        add_category(&conn, "To Delete", "expense", None, None).unwrap();
        let id = list_categories(&conn)
            .unwrap()
            .iter()
            .find(|c| c.name == "To Delete")
            .unwrap()
            .id;

        delete_category(&conn, id).unwrap();

        let found = list_categories(&conn).unwrap().iter().any(|c| c.id == id);
        assert!(!found, "deleted category should not appear in active list");
    }

    #[test]
    fn test_delete_category_with_transactions_blocked() {
        let (_dir, conn) = test_conn();
        add_category(&conn, "Has Txns", "expense", None, None).unwrap();
        let cat_id = list_categories(&conn)
            .unwrap()
            .iter()
            .find(|c| c.name == "Has Txns")
            .unwrap()
            .id;

        // Need an account to create a transaction
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test Acct', 'checking')",
            [],
        )
        .unwrap();
        let acct_id: i64 = conn
            .query_row(
                "SELECT id FROM accounts WHERE name = 'Test Acct'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-01', 'Test', -50.0, ?2)",
            rusqlite::params![acct_id, cat_id],
        )
        .unwrap();

        let err = delete_category(&conn, cat_id).unwrap_err();
        assert!(err.to_string().contains("Cannot delete"));
        assert!(err.to_string().contains("1 transaction"));
    }

    #[test]
    fn test_delete_category_with_active_rules_blocked() {
        let (_dir, conn) = test_conn();
        add_category(&conn, "Has Rules", "expense", None, None).unwrap();
        let cat_id = list_categories(&conn)
            .unwrap()
            .iter()
            .find(|c| c.name == "Has Rules")
            .unwrap()
            .id;

        conn.execute(
            "INSERT INTO rules (pattern, category_id, is_active) VALUES ('test%', ?1, 1)",
            [cat_id],
        )
        .unwrap();

        let err = delete_category(&conn, cat_id).unwrap_err();
        assert!(err.to_string().contains("Cannot delete"));
        assert!(err.to_string().contains("1 active rule"));
    }

    #[test]
    fn test_get_category_round_trips_and_404s() {
        let (_dir, conn) = test_conn();
        let id = add_category(&conn, "Fetched", "expense", Some("Line 8"), None).unwrap();

        let cat = get_category(&conn, id).unwrap();
        assert_eq!(cat.name, "Fetched");
        assert_eq!(cat.tax_line.as_deref(), Some("Line 8"));

        delete_category(&conn, id).unwrap();
        let err = get_category(&conn, id).unwrap_err();
        assert!(
            matches!(err, NigelError::NotFound(_)),
            "a soft-deleted category is gone: {err}"
        );
    }

    #[test]
    fn test_delete_blocker_distinguishes_its_two_reasons() {
        let (_dir, conn) = test_conn();
        let cat_id = add_category(&conn, "Blocked", "expense", None, None).unwrap();
        assert!(delete_blocker(&conn, cat_id).unwrap().is_none());

        conn.execute(
            "INSERT INTO rules (pattern, category_id, is_active) VALUES ('blocked%', ?1, 1)",
            [cat_id],
        )
        .unwrap();
        let block = delete_blocker(&conn, cat_id).unwrap().expect("blocked");
        assert_eq!(block.reason_code(), "has_active_rules");
        assert_eq!(block.count(), Some(1));

        // Transactions outrank rules when a category has both.
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Acct', 'checking')",
            [],
        )
        .unwrap();
        let acct_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-01', 'Test', -5.0, ?2)",
            rusqlite::params![acct_id, cat_id],
        )
        .unwrap();
        let block = delete_blocker(&conn, cat_id).unwrap().expect("blocked");
        assert_eq!(block.reason_code(), "has_transactions");

        // The TUI's sentence still comes from the same place.
        assert_eq!(
            blocking_reason(&conn, cat_id).unwrap().as_deref(),
            Some("Cannot delete: category has 1 transaction")
        );
    }

    #[test]
    fn test_usage_count() {
        let (_dir, conn) = test_conn();
        add_category(&conn, "Counted", "expense", None, None).unwrap();
        let cat_id = list_categories(&conn)
            .unwrap()
            .iter()
            .find(|c| c.name == "Counted")
            .unwrap()
            .id;

        let (txns, rules) = usage_count(&conn, cat_id).unwrap();
        assert_eq!(txns, 0);
        assert_eq!(rules, 0);

        // Add an account, a transaction, and a rule
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Cnt Acct', 'checking')",
            [],
        )
        .unwrap();
        let acct_id: i64 = conn
            .query_row("SELECT id FROM accounts WHERE name = 'Cnt Acct'", [], |r| {
                r.get(0)
            })
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-02-01', 'Count test', -10.0, ?2)",
            rusqlite::params![acct_id, cat_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO rules (pattern, category_id, is_active) VALUES ('count%', ?1, 1)",
            [cat_id],
        )
        .unwrap();
        // Inactive rule should not count
        conn.execute(
            "INSERT INTO rules (pattern, category_id, is_active) VALUES ('inactive%', ?1, 0)",
            [cat_id],
        )
        .unwrap();

        let (txns, rules) = usage_count(&conn, cat_id).unwrap();
        assert_eq!(txns, 1);
        assert_eq!(rules, 1);
    }

    #[test]
    fn test_cli_list_runs_without_error() {
        // CLI list() requires settings + a real DB; just verify the data-layer list works
        let (_dir, conn) = test_conn();
        let categories = list_categories(&conn).unwrap();
        assert!(!categories.is_empty());
    }

    #[test]
    fn test_delete_nonexistent_category() {
        let (_dir, conn) = test_conn();
        let err = delete_category(&conn, 99999).unwrap_err();
        assert!(err.to_string().contains("Category not found"));
    }

    #[test]
    fn test_rename_nonexistent_category() {
        let (_dir, conn) = test_conn();
        let err = rename_category(&conn, 99999, "Nope").unwrap_err();
        assert!(err.to_string().contains("Category not found"));
    }

    #[test]
    fn test_update_nonexistent_category() {
        let (_dir, conn) = test_conn();
        let err = update_category(&conn, 99999, "Nope", "income", None, None).unwrap_err();
        assert!(err.to_string().contains("Category not found"));
    }
}
