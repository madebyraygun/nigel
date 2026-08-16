use std::collections::HashMap;

use regex::Regex;
use rusqlite::Connection;
use serde::Serialize;

use crate::categories::ensure_category_exists;
use crate::categorizer::matches as rule_matches;
use crate::error::{NigelError, Result};

/// An active categorization rule joined to the category it assigns.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleRow {
    pub id: i64,
    pub pattern: String,
    pub match_type: String,
    pub vendor: Option<String>,
    pub category: String,
    pub category_id: i64,
    pub priority: i64,
    pub hit_count: i64,
}

/// Active rules in the order the categorizer applies them: highest priority
/// first, ties broken by insertion order so the sequence is stable.
pub fn list_rules(conn: &Connection) -> Result<Vec<RuleRow>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.pattern, r.match_type, r.vendor, c.name, r.category_id, \
                r.priority, r.hit_count \
         FROM rules r JOIN categories c ON r.category_id = c.id \
         WHERE r.is_active = 1 \
         ORDER BY r.priority DESC, r.id ASC",
    )?;
    let rules = stmt
        .query_map([], |row| {
            Ok(RuleRow {
                id: row.get(0)?,
                pattern: row.get(1)?,
                match_type: row.get(2)?,
                vendor: row.get(3)?,
                category: row.get(4)?,
                category_id: row.get(5)?,
                priority: row.get(6)?,
                hit_count: row.get(7)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rules)
}

/// The match types the categorizer understands.
pub const MATCH_TYPES: [&str; 3] = ["contains", "starts_with", "regex"];

/// A rule about to be created.
pub struct NewRule<'a> {
    pub pattern: &'a str,
    pub category_id: i64,
    pub vendor: Option<&'a str>,
    pub match_type: &'a str,
    pub priority: i64,
}

/// Fields to change on an existing rule. `None` leaves a field alone; a
/// `Some(None)` vendor clears it.
#[derive(Debug, Default, Clone)]
pub struct RuleUpdate {
    pub pattern: Option<String>,
    pub match_type: Option<String>,
    pub vendor: Option<Option<String>>,
    pub category_id: Option<i64>,
    pub priority: Option<i64>,
}

impl RuleUpdate {
    fn is_empty(&self) -> bool {
        self.pattern.is_none()
            && self.match_type.is_none()
            && self.vendor.is_none()
            && self.category_id.is_none()
            && self.priority.is_none()
    }
}

/// One description a pattern would match, and how many transactions carry it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleTestMatch {
    pub description: String,
    pub count: i64,
}

/// The dry-run result for a pattern: what it would match today.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleTestResult {
    pub total: i64,
    pub matches: Vec<RuleTestMatch>,
}

/// Reject a match type the categorizer would silently never match on, and a
/// regex that will not compile. `matches()` answers `false` for both, so a rule
/// saved with either is dead weight nobody would notice.
pub fn validate_match_type(match_type: &str, pattern: &str) -> Result<()> {
    if !MATCH_TYPES.contains(&match_type) {
        return Err(NigelError::Invalid(format!(
            "Invalid match type: {match_type}. Must be one of: {}",
            MATCH_TYPES.join(", ")
        )));
    }
    if match_type == "regex" {
        Regex::new(pattern).map_err(|e| NigelError::Invalid(format!("Invalid regex: {e}")))?;
    }
    Ok(())
}

/// Look up a category id by name, the way the CLI addresses categories.
pub fn resolve_category_id(conn: &Connection, name: &str) -> Result<i64> {
    conn.query_row(
        "SELECT id FROM categories WHERE name = ?1 AND is_active = 1",
        [name],
        |row| row.get(0),
    )
    .map_err(|_| NigelError::UnknownCategory(name.to_string()))
}

/// A rule by id, active or not — an edit screen still has to be able to show a
/// rule it is about to reject.
pub fn get_rule(conn: &Connection, id: i64) -> Result<RuleRow> {
    conn.query_row(
        "SELECT r.id, r.pattern, r.match_type, r.vendor, c.name, r.category_id, \
                r.priority, r.hit_count \
         FROM rules r JOIN categories c ON r.category_id = c.id \
         WHERE r.id = ?1",
        [id],
        |row| {
            Ok(RuleRow {
                id: row.get(0)?,
                pattern: row.get(1)?,
                match_type: row.get(2)?,
                vendor: row.get(3)?,
                category: row.get(4)?,
                category_id: row.get(5)?,
                priority: row.get(6)?,
                hit_count: row.get(7)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            NigelError::NotFound(format!("No rule with ID {id}"))
        }
        other => NigelError::Db(other),
    })
}

fn rule_is_active(conn: &Connection, id: i64) -> Result<bool> {
    conn.query_row("SELECT is_active FROM rules WHERE id = ?1", [id], |row| {
        row.get::<_, i32>(0)
    })
    .map(|active| active == 1)
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            NigelError::NotFound(format!("No rule with ID {id}"))
        }
        other => NigelError::Db(other),
    })
}

/// Create a rule and return its id.
pub fn add_rule(conn: &Connection, rule: NewRule<'_>) -> Result<i64> {
    if rule.pattern.trim().is_empty() {
        return Err(NigelError::Invalid("Pattern is required".into()));
    }
    validate_match_type(rule.match_type, rule.pattern)?;
    ensure_category_exists(conn, rule.category_id)?;
    conn.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            rule.pattern,
            rule.match_type,
            rule.vendor,
            rule.category_id,
            rule.priority
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Apply a partial update to an active rule.
pub fn update_rule(conn: &Connection, id: i64, update: &RuleUpdate) -> Result<()> {
    let current = get_rule(conn, id)?;
    if !rule_is_active(conn, id)? {
        return Err(NigelError::Conflict {
            code: "already_inactive",
            message: format!("Rule {id} is inactive"),
        });
    }
    if update.is_empty() {
        return Err(NigelError::Invalid(
            "Nothing to update — provide at least one flag".to_string(),
        ));
    }

    // Only re-validate when the pattern or the match type is actually moving:
    // a rule stored with a since-broken regex should still accept a priority
    // change.
    if update.pattern.is_some() || update.match_type.is_some() {
        let pattern = update.pattern.as_deref().unwrap_or(&current.pattern);
        let match_type = update.match_type.as_deref().unwrap_or(&current.match_type);
        validate_match_type(match_type, pattern)?;
    }
    if let Some(category_id) = update.category_id {
        ensure_category_exists(conn, category_id)?;
    }

    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref pattern) = update.pattern {
        params.push(Box::new(pattern.clone()));
        updates.push(format!("pattern = ?{}", params.len()));
    }
    if let Some(ref match_type) = update.match_type {
        params.push(Box::new(match_type.clone()));
        updates.push(format!("match_type = ?{}", params.len()));
    }
    if let Some(ref vendor) = update.vendor {
        params.push(Box::new(vendor.clone()));
        updates.push(format!("vendor = ?{}", params.len()));
    }
    if let Some(category_id) = update.category_id {
        params.push(Box::new(category_id));
        updates.push(format!("category_id = ?{}", params.len()));
    }
    if let Some(priority) = update.priority {
        params.push(Box::new(priority));
        updates.push(format!("priority = ?{}", params.len()));
    }

    params.push(Box::new(id));
    let sql = format!(
        "UPDATE rules SET {} WHERE id = ?{}",
        updates.join(", "),
        params.len()
    );
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice())?;
    Ok(())
}

/// Soft-delete a rule. The row stays for its hit count and for anything that
/// still references it.
pub fn deactivate_rule(conn: &Connection, id: i64) -> Result<()> {
    if !rule_is_active(conn, id)? {
        return Err(NigelError::Conflict {
            code: "already_inactive",
            message: format!("Rule {id} is already inactive"),
        });
    }
    conn.execute("UPDATE rules SET is_active = 0 WHERE id = ?1", [id])?;
    Ok(())
}

/// What a pattern would match right now, without saving anything: the same
/// scan `nigel rules test` prints, as data.
pub fn test_pattern(conn: &Connection, pattern: &str, match_type: &str) -> Result<RuleTestResult> {
    validate_match_type(match_type, pattern)?;

    let mut stmt = conn.prepare("SELECT description FROM transactions")?;
    let descriptions: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut match_counts: HashMap<String, i64> = HashMap::new();
    for desc in &descriptions {
        if rule_matches(desc, pattern, match_type) {
            *match_counts.entry(desc.clone()).or_default() += 1;
        }
    }

    let total: i64 = match_counts.values().sum();
    let mut matches: Vec<RuleTestMatch> = match_counts
        .into_iter()
        .map(|(description, count)| RuleTestMatch { description, count })
        .collect();
    // Busiest description first, alphabetical within a tie.
    matches.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(a.description.cmp(&b.description))
    });

    Ok(RuleTestResult { total, matches })
}
