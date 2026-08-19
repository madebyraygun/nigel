# Import Integrity Implementation Plan (TASK-50, TASK-51, TASK-52)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An import either happens completely and honestly or leaves no trace. One SQL transaction wraps import + categorize so a failure rolls back to the pre-import state and never spends the file's checksum; a parse that yields no rows is refused with reasons instead of recorded as an empty success; and the rows a parser drops become data — a count on the `imports` row and a row apiece in `import_rejects` — visible in the history, the CLI, the API and `nigel status`.

**Architecture:** The unit of work becomes a single function in `importer.rs` — `import_and_categorize(conn, &ImportRequest)` — that opens `conn.unchecked_transaction()`, runs `import_file`, runs `categorize_transactions`, and commits. All three entry points (the CLI `nigel import`, the TUI import manager, and `POST /api/imports/confirm`) call it instead of sequencing the two steps themselves; the pre-import snapshot stays outside, taken before the transaction opens, because it is the escape hatch for the failures a transaction cannot help with. Parsers stop returning a bare malformed count and return a `ParseOutcome { rows, rejects }` where each `RejectedRow` carries the file line, the raw row and the parser's own reason. Those reasons feed two things: the refusal message for a zero-row parse (`NigelError::EmptyImport`), and the `import_rejects` rows written inside the same transaction, removed with the import by an `ON DELETE CASCADE`.

**Tech Stack:** Rust 2021, rusqlite/SQLite, axum, serde; TypeScript, Lit 3, Web Awesome, vitest, axe.

**Spec:** `docs/superpowers/specs/2026-08-19-import-integrity-design.md`. Tasks: `backlog task 50 --plain`, `backlog task 51 --plain`, `backlog task 52 --plain`.

## Spec-versus-code notes

Three places where the spec's description and the code on the branch do not line up. Each is resolved here rather than silently:

1. **There are three entry points, not two.** The spec names "the confirm route's plan and the CLI import". `crates/nigel/src/cli/import_manager.rs::run_import` — the TUI's import screen — runs the same snapshot → `import_file` → `categorize_transactions` sequence and has the same defect. It is converted with the other two in Task 2.
2. **The refusal needs the reasons before the rejects table exists.** The spec sequences the zero-row refusal (§2) before the reject capture (§3), but the refusal message is specified to carry "the first few reasons", which no parser produces today. So the *parser-level* capture (`ParseOutcome`/`RejectedRow`) lands in Task 3 with the refusal that first consumes it; the *persistence* of those rejects (schema, writes, surfacing) follows in Tasks 4–8, exactly as the spec sequences the user-visible behaviour.
3. **The zero-row refusal applies to a real import, not to a preview.** `POST /api/imports/preview` is a dry run whose whole job is to report what would happen, and the SPA already renders a zero-row preview with "There is nothing here to import…". Turning that into a `400` would break the wizard's one honest use for a zero-row answer. The refusal is raised only when `dry_run` is false.
4. **TASK-52's headline scenario is not on a counted path.** "A bank changing its date format mid-statement" lands in `parse_bofa_checking`'s `let Some(date) = parse_date_mdy(…) else { continue }` — a silent skip that never touched the malformed counter. Task 3 reclassifies the post-header date failure, short row and empty description as rejects in both BofA parsers, keeping only the genuinely-not-a-transaction skips (empty first field, "Beginning balance") silent.

## Global Constraints

- **⛔ No real book data, in any file or commit message.** Fixture cast only: Acme, Cedar Systems, Juniper Labs, Harbor & Vale, Globex, Initech, with invented amounts. `./scripts/check-no-real-data.sh --staged` runs in the pre-commit hook — **judge it by exit status, never by grepping its output**, and never bypass it.
- **Tests run serially:** `cargo test -- --test-threads=1`. The DB password is a process global.
- **CI runs, in order:** `./scripts/check-no-real-data.sh`, `npm run lint`, `npm run typecheck`, `npm test`, `npm run build`, `cargo fmt --check`, `cargo clippy -- -D warnings`, then four `cargo test` variants: default, `--no-default-features`, `--no-default-features --features serve`, and `-p nigel-core` — each with `-- --test-threads=1`. A task is not done until the ones it can affect pass locally. `-p nigel-core` matters here: a root-level run unifies features and would mask a `#[cfg(feature = "gusto")]` mistake in the parser changes.
- **`cargo fmt` and `cargo clippy -- -D warnings` are part of every task**, not a final sweep.
- **Web changes run `npm test`, `npm run lint`, `npm run typecheck` from `web/`.**
- **Component-first for every visual change:** the component lives in `web/packages/ui/src/components/wc-*.ts`, its `.preview.ts` covers the visible states, its `.test.ts` calls `describePreviewA11y(preview)` with zero violations, and only then does `web/apps/app` consume it. No bespoke components in `apps/app`.
- **No provenance comments.** No "added in", "was formerly", "renamed because". Describe the current state; `git log` carries the history. This applies to docs prose too.
- **Never edit a task file directly.** `backlog task …` owns them, and task files are committed to `main`, not to this branch.

---

### Task 1: One transaction around import and categorize

**Files:**
- Modify: `crates/nigel-core/src/importer.rs`
- Test: `crates/nigel-core/src/importer.rs` (its `mod tests`)

**Interfaces:**
- Consumes: `importer::import_file`, `categorizer::{categorize_transactions, CategorizeResult}`.
- Produces, all from `crates/nigel-core/src/importer.rs`:
  ```rust
  pub struct ImportRequest<'a> {
      pub file_path: &'a Path,
      pub account_name: &'a str,
      pub format_key: Option<&'a str>,
      pub inline_config: Option<&'a GenericCsvConfig>,
  }

  pub struct ImportOutcome {
      pub result: ImportResult,
      pub categorized: usize,
      pub still_flagged: usize,
  }

  pub fn import_and_categorize(
      conn: &Connection,
      request: &ImportRequest<'_>,
  ) -> Result<ImportOutcome>;
  ```

`ImportOutcome` serializes with `#[serde(flatten)]` on `result`, so the confirm route's body keeps the exact shape it has today: the `ImportResult` fields at the top level beside `categorized` and `stillFlagged`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/nigel-core/src/importer.rs`, inside `mod tests`:

```rust
    /// A trigger that aborts the UPDATE `categorize_transactions` runs, which
    /// is a failure between the import and the categorize step with no test
    /// hook in the production path.
    fn break_categorization(conn: &Connection) {
        conn.execute_batch(
            "CREATE TRIGGER fail_categorize BEFORE UPDATE ON transactions \
             BEGIN SELECT RAISE(ABORT, 'injected failure'); END",
        )
        .unwrap();
    }

    fn repair_categorization(conn: &Connection) {
        conn.execute_batch("DROP TRIGGER fail_categorize").unwrap();
    }

    fn add_matching_rule(conn: &Connection) {
        let category_id: i64 = conn
            .query_row("SELECT id FROM categories LIMIT 1", [], |r| r.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO rules (pattern, match_type, vendor, category_id) \
             VALUES ('CEDAR', 'contains', 'Cedar Systems', ?1)",
            [category_id],
        )
        .unwrap();
    }

    fn table_count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn a_failure_during_categorization_leaves_nothing_behind() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        add_matching_rule(&conn);
        let csv_path = write_bofa_csv(
            dir.path(),
            "march.csv",
            &[
                ("03/02/2026", "CEDAR SYSTEMS RETAINER", "2400.00"),
                ("03/09/2026", "GLOBEX HOSTING", "-88.00"),
            ],
        );
        break_categorization(&conn);

        let err = import_and_categorize(
            &conn,
            &ImportRequest {
                file_path: &csv_path,
                account_name: "Test Checking",
                format_key: Some("bofa_checking"),
                inline_config: None,
            },
        )
        .unwrap_err();

        assert!(
            err.to_string().contains("injected failure"),
            "unexpected error: {err}"
        );
        assert_eq!(table_count(&conn, "transactions"), 0);
        assert_eq!(table_count(&conn, "imports"), 0);
    }

    #[test]
    fn a_rolled_back_import_does_not_spend_the_checksum() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        add_matching_rule(&conn);
        let csv_path = write_bofa_csv(
            dir.path(),
            "march.csv",
            &[("03/02/2026", "CEDAR SYSTEMS RETAINER", "2400.00")],
        );
        let request = ImportRequest {
            file_path: &csv_path,
            account_name: "Test Checking",
            format_key: Some("bofa_checking"),
            inline_config: None,
        };

        break_categorization(&conn);
        assert!(import_and_categorize(&conn, &request).is_err());
        repair_categorization(&conn);

        let outcome = import_and_categorize(&conn, &request).unwrap();
        assert!(
            !outcome.result.duplicate_file,
            "the failed run spent the checksum"
        );
        assert_eq!(outcome.result.imported, 1);
        assert_eq!(outcome.categorized, 1);
        assert_eq!(table_count(&conn, "imports"), 1);
    }

    #[test]
    fn a_duplicate_file_commits_nothing_and_categorizes_nothing() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        let csv_path = write_bofa_csv(
            dir.path(),
            "march.csv",
            &[("03/02/2026", "CEDAR SYSTEMS RETAINER", "2400.00")],
        );
        let request = ImportRequest {
            file_path: &csv_path,
            account_name: "Test Checking",
            format_key: Some("bofa_checking"),
            inline_config: None,
        };

        import_and_categorize(&conn, &request).unwrap();
        let again = import_and_categorize(&conn, &request).unwrap();

        assert!(again.result.duplicate_file);
        assert_eq!(again.categorized, 0);
        assert_eq!(again.still_flagged, 0);
        assert_eq!(table_count(&conn, "imports"), 1);
    }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p nigel-core importer:: -- --test-threads=1`
Expected: FAIL to compile — `cannot find function `import_and_categorize` in this scope` and `cannot find struct `ImportRequest` in this scope`.

- [ ] **Step 3: Add the request, the outcome, and the transaction**

In `crates/nigel-core/src/importer.rs`, add to the imports at the top:

```rust
use crate::categorizer::{categorize_transactions, CategorizeResult};
```

Below `import_file`, add:

```rust
/// One import, as an entry point describes it.
///
/// A struct rather than six positional arguments because three callers hand
/// over the same thing and only one of them can see the others.
pub struct ImportRequest<'a> {
    pub file_path: &'a Path,
    pub account_name: &'a str,
    pub format_key: Option<&'a str>,
    pub inline_config: Option<&'a GenericCsvConfig>,
}

/// A committed import: what was written, and what categorization made of it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOutcome {
    #[serde(flatten)]
    pub result: ImportResult,
    pub categorized: usize,
    /// The whole ledger's flagged count, not just this import's.
    pub still_flagged: usize,
}

/// Import a file and categorize what it added, as one unit of work.
///
/// The transaction is the whole point: `import_file` writes the `imports` row
/// and then a transaction apiece, and `categorize_transactions` updates each
/// row it matches, so a failure anywhere in the middle would otherwise leave
/// committed transactions, a spent checksum, and no way to retry the file.
/// Everything here rolls back to the state the pre-import snapshot describes.
///
/// The snapshot itself belongs outside, and before: it is the escape hatch for
/// what a transaction cannot undo.
pub fn import_and_categorize(
    conn: &Connection,
    request: &ImportRequest<'_>,
) -> Result<ImportOutcome> {
    let tx = conn.unchecked_transaction()?;
    let result = import_file(
        &tx,
        request.file_path,
        request.account_name,
        request.format_key,
        false,
        request.inline_config,
    )?;
    // A file already imported is answered, not undone — there is nothing new
    // to categorize, and saying "0 categorized" is the truth rather than a
    // sweep of the whole ledger.
    let counts = if result.duplicate_file {
        CategorizeResult {
            categorized: 0,
            still_flagged: 0,
        }
    } else {
        categorize_transactions(&tx)?
    };
    tx.commit()?;

    Ok(ImportOutcome {
        result,
        categorized: counts.categorized,
        still_flagged: counts.still_flagged,
    })
}
```

`&tx` is accepted where `&Connection` is wanted: `rusqlite::Transaction` derefs to `Connection`, which is how `apply_review` and `delete_import` already work.

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p nigel-core importer:: -- --test-threads=1`
Expected: PASS — `a_failure_during_categorization_leaves_nothing_behind`, `a_rolled_back_import_does_not_spend_the_checksum` and `a_duplicate_file_commits_nothing_and_categorizes_nothing` among them, `test result: ok`, 0 failed.

- [ ] **Step 5: Format, lint, and commit**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test -- --test-threads=1
git add -A && git commit -m "Wrap import and categorize in one transaction"
```
Expected: clippy clean, `test result: ok` for every target, the pre-commit hook exits 0.

---

### Task 2: Every entry point runs the shared unit of work

**Files:**
- Modify: `crates/nigel-core/src/server/routes/imports.rs`
- Modify: `crates/nigel/src/cli/import.rs`
- Modify: `crates/nigel/src/cli/import_manager.rs`
- Test: `crates/nigel-core/src/server/routes/imports.rs` (its `mod tests`)

**Interfaces:**
- Consumes: `importer::{ImportRequest, ImportOutcome, import_and_categorize}` from Task 1.
- Produces: `ImportPlan::run_import(&self, conn: &rusqlite::Connection) -> ApiResult<ImportOutcome>` in `crates/nigel-core/src/server/routes/imports.rs`, replacing the confirm route's hand-rolled sequence. `ConfirmResponse` becomes `{ #[serde(flatten)] outcome: ImportOutcome, snapshot: String }` — the same JSON as today.

- [ ] **Step 1: Write the failing test**

Add to `crates/nigel-core/src/server/routes/imports.rs`, inside `mod tests`:

```rust
    /// The same injected failure the importer tests use, reached through the
    /// route: the confirm must answer an error and leave the database alone.
    #[tokio::test]
    async fn a_confirm_that_fails_partway_writes_nothing_and_can_be_retried() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let before = counts(&db_path);

        {
            let conn = crate::db::open_connection(&db_path, None).expect("open db");
            conn.execute_batch(
                "CREATE TRIGGER fail_categorize BEFORE UPDATE ON transactions \
                 BEGIN SELECT RAISE(ABORT, 'injected failure'); END",
            )
            .unwrap();
        }

        let upload_id = upload_ok(&app, &token, "april.csv", &statement()).await;
        let request = json!({"uploadId": upload_id, "account": "BofA Checking"});
        let (status, body) = post_json(&app, "/api/imports/confirm", &token, &request).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
        assert_eq!(counts(&db_path), before, "the failed confirm wrote rows");
        // The upload survives a failure so the same id can be retried.
        assert_eq!(spooled_count(&db_path), 1);

        {
            let conn = crate::db::open_connection(&db_path, None).expect("open db");
            conn.execute_batch("DROP TRIGGER fail_categorize").unwrap();
        }

        let (status, retried) = post_json(&app, "/api/imports/confirm", &token, &request).await;
        assert_eq!(status, StatusCode::OK, "{retried}");
        assert_eq!(
            retried["duplicateFile"], false,
            "the failed run spent the checksum: {retried}"
        );
        assert_eq!(retried["imported"], 3);
    }
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p nigel-core routes::imports -- --test-threads=1`
Expected: FAIL — the retry answers `duplicateFile: true` and `imported: 0`, so the assertion `the failed run spent the checksum` fires; `counts` also differs from `before`, whichever assertion is reached first.

- [ ] **Step 3: Convert the confirm route**

In `crates/nigel-core/src/server/routes/imports.rs`, replace the `ConfirmResponse` struct and the body of `confirm`'s blocking closure.

The struct becomes:

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfirmResponse {
    #[serde(flatten)]
    outcome: ImportOutcome,
    /// Where the pre-import snapshot went, the same line the CLI prints.
    snapshot: String,
}
```

Add `ImportOutcome`, `ImportRequest` and `import_and_categorize` to the `use crate::importer::{…}` line, and drop the now-unused `use crate::categorizer::categorize_transactions;`.

In `impl ImportPlan`, replace `fn import` with a pair — `import` stays for the preview, and the committed path gets its own:

```rust
    /// The preview: a dry run that writes nothing.
    fn preview(&self, conn: &rusqlite::Connection) -> ApiResult<ImportResult> {
        importer::import_file(
            conn,
            &self.upload.path,
            &self.account,
            self.format.as_deref(),
            true,
            self.mapping.as_ref(),
        )
        .map_err(import_error)
    }

    /// The real thing: import and categorize, committed together or not at all.
    fn run_import(&self, conn: &rusqlite::Connection) -> ApiResult<ImportOutcome> {
        importer::import_and_categorize(
            conn,
            &ImportRequest {
                file_path: &self.upload.path,
                account_name: &self.account,
                format_key: self.format.as_deref(),
                inline_config: self.mapping.as_ref(),
            },
        )
        .map_err(import_error)
    }
```

`fn run` (the preview's entry) becomes:

```rust
    fn run(&self, db_path: &std::path::Path) -> ApiResult<ImportResult> {
        let conn = crate::db::get_connection(db_path)?;
        self.preview(&conn)
    }
```

and `preview`'s handler drops its `true` argument: `blocking(&state, move |db_path| plan.run(&db_path)).await?`.

The confirm closure becomes:

```rust
    let response = blocking(&state, move |db_path| {
        let conn = crate::db::get_connection(&db_path)?;
        // Checked before the snapshot: `import_file` would catch it a moment
        // later, but only after writing a snapshot for an import that was never
        // going to happen.
        super::ensure_account_exists(&conn, &plan.account)?;
        backup::snapshot(&conn, &snapshot_path)?;

        let outcome = plan.run_import(&conn)?;

        // After the import rather than before it, so a file that would not
        // parse does not leave a profile behind.
        if let Some(profile) = profile {
            profile.save(&conn)?;
        }

        Ok(ConfirmResponse {
            outcome,
            snapshot: snapshot_path.display().to_string(),
        })
    })
    .await?;
```

- [ ] **Step 4: Convert the CLI import**

In `crates/nigel/src/cli/import.rs`, change the imports to

```rust
use nigel_core::importer::{
    import_and_categorize, import_file, save_csv_profile, GenericCsvConfig, ImportRequest,
};
```

(dropping `use nigel_core::categorizer::categorize_transactions;`), and split the dry run from the real one. The dry-run branch keeps calling `import_file(&conn, &file_path, account, opts.format, true, inline_config.as_ref())?` and its printing is unchanged. The committed branch becomes:

```rust
    let outcome = import_and_categorize(
        &conn,
        &ImportRequest {
            file_path: &file_path,
            account_name: account,
            format_key: opts.format,
            inline_config: inline_config.as_ref(),
        },
    )?;
    let result = &outcome.result;

    if result.duplicate_file {
        println!("This file has already been imported (duplicate checksum).");
        return Ok(());
    }

    if result.malformed > 0 {
        println!(
            "{} imported, {} skipped (duplicates), {} skipped (malformed data)",
            result.imported, result.skipped, result.malformed
        );
    } else {
        println!(
            "{} imported, {} skipped (duplicates)",
            result.imported, result.skipped
        );
    }
    println!(
        "{} categorized, {} still flagged",
        outcome.categorized, outcome.still_flagged
    );

    Ok(())
```

The duplicate-file check now happens after the snapshot rather than before the categorize, which is where it already was. The dry-run branch returns before any of this, as it does today.

- [ ] **Step 5: Convert the TUI import manager**

In `crates/nigel/src/cli/import_manager.rs`, `run_import` keeps its snapshot and its message building, and the `import_file` + `categorize_transactions` pair becomes one call:

```rust
    match import_and_categorize(
        conn,
        &ImportRequest {
            file_path,
            account_name,
            format_key: None,
            inline_config: None,
        },
    ) {
        Err(e) => ImportResult {
            message: format!("Import failed: {e}"),
            is_error: true,
        },
        Ok(outcome) => {
            if outcome.result.duplicate_file {
                return ImportResult {
                    message: "This file has already been imported (duplicate checksum).".into(),
                    is_error: false,
                };
            }
            let mut msg = if outcome.result.malformed > 0 {
                format!(
                    "{} imported, {} skipped (duplicates), {} skipped (malformed data)",
                    outcome.result.imported, outcome.result.skipped, outcome.result.malformed
                )
            } else {
                format!(
                    "{} imported, {} skipped (duplicates)",
                    outcome.result.imported, outcome.result.skipped
                )
            };
            msg.push_str(&format!(
                "\n{} categorized, {} still flagged",
                outcome.categorized, outcome.still_flagged
            ));
            ImportResult {
                message: msg,
                is_error: false,
            }
        }
    }
```

Update its `use` line to `use nigel_core::importer::{import_and_categorize, ImportRequest};` (keeping whatever else it imports) and drop `categorize_transactions`. A categorization failure is now the import's failure — it rolls back — so the separate "Categorization error" branch goes away with it.

- [ ] **Step 6: Run the tests and watch them pass**

Run: `cargo test -- --test-threads=1`
Expected: PASS — `a_confirm_that_fails_partway_writes_nothing_and_can_be_retried` passes, and every pre-existing importer, route and TUI test still passes. `test result: ok`, 0 failed.

- [ ] **Step 7: Format, lint, and commit**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test --no-default-features -- --test-threads=1
cargo test -p nigel-core -- --test-threads=1
git add -A && git commit -m "Run every import through the transactional seam"
```
Expected: clippy clean, both variants `test result: ok`, hook exits 0.

---

### Task 3: A zero-row parse is a refusal, and parsers say why

**Files:**
- Modify: `crates/nigel-core/src/error.rs`
- Modify: `crates/nigel-core/src/importer.rs`
- Modify: `crates/nigel-core/src/server/error.rs`
- Test: `crates/nigel-core/src/error.rs`, `crates/nigel-core/src/importer.rs`, `crates/nigel-core/src/server/routes/imports.rs`

**Interfaces:**
- Produces, from `crates/nigel-core/src/error.rs`:
  ```rust
  pub struct EmptyImport {
      pub format: String,
      pub malformed: usize,
      pub reasons: Vec<String>,
  }
  impl EmptyImport { pub const REASON_LIMIT: usize = 3; }
  // NigelError::EmptyImport(EmptyImport), Display = EmptyImport's
  ```
- Produces, from `crates/nigel-core/src/importer.rs`:
  ```rust
  pub struct RejectedRow {
      pub row_number: u64,
      pub content: String,
      pub reason: String,
  }

  pub struct ParseOutcome {
      pub rows: Vec<ParsedRow>,
      pub rejects: Vec<RejectedRow>,
  }
  impl ParseOutcome { pub fn malformed(&self) -> usize; }

  impl ImporterKind { pub fn parse(&self, file_path: &Path) -> Result<ParseOutcome>; }
  pub fn parse_generic_csv(file_path: &Path, config: &GenericCsvConfig) -> Result<ParseOutcome>;
  ```
- Produces on the wire: `400 bad_request` with `details = { "reason": "empty_import", "format": …, "malformed": …, "reasons": [...] }`. No new envelope — `details` is what the envelope already carries.

- [ ] **Step 1: Write the failing tests**

In `crates/nigel-core/src/error.rs`, inside `mod tests`:

```rust
    #[test]
    fn an_empty_import_says_the_format_the_count_and_the_first_reasons() {
        let empty = EmptyImport {
            format: "bofa_checking".into(),
            malformed: 3,
            reasons: vec![
                "date \"2026-03-02\" is not MM/DD/YYYY".into(),
                "date \"2026-03-09\" is not MM/DD/YYYY".into(),
                "amount \"n/a\" is not a number".into(),
            ],
        };

        assert_eq!(
            empty.to_string(),
            "Nothing could be read from this file as `bofa_checking`: 3 rows were malformed \
             (date \"2026-03-02\" is not MM/DD/YYYY; date \"2026-03-09\" is not MM/DD/YYYY; \
             amount \"n/a\" is not a number). Nothing was imported and the file was not \
             recorded — correct the format or the column mapping and import it again."
        );
        assert_eq!(NigelError::EmptyImport(empty.clone()).to_string(), empty.to_string());
    }

    #[test]
    fn an_empty_import_with_nothing_malformed_says_so() {
        let empty = EmptyImport {
            format: "generic".into(),
            malformed: 0,
            reasons: Vec::new(),
        };

        assert_eq!(
            empty.to_string(),
            "Nothing could be read from this file as `generic`: no rows matched the format. \
             Nothing was imported and the file was not recorded — correct the format or the \
             column mapping and import it again."
        );
    }
```

In `crates/nigel-core/src/importer.rs`, inside `mod tests`:

```rust
    #[test]
    fn a_parse_that_reads_nothing_is_refused_and_the_file_stays_importable() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        // A statement whose dates are ISO, read as BofA checking, which wants
        // MM/DD/YYYY: every row is malformed and none parses.
        let path = dir.path().join("harbor-and-vale.csv");
        std::fs::write(
            &path,
            "Date,Description,Amount,Running Bal.\n\
             2026-03-02,HARBOR & VALE RETAINER,1800.00,0.00\n\
             2026-03-11,INITECH LICENSE,-240.00,0.00\n",
        )
        .unwrap();

        let err = import_and_categorize(
            &conn,
            &ImportRequest {
                file_path: &path,
                account_name: "Test Checking",
                format_key: Some("bofa_checking"),
                inline_config: None,
            },
        )
        .unwrap_err();

        let NigelError::EmptyImport(empty) = &err else {
            panic!("expected a refusal, got {err}");
        };
        assert_eq!(empty.format, "bofa_checking");
        assert_eq!(empty.malformed, 2);
        assert!(
            empty.reasons[0].contains("is not MM/DD/YYYY"),
            "{:?}",
            empty.reasons
        );
        assert_eq!(table_count(&conn, "imports"), 0);
        assert_eq!(table_count(&conn, "transactions"), 0);

        // The same file, read with a mapping that fits it, imports normally:
        // the refused attempt spent no checksum.
        let outcome = import_and_categorize(
            &conn,
            &ImportRequest {
                file_path: &path,
                account_name: "Test Checking",
                format_key: None,
                inline_config: Some(&GenericCsvConfig {
                    date_col: 0,
                    desc_col: 1,
                    amount_col: 2,
                    date_format: "%Y-%m-%d".into(),
                }),
            },
        )
        .unwrap();
        assert!(!outcome.result.duplicate_file);
        assert_eq!(outcome.result.imported, 2);
    }

    #[test]
    fn a_dry_run_of_an_unreadable_file_still_reports_zero() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        let path = dir.path().join("harbor-and-vale.csv");
        std::fs::write(
            &path,
            "Date,Description,Amount,Running Bal.\n\
             2026-03-02,HARBOR & VALE RETAINER,1800.00,0.00\n",
        )
        .unwrap();

        let result = import_file(
            &conn,
            &path,
            "Test Checking",
            Some("bofa_checking"),
            true,
            None,
        )
        .unwrap();

        assert_eq!(result.imported, 0);
        assert_eq!(result.malformed, 1);
    }

    #[test]
    fn a_rejected_row_carries_its_line_its_content_and_a_reason() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cedar.csv");
        std::fs::write(
            &path,
            "Date,Description,Amount,Running Bal.\n\
             03/02/2026,CEDAR SYSTEMS RETAINER,2400.00,0.00\n\
             03/09/2026,GLOBEX HOSTING,n/a,0.00\n",
        )
        .unwrap();

        let outcome = ImporterKind::BofaChecking.parse(&path).unwrap();

        assert_eq!(outcome.rows.len(), 1);
        assert_eq!(outcome.malformed(), 1);
        let reject = &outcome.rejects[0];
        assert_eq!(reject.row_number, 3);
        assert_eq!(reject.content, "03/09/2026,GLOBEX HOSTING,n/a,0.00");
        assert_eq!(reject.reason, "amount \"n/a\" is not a number");
    }

    #[test]
    fn a_date_the_parser_cannot_read_is_a_reject_rather_than_a_silent_skip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cedar.csv");
        // The bank changed its date format mid-statement.
        std::fs::write(
            &path,
            "Date,Description,Amount,Running Bal.\n\
             03/02/2026,CEDAR SYSTEMS RETAINER,2400.00,0.00\n\
             2026-03-09,GLOBEX HOSTING,-88.00,0.00\n",
        )
        .unwrap();

        let outcome = ImporterKind::BofaChecking.parse(&path).unwrap();

        assert_eq!(outcome.rows.len(), 1);
        assert_eq!(outcome.malformed(), 1);
        assert_eq!(
            outcome.rejects[0].reason,
            "date \"2026-03-09\" is not MM/DD/YYYY"
        );
    }
```

In `crates/nigel-core/src/server/routes/imports.rs`, inside `mod tests`:

```rust
    #[tokio::test]
    async fn a_confirm_that_parses_nothing_is_refused_with_reasons() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let before = counts(&db_path);

        let iso = b"Date,Description,Amount,Running Bal.\n\
                    2026-03-02,HARBOR & VALE RETAINER,1800.00,0.00\n"
            .to_vec();
        let upload_id = upload_ok(&app, &token, "march.csv", &iso).await;
        let request = json!({"uploadId": upload_id, "account": "BofA Checking"});

        let (status, body) = post_json(&app, "/api/imports/confirm", &token, &request).await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "empty_import");
        assert_eq!(body["error"]["details"]["format"], "bofa_checking");
        assert_eq!(body["error"]["details"]["malformed"], 1);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("Nothing could be read"),
            "{body}"
        );
        assert_eq!(counts(&db_path), before, "the refused confirm wrote rows");
    }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p nigel-core -- --test-threads=1 error:: importer:: routes::imports`
Expected: FAIL to compile — `cannot find struct `EmptyImport``, `no variant `EmptyImport` found for enum `NigelError``, `no method named `malformed` found for tuple`.

- [ ] **Step 3: Add the refusal type**

In `crates/nigel-core/src/error.rs`, above `NigelError`:

```rust
/// A parse that produced no rows: which importer ran, how many rows it
/// refused, and the first few reasons it gave.
///
/// Carried rather than flattened into a string at the raise site, because the
/// API publishes the parts as `details` and the CLI prints the sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptyImport {
    /// A built-in importer key, a saved profile name, or `generic`.
    pub format: String,
    pub malformed: usize,
    pub reasons: Vec<String>,
}

impl EmptyImport {
    /// How many reasons a message quotes. Enough to see a pattern, few enough
    /// that a statement of nothing but bad rows does not print itself.
    pub const REASON_LIMIT: usize = 3;
}

impl fmt::Display for EmptyImport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Nothing could be read from this file as `{}`: ", self.format)?;
        if self.malformed == 0 {
            write!(f, "no rows matched the format.")?;
        } else {
            let rows = if self.malformed == 1 { "row was" } else { "rows were" };
            write!(f, "{} {rows} malformed", self.malformed)?;
            if self.reasons.is_empty() {
                write!(f, ".")?;
            } else {
                write!(f, " ({}).", self.reasons.join("; "))?;
            }
        }
        write!(
            f,
            " Nothing was imported and the file was not recorded — correct the format or the column mapping and import it again."
        )
    }
}
```

And the variant, beside `Blocked`:

```rust
    /// A parse that produced no rows. Refused rather than recorded, so the
    /// file's checksum stays free for the corrected import.
    #[error("{0}")]
    EmptyImport(EmptyImport),
```

- [ ] **Step 4: Make the parsers carry rejects**

In `crates/nigel-core/src/importer.rs`, add the two types beside `GenericCsvConfig`:

```rust
/// A row a parser refused, kept as it appeared in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedRow {
    /// The line in the file, 1-based, as the CSV reader counted it.
    pub row_number: u64,
    /// The row's fields, rejoined with commas.
    pub content: String,
    /// What failed, in the parser's own words.
    pub reason: String,
}

/// What one parse produced: the rows that can become transactions, and the
/// rows that cannot with the reason each was refused.
#[derive(Debug, Default)]
pub struct ParseOutcome {
    pub rows: Vec<ParsedRow>,
    pub rejects: Vec<RejectedRow>,
}

impl ParseOutcome {
    /// How many rows were refused. The count and the rows are the same fact,
    /// so there is only one of them to keep correct.
    pub fn malformed(&self) -> usize {
        self.rejects.len()
    }

    fn reject(&mut self, record: &csv::StringRecord, reason: String) {
        self.rejects.push(RejectedRow {
            row_number: record.position().map_or(0, |p| p.line()),
            content: record.iter().collect::<Vec<_>>().join(","),
            reason,
        });
    }

    fn reject_unreadable(&mut self, err: &csv::Error) {
        self.rejects.push(RejectedRow {
            row_number: err.position().map_or(0, |p| p.line()),
            content: String::new(),
            reason: format!("the row could not be read as CSV: {err}"),
        });
    }
}
```

Change the four parse signatures to `Result<ParseOutcome>`: `ImporterKind::parse`, `parse_generic_csv`, `parse_bofa_checking`, `parse_bofa_line_of_credit`/`parse_bofa_credit_card` via `parse_bofa_card_format`. Each `malformed += 1; continue;` becomes an `out.reject(&record, …)` with the reason for that branch, and each `rows.push(…)` becomes `out.rows.push(…)`:

- unreadable record → `out.reject_unreadable(&err)`
- too few columns → `format!("expected at least {min_cols} columns, found {}", record.len())`
- unparsable date, generic → `format!("date {:?} is not {}", raw_date, config.date_format)`
- unparsable date, BofA → `format!("date {:?} is not MM/DD/YYYY", record[idx].trim())`
- empty description → `"the description column is empty".to_string()`
- unparsable amount → `format!("amount {:?} is not a number", raw.trim())`

`{:?}` on a `&str` prints it in double quotes, which is what the expected messages above assert.

In `parse_bofa_checking`, the branches that currently `continue` silently are split: `record[0].trim().is_empty()` and a description containing `Beginning balance` stay silent skips (they are not transactions); `record.len() < 3` with a non-empty first field, an unparsable date, and an empty description become rejects. In `parse_bofa_card_format` the same applies to `record.len() < min_cols`, the unparsable date and the empty description.

The gusto arm becomes:

```rust
            #[cfg(feature = "gusto")]
            // Gusto extracts aggregate totals only; there is no per-row parse
            // to refuse.
            Self::GustoPayroll => parse_gusto_payroll(file_path).map(|rows| ParseOutcome {
                rows,
                rejects: Vec::new(),
            }),
```

Every `let (rows, _malformed) = ImporterKind::…parse(&path).unwrap();` in the existing tests becomes `let outcome = …; ` with `outcome.rows` / `outcome.malformed()` — a mechanical rewrite of the call sites at `importer.rs` lines ~1086–1460.

- [ ] **Step 5: Raise the refusal inside `import_file`**

In `import_file`, replace the parse and the sample lines with:

```rust
    let parsed = match &resolved {
        ResolvedImporter::BuiltIn(kind) => kind.parse(file_path)?,
        ResolvedImporter::Generic(config, _) => parse_generic_csv(file_path, config)?,
    };
    let malformed = parsed.malformed();

    // A parse that read nothing is refused rather than recorded: an `imports`
    // row here would spend the file's checksum on an import that added no
    // transactions, and every corrected retry would answer "already imported".
    // A dry run is exempt — reporting that nothing would be imported is the
    // preview's job.
    if !dry_run && parsed.rows.is_empty() {
        return Err(NigelError::EmptyImport(EmptyImport {
            format,
            malformed,
            reasons: parsed
                .rejects
                .iter()
                .take(EmptyImport::REASON_LIMIT)
                .map(|reject| reject.reason.clone())
                .collect(),
        }));
    }

    let parsed_rows = parsed.rows;
    let sample: Vec<ParsedRow> = parsed_rows.iter().take(5).cloned().collect();
```

Add `EmptyImport` to the `use crate::error::{…}` line. `format` is already bound above the parse and is moved into the error; the later `format: Some(format)` in the `Ok(ImportResult { … })` still compiles, because the branch that moves it returns.

- [ ] **Step 6: Map it on the wire**

In `crates/nigel-core/src/server/error.rs`, in `impl From<NigelError> for ApiError`, add above the `NigelError::Invalid(_)` arm:

```rust
            // A file that parsed to nothing is the caller's to fix — the wrong
            // format, or a column mapping off by one — so the parts go in
            // `details` for a client that wants to say it in its own words.
            NigelError::EmptyImport(empty) => {
                let details = serde_json::json!({
                    "reason": "empty_import",
                    "format": empty.format,
                    "malformed": empty.malformed,
                    "reasons": empty.reasons,
                });
                Self::bad_request(empty.to_string()).with_details(details)
            }
```

- [ ] **Step 7: Run the tests and watch them pass**

Run: `cargo test -- --test-threads=1`
Expected: PASS — the five new tests plus the rewritten parser tests, `test result: ok`, 0 failed.

- [ ] **Step 8: Format, lint, and commit**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test --no-default-features -- --test-threads=1
cargo test -p nigel-core -- --test-threads=1
git add -A && git commit -m "Refuse a zero-row import and give parsers reasons"
```
Expected: clippy clean, both variants `test result: ok`, hook exits 0.

---

### Task 4: Schema for what an import dropped

**Files:**
- Modify: `crates/nigel-core/src/db.rs`
- Modify: `crates/nigel-core/src/migrations.rs`
- Test: `crates/nigel-core/src/migrations.rs` (its `mod tests`)

**Interfaces:**
- Produces: `imports.malformed_count INTEGER NOT NULL DEFAULT 0`, and
  ```sql
  CREATE TABLE import_rejects (
      id INTEGER PRIMARY KEY,
      import_id INTEGER NOT NULL,
      row_number INTEGER NOT NULL,
      content TEXT NOT NULL,
      reason TEXT NOT NULL,
      FOREIGN KEY (import_id) REFERENCES imports(id) ON DELETE CASCADE
  );
  CREATE INDEX idx_import_rejects_import ON import_rejects(import_id);
  ```
  reachable both from `db::SCHEMA` (fresh databases) and from migration v10 (existing ones). `migrations::LATEST_VERSION` becomes 10.

- [ ] **Step 1: Write the failing tests**

Add to `crates/nigel-core/src/migrations.rs`, inside `mod tests`:

```rust
    #[test]
    fn a_database_from_before_the_column_migrates_to_zero_dropped_rows() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        // An `imports` table as it was before it recorded what it dropped.
        conn.execute_batch(
            "CREATE TABLE accounts (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                account_type TEXT NOT NULL
             );
             CREATE TABLE imports (
                id INTEGER PRIMARY KEY,
                filename TEXT NOT NULL,
                account_id INTEGER,
                import_date TEXT DEFAULT (datetime('now')),
                record_count INTEGER,
                date_range_start TEXT,
                date_range_end TEXT,
                checksum TEXT
             );
             CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO accounts (name, account_type)
                 VALUES ('Cedar Systems Checking', 'checking');
             INSERT INTO imports (filename, account_id, record_count, checksum)
                 VALUES ('march.csv', 1, 12, 'a1b2c3');",
        )
        .unwrap();
        set_metadata(&conn, "schema_version", "9").unwrap();

        run_migrations(&conn).unwrap();

        assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
        let malformed: i64 = conn
            .query_row("SELECT malformed_count FROM imports WHERE id = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(malformed, 0, "an old import reads as having dropped nothing");
        let rejects: i64 = conn
            .query_row("SELECT count(*) FROM import_rejects", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rejects, 0);
    }

    #[test]
    fn deleting_an_import_takes_its_rejects_with_it() {
        let (_dir, conn) = test_db();
        conn.execute_batch(
            "INSERT INTO accounts (name, account_type) VALUES ('Cedar Systems Checking', 'checking');
             INSERT INTO imports (filename, account_id, record_count, checksum, malformed_count)
                 VALUES ('march.csv', 1, 4, 'a1b2c3', 2);
             INSERT INTO import_rejects (import_id, row_number, content, reason)
                 VALUES (1, 7, '2026-03-09,GLOBEX HOSTING,-88.00', 'date \"2026-03-09\" is not MM/DD/YYYY'),
                        (1, 9, '2026-03-11,INITECH LICENSE,n/a', 'amount \"n/a\" is not a number');",
        )
        .unwrap();

        conn.execute("DELETE FROM imports WHERE id = 1", []).unwrap();

        let left: i64 = conn
            .query_row("SELECT count(*) FROM import_rejects", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0, "the cascade did not fire");
    }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p nigel-core migrations:: -- --test-threads=1`
Expected: FAIL — both tests error with `no such table: import_rejects`.

- [ ] **Step 3: Add the column and the table**

In `crates/nigel-core/src/db.rs`, in `SCHEMA`, the `imports` table gains a column before its `FOREIGN KEY` line:

```sql
    checksum TEXT,
    malformed_count INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
);

CREATE TABLE IF NOT EXISTS import_rejects (
    id INTEGER PRIMARY KEY,
    import_id INTEGER NOT NULL,
    row_number INTEGER NOT NULL,
    content TEXT NOT NULL,
    reason TEXT NOT NULL,
    FOREIGN KEY (import_id) REFERENCES imports(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_import_rejects_import ON import_rejects(import_id);
```

In `crates/nigel-core/src/migrations.rs`, append to `MIGRATIONS`:

```rust
    Migration {
        version: 10,
        description: "record what an import dropped: imports.malformed_count and import_rejects",
        up: |conn| {
            // v5's probe, for v5's reason: SQLite has no ADD COLUMN IF NOT
            // EXISTS, and a replay must be harmless.
            let has_column: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('imports') WHERE name = 'malformed_count'",
                [],
                |r| r.get(0),
            )?;
            if !has_column {
                // NOT NULL DEFAULT 0 fills every existing row: an import from
                // before the count existed dropped nothing anyone recorded.
                conn.execute_batch(
                    "ALTER TABLE imports ADD COLUMN malformed_count INTEGER NOT NULL DEFAULT 0",
                )?;
            }
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS import_rejects (
                    id INTEGER PRIMARY KEY,
                    import_id INTEGER NOT NULL,
                    row_number INTEGER NOT NULL,
                    content TEXT NOT NULL,
                    reason TEXT NOT NULL,
                    FOREIGN KEY (import_id) REFERENCES imports(id) ON DELETE CASCADE
                 );
                 CREATE INDEX IF NOT EXISTS idx_import_rejects_import
                     ON import_rejects(import_id);",
            )?;
            Ok(())
        },
    },
```

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p nigel-core migrations:: -- --test-threads=1`
Expected: PASS — including `test_fresh_install_at_latest_version` at v10 and `test_v0_upgrade`. `test result: ok`, 0 failed.

- [ ] **Step 5: Format, lint, and commit**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test -- --test-threads=1
git add -A && git commit -m "Add malformed_count and the import_rejects table"
```
Expected: clippy clean, `test result: ok`, hook exits 0.

---

### Task 5: An import writes what it dropped

**Files:**
- Modify: `crates/nigel-core/src/importer.rs`
- Test: `crates/nigel-core/src/importer.rs` (its `mod tests`)

**Interfaces:**
- Consumes: `ParseOutcome`/`RejectedRow` (Task 3), the schema (Task 4).
- Produces: `import_file` writes `imports.malformed_count` and one `import_rejects` row per `RejectedRow`, inside the caller's transaction. `ImportResult` is unchanged on the wire — `malformed` stays the count.

- [ ] **Step 1: Write the failing test**

Add to `crates/nigel-core/src/importer.rs`, inside `mod tests`:

```rust
    #[test]
    fn a_committed_import_records_the_rows_it_dropped() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        let path = dir.path().join("march.csv");
        std::fs::write(
            &path,
            "Date,Description,Amount,Running Bal.\n\
             03/02/2026,CEDAR SYSTEMS RETAINER,2400.00,0.00\n\
             2026-03-09,GLOBEX HOSTING,-88.00,0.00\n\
             03/11/2026,INITECH LICENSE,n/a,0.00\n",
        )
        .unwrap();

        let outcome = import_and_categorize(
            &conn,
            &ImportRequest {
                file_path: &path,
                account_name: "Test Checking",
                format_key: Some("bofa_checking"),
                inline_config: None,
            },
        )
        .unwrap();

        assert_eq!(outcome.result.imported, 1);
        assert_eq!(outcome.result.malformed, 2);

        let import_id = outcome.result.import_id.expect("an imports row");
        let (records, malformed): (i64, i64) = conn
            .query_row(
                "SELECT record_count, malformed_count FROM imports WHERE id = ?1",
                [import_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(records, 1);
        assert_eq!(malformed, 2);

        let mut stmt = conn
            .prepare("SELECT row_number, content, reason FROM import_rejects WHERE import_id = ?1 ORDER BY row_number")
            .unwrap();
        let rejects: Vec<(i64, String, String)> = stmt
            .query_map([import_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(|row| row.unwrap())
            .collect();

        assert_eq!(rejects.len(), 2);
        assert_eq!(rejects[0].0, 3);
        assert_eq!(rejects[0].1, "2026-03-09,GLOBEX HOSTING,-88.00,0.00");
        assert_eq!(rejects[0].2, "date \"2026-03-09\" is not MM/DD/YYYY");
        assert_eq!(rejects[1].2, "amount \"n/a\" is not a number");
    }

    #[test]
    fn a_rolled_back_import_leaves_no_rejects_either() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        add_matching_rule(&conn);
        let path = dir.path().join("march.csv");
        std::fs::write(
            &path,
            "Date,Description,Amount,Running Bal.\n\
             03/02/2026,CEDAR SYSTEMS RETAINER,2400.00,0.00\n\
             03/11/2026,INITECH LICENSE,n/a,0.00\n",
        )
        .unwrap();
        break_categorization(&conn);

        assert!(import_and_categorize(
            &conn,
            &ImportRequest {
                file_path: &path,
                account_name: "Test Checking",
                format_key: Some("bofa_checking"),
                inline_config: None,
            },
        )
        .is_err());

        assert_eq!(table_count(&conn, "import_rejects"), 0);
    }

    #[test]
    fn a_dry_run_records_no_rejects() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        let path = dir.path().join("march.csv");
        std::fs::write(
            &path,
            "Date,Description,Amount,Running Bal.\n\
             03/02/2026,CEDAR SYSTEMS RETAINER,2400.00,0.00\n\
             03/11/2026,INITECH LICENSE,n/a,0.00\n",
        )
        .unwrap();

        let result = import_file(
            &conn,
            &path,
            "Test Checking",
            Some("bofa_checking"),
            true,
            None,
        )
        .unwrap();

        assert_eq!(result.malformed, 1);
        assert_eq!(table_count(&conn, "import_rejects"), 0);
    }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p nigel-core importer:: -- --test-threads=1`
Expected: FAIL — `a_committed_import_records_the_rows_it_dropped` fails at `assert_eq!(malformed, 2)` with `left: 0, right: 2` (nothing writes the column yet).

- [ ] **Step 3: Write the count and the rows**

In `import_file`, the `imports` INSERT gains the column:

```rust
        conn.execute(
            "INSERT INTO imports (filename, account_id, record_count, date_range_start, date_range_end, checksum, malformed_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                file_path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
                account_id,
                parsed_rows.len() as i64,
                min_date,
                max_date,
                checksum,
                malformed as i64,
            ],
        )?;
```

and directly below `let import_id = conn.last_insert_rowid();`:

```rust
        // The rejected rows ride in the caller's transaction with the import
        // they belong to, and leave with it: `import_rejects.import_id`
        // cascades on delete.
        {
            let mut stmt = conn.prepare_cached(
                "INSERT INTO import_rejects (import_id, row_number, content, reason) \
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for reject in &rejects {
                stmt.execute(rusqlite::params![
                    import_id,
                    reject.row_number as i64,
                    reject.content,
                    reject.reason,
                ])?;
            }
        }
```

For this the refusal block in Task 3 keeps the rejects rather than dropping them: replace `let parsed_rows = parsed.rows;` with

```rust
    let ParseOutcome {
        rows: parsed_rows,
        rejects,
    } = parsed;
```

placed after the refusal check (which borrows `parsed.rejects` before the move).

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p nigel-core importer:: -- --test-threads=1`
Expected: PASS — `test result: ok`, 0 failed.

- [ ] **Step 5: Format, lint, and commit**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test -- --test-threads=1
git add -A && git commit -m "Persist rejected rows with the import that dropped them"
```
Expected: clippy clean, `test result: ok`, hook exits 0.

---

### Task 6: Reading the rejects — data layer and API

**Files:**
- Modify: `crates/nigel-core/src/imports.rs`
- Modify: `crates/nigel-core/src/server/routes/imports.rs`
- Test: `crates/nigel-core/src/imports.rs`, `crates/nigel-core/src/server/routes/imports.rs`

**Interfaces:**
- Produces, from `crates/nigel-core/src/imports.rs`:
  ```rust
  pub struct ImportListItem {
      pub id: i64,
      pub filename: String,
      pub account_name: String,
      pub import_date: String,
      pub transaction_count: i64,
      pub malformed_count: i64,   // new
  }

  pub struct ImportReject {
      pub id: i64,
      pub row_number: i64,
      pub content: String,
      pub reason: String,
  }

  pub struct DroppedRows {
      pub account_name: String,
      pub malformed_count: i64,
  }

  pub fn list_rejects(conn: &Connection, import_id: i64) -> Result<Vec<ImportReject>>;
  /// Accounts whose imports dropped rows, worst first. Empty when the books are whole.
  pub fn dropped_rows_by_account(conn: &Connection) -> Result<Vec<DroppedRows>>;
  ```
  All serialize `camelCase`: `malformedCount`, `rowNumber`, `accountName`.
- Produces on the wire: `GET /api/imports/{id}/rejects` → `ImportReject[]`, `404` when the import does not exist.

- [ ] **Step 1: Write the failing tests**

Add to `crates/nigel-core/src/imports.rs` a `mod tests` (the file has none):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};

    fn seeded() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        init_db(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO accounts (name, account_type) VALUES ('Cedar Systems Checking', 'checking');
             INSERT INTO accounts (name, account_type) VALUES ('Globex Card', 'credit_card');
             INSERT INTO imports (filename, account_id, record_count, checksum, malformed_count)
                 VALUES ('march.csv', 1, 4, 'a1b2c3', 2);
             INSERT INTO imports (filename, account_id, record_count, checksum, malformed_count)
                 VALUES ('april.csv', 2, 6, 'd4e5f6', 1);
             INSERT INTO import_rejects (import_id, row_number, content, reason)
                 VALUES (1, 7, '2026-03-09,GLOBEX HOSTING,-88.00', 'date \"2026-03-09\" is not MM/DD/YYYY'),
                        (1, 9, '03/11/2026,INITECH LICENSE,n/a', 'amount \"n/a\" is not a number'),
                        (2, 4, '04/02/2026,HARBOR & VALE,,', 'amount \"\" is not a number');",
        )
        .unwrap();
        (dir, conn)
    }

    #[test]
    fn the_history_carries_the_count_of_what_each_import_dropped() {
        let (_dir, conn) = seeded();
        let items = list_imports(&conn).unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].filename, "april.csv");
        assert_eq!(items[0].malformed_count, 1);
        assert_eq!(items[1].filename, "march.csv");
        assert_eq!(items[1].malformed_count, 2);
    }

    #[test]
    fn an_imports_rejects_read_back_in_file_order() {
        let (_dir, conn) = seeded();
        let rejects = list_rejects(&conn, 1).unwrap();

        assert_eq!(rejects.len(), 2);
        assert_eq!(rejects[0].row_number, 7);
        assert_eq!(rejects[0].content, "2026-03-09,GLOBEX HOSTING,-88.00");
        assert_eq!(rejects[0].reason, "date \"2026-03-09\" is not MM/DD/YYYY");
        assert_eq!(rejects[1].row_number, 9);
        assert!(list_rejects(&conn, 999).unwrap().is_empty());
    }

    #[test]
    fn dropped_rows_are_totalled_per_account_worst_first() {
        let (_dir, conn) = seeded();
        let dropped = dropped_rows_by_account(&conn).unwrap();

        assert_eq!(dropped.len(), 2);
        assert_eq!(dropped[0].account_name, "Cedar Systems Checking");
        assert_eq!(dropped[0].malformed_count, 2);
        assert_eq!(dropped[1].account_name, "Globex Card");
        assert_eq!(dropped[1].malformed_count, 1);
    }

    #[test]
    fn whole_books_report_nothing_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        init_db(&conn).unwrap();

        assert!(dropped_rows_by_account(&conn).unwrap().is_empty());
    }
}
```

Add to `crates/nigel-core/src/server/routes/imports.rs`, inside `mod tests`:

```rust
    #[tokio::test]
    async fn an_imports_rejects_are_readable_and_go_with_the_undo() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let mixed = b"Date,Description,Amount,Running Bal.\n\
                      04/01/2026,CEDAR SYSTEMS RETAINER,2400.00,0.00\n\
                      2026-04-09,GLOBEX HOSTING,-88.00,0.00\n"
            .to_vec();
        let upload_id = upload_ok(&app, &token, "april.csv", &mixed).await;
        let request = json!({"uploadId": upload_id, "account": "BofA Checking"});
        let (status, confirmed) = post_json(&app, "/api/imports/confirm", &token, &request).await;
        assert_eq!(status, StatusCode::OK, "{confirmed}");
        assert_eq!(confirmed["malformed"], 1);
        let import_id = confirmed["importId"].as_i64().expect("an importId");

        let history = ok_json(&app, "/api/imports", &token).await;
        assert_eq!(history[0]["malformedCount"], 1);

        let rejects = ok_json(&app, &format!("/api/imports/{import_id}/rejects"), &token).await;
        assert_eq!(rejects.as_array().unwrap().len(), 1);
        assert_eq!(rejects[0]["rowNumber"], 3);
        assert_eq!(rejects[0]["content"], "2026-04-09,GLOBEX HOSTING,-88.00,0.00");
        assert_eq!(rejects[0]["reason"], "date \"2026-04-09\" is not MM/DD/YYYY");

        let (status, _) = delete_json(&app, &format!("/api/imports/{import_id}"), &token).await;
        assert_eq!(status, StatusCode::OK);
        let (status, body) =
            get_json(&app, &format!("/api/imports/{import_id}/rejects"), &token).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p nigel-core imports:: routes::imports -- --test-threads=1`
Expected: FAIL to compile — `cannot find function `list_rejects``, `no field `malformed_count` on type `ImportListItem``.

- [ ] **Step 3: Add the queries**

In `crates/nigel-core/src/imports.rs`, add `malformed_count` to `ImportListItem` with a doc line, and extend `list_imports`:

```rust
/// One row of import history.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportListItem {
    pub id: i64,
    pub filename: String,
    pub account_name: String,
    pub import_date: String,
    pub transaction_count: i64,
    /// How many rows the parser refused. The rows themselves are in
    /// `import_rejects`, one per count.
    pub malformed_count: i64,
}
```

with the SQL selecting `i.malformed_count` after `COUNT(t.id)` and `malformed_count: row.get(5)?` in the mapping.

Below `delete_import`:

```rust
/// One row an import could not parse.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReject {
    pub id: i64,
    /// The line in the file, as the parser counted it.
    pub row_number: i64,
    pub content: String,
    pub reason: String,
}

/// The rows one import dropped, in the order they appeared in the file.
pub fn list_rejects(conn: &Connection, import_id: i64) -> Result<Vec<ImportReject>> {
    let mut stmt = conn.prepare(
        "SELECT id, row_number, content, reason FROM import_rejects \
         WHERE import_id = ?1 ORDER BY row_number, id",
    )?;
    let rejects = stmt
        .query_map([import_id], |row| {
            Ok(ImportReject {
                id: row.get(0)?,
                row_number: row.get(1)?,
                content: row.get(2)?,
                reason: row.get(3)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rejects)
}

/// An account whose imports dropped rows, and how many.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DroppedRows {
    pub account_name: String,
    pub malformed_count: i64,
}

/// Accounts with incomplete books, worst first. Empty when nothing was ever
/// dropped, which is what makes it worth printing only when it is not.
pub fn dropped_rows_by_account(conn: &Connection) -> Result<Vec<DroppedRows>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(a.name, '(unknown)'), SUM(i.malformed_count)
         FROM imports i
         LEFT JOIN accounts a ON a.id = i.account_id
         GROUP BY i.account_id
         HAVING SUM(i.malformed_count) > 0
         ORDER BY SUM(i.malformed_count) DESC, 1",
    )?;
    let dropped = stmt
        .query_map([], |row| {
            Ok(DroppedRows {
                account_name: row.get(0)?,
                malformed_count: row.get(1)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(dropped)
}
```

- [ ] **Step 4: Add the route**

In `crates/nigel-core/src/server/routes/imports.rs`, register it beside the others — before `/imports/{id}` so the more specific path is not shadowed:

```rust
        .route("/imports/{id}/rejects", get(import_rejects))
```

and the handler, below `list_imports`:

```rust
/// The rows one import could not parse. A missing import is a `404` rather
/// than an empty list: "nothing was dropped" and "no such import" are
/// different answers.
async fn import_rejects(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<Json<Vec<imports::ImportReject>>> {
    let rejects = with_conn(&state, move |conn| {
        if !imports::import_exists(conn, id)? {
            return Err(NigelError::NotFound(format!("No import with ID {id}")));
        }
        imports::list_rejects(conn, id)
    })
    .await?;

    Ok(Json(rejects))
}
```

- [ ] **Step 5: Run the tests and watch them pass**

Run: `cargo test -p nigel-core -- --test-threads=1`
Expected: PASS — including `import_history_is_newest_first_with_counts`, which compares against `list_imports` and so covers the new field for free. `test result: ok`, 0 failed.

- [ ] **Step 6: Format, lint, and commit**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test -- --test-threads=1
git add -A && git commit -m "Read rejects back through the data layer and the API"
```
Expected: clippy clean, `test result: ok`, hook exits 0.

---

### Task 7: The CLI says what was dropped

**Files:**
- Modify: `crates/nigel/src/cli/mod.rs`
- Modify: `crates/nigel/src/main.rs`
- Create: `crates/nigel/src/cli/imports.rs`
- Modify: `crates/nigel/src/cli/status.rs`
- Test: `crates/nigel/src/cli/imports.rs`, `crates/nigel/src/cli/status.rs`

**Interfaces:**
- Consumes: `nigel_core::imports::{list_imports, list_rejects, dropped_rows_by_account, ImportListItem, ImportReject, DroppedRows}`.
- Produces:
  ```rust
  // crates/nigel/src/cli/mod.rs
  pub enum ImportsCommands {
      List,
      Rejects { id: i64 },
  }
  // Commands::Imports { command: ImportsCommands }

  // crates/nigel/src/cli/imports.rs
  pub fn list() -> Result<()>;
  pub fn rejects(id: i64) -> Result<()>;
  pub fn counts_label(item: &ImportListItem) -> String;
  pub fn history_line(item: &ImportListItem) -> String;
  pub fn reject_lines(rejects: &[ImportReject]) -> Vec<String>;

  // crates/nigel/src/cli/status.rs
  pub fn dropped_rows_line(dropped: &[DroppedRows]) -> String;
  ```

- [ ] **Step 1: Write the failing tests**

Create `crates/nigel/src/cli/imports.rs` with its tests first (the module body follows in step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn item(filename: &str, transactions: i64, malformed: i64) -> ImportListItem {
        ImportListItem {
            id: 7,
            filename: filename.into(),
            account_name: "Cedar Systems Checking".into(),
            import_date: "2026-03-02 09:14:11".into(),
            transaction_count: transactions,
            malformed_count: malformed,
        }
    }

    #[test]
    fn a_history_line_puts_the_dropped_count_beside_the_records() {
        assert_eq!(counts_label(&item("march.csv", 42, 2)), "42 rows, 2 dropped");
    }

    #[test]
    fn an_import_that_dropped_nothing_says_nothing_about_it() {
        assert_eq!(counts_label(&item("march.csv", 42, 0)), "42 rows");
    }

    #[test]
    fn a_history_line_carries_the_id_the_file_the_account_and_the_counts() {
        let line = history_line(&item("march.csv", 42, 2));

        assert!(line.starts_with("7 "), "{line}");
        assert!(line.contains("march.csv"), "{line}");
        assert!(line.contains("Cedar Systems Checking"), "{line}");
        assert!(line.contains("2026-03-02 09:14:11"), "{line}");
        assert!(line.ends_with("42 rows, 2 dropped"), "{line}");
    }

    #[test]
    fn a_reject_prints_its_line_its_reason_and_the_row_itself() {
        let rejects = vec![ImportReject {
            id: 1,
            row_number: 7,
            content: "2026-03-09,GLOBEX HOSTING,-88.00".into(),
            reason: "date \"2026-03-09\" is not MM/DD/YYYY".into(),
        }];

        assert_eq!(
            reject_lines(&rejects),
            vec![
                "  line 7   date \"2026-03-09\" is not MM/DD/YYYY".to_string(),
                "           2026-03-09,GLOBEX HOSTING,-88.00".to_string(),
            ]
        );
    }
}
```

Add to `crates/nigel/src/cli/status.rs`, inside `mod tests`:

```rust
    use nigel_core::imports::DroppedRows;

    #[test]
    fn whole_books_report_no_dropped_rows() {
        assert_eq!(super::dropped_rows_line(&[]), "Dropped rows:  0");
    }

    #[test]
    fn dropped_rows_name_the_accounts_they_came_from() {
        let dropped = vec![
            DroppedRows {
                account_name: "Cedar Systems Checking".into(),
                malformed_count: 2,
            },
            DroppedRows {
                account_name: "Globex Card".into(),
                malformed_count: 1,
            },
        ];

        assert_eq!(
            super::dropped_rows_line(&dropped),
            "Dropped rows:  3 (Cedar Systems Checking 2, Globex Card 1) \
             — run `nigel imports rejects <id>` to see them"
        );
    }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p nigel cli::imports cli::status -- --test-threads=1`
Expected: FAIL to compile — `file not found for module `imports`` and `cannot find function `dropped_rows_line` in module `super``.

- [ ] **Step 3: Write the module**

`crates/nigel/src/cli/imports.rs`, above the tests:

```rust
use nigel_core::db::get_connection;
use nigel_core::error::Result;
use nigel_core::imports::{list_imports, list_rejects, ImportListItem, ImportReject};
use nigel_core::settings::get_data_dir;

/// Every import, newest first, with what it kept and what it dropped.
pub fn list() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let items = list_imports(&conn)?;

    if items.is_empty() {
        println!("No imports yet.");
        return Ok(());
    }

    for item in &items {
        println!("{}", history_line(item));
    }
    Ok(())
}

/// What an import kept, and what it dropped when it dropped anything.
pub fn counts_label(item: &ImportListItem) -> String {
    if item.malformed_count > 0 {
        format!(
            "{} rows, {} dropped",
            item.transaction_count, item.malformed_count
        )
    } else {
        format!("{} rows", item.transaction_count)
    }
}

/// One import as the history prints it.
pub fn history_line(item: &ImportListItem) -> String {
    format!(
        "{:<4} {:<25} {:<26} {:<22} {}",
        item.id,
        item.filename,
        item.account_name,
        item.import_date,
        counts_label(item)
    )
}

/// The rows one import could not parse.
pub fn rejects(id: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let rejects = list_rejects(&conn, id)?;

    if rejects.is_empty() {
        println!("Import {id} dropped no rows.");
        return Ok(());
    }

    println!("Import {id} dropped {} rows:", rejects.len());
    for line in reject_lines(&rejects) {
        println!("{line}");
    }
    Ok(())
}

/// Two lines per reject: what failed, then the row it failed on.
pub fn reject_lines(rejects: &[ImportReject]) -> Vec<String> {
    let mut lines = Vec::with_capacity(rejects.len() * 2);
    for reject in rejects {
        lines.push(format!(
            "  line {:<4} {}",
            reject.row_number, reject.reason
        ));
        lines.push(format!("           {}", reject.content));
    }
    lines
}
```

In `crates/nigel/src/cli/mod.rs`, declare the module beside the others (`pub mod imports;`), and add the command and its subcommands:

```rust
    /// Inspect import history and the rows an import dropped.
    Imports {
        #[command(subcommand)]
        command: ImportsCommands,
    },
```

```rust
#[derive(Subcommand)]
pub enum ImportsCommands {
    /// List every import, newest first, with its row and dropped counts.
    List,
    /// Show the rows an import could not parse.
    Rejects {
        /// Import id, from `nigel imports list`.
        id: i64,
    },
}
```

In `crates/nigel/src/main.rs`, add the dispatch arm beside `Commands::Import`, and `ImportsCommands` to the `use nigel::cli::{…}` list:

```rust
        Commands::Imports { command } => match command {
            ImportsCommands::List => cli::imports::list(),
            ImportsCommands::Rejects { id } => cli::imports::rejects(id),
        },
```

In `crates/nigel/src/cli/status.rs`, add the query and the line. After the `rules` query:

```rust
    let dropped = nigel_core::imports::dropped_rows_by_account(&conn)?;
```

after the `Rules:` line:

```rust
    println!("{}", dropped_rows_line(&dropped));
```

and the function itself:

```rust
/// What the books are missing, if anything.
///
/// Named per account, because "some rows were dropped" is not actionable and
/// "Cedar Systems Checking dropped 2" points at the statement to re-import.
pub fn dropped_rows_line(dropped: &[nigel_core::imports::DroppedRows]) -> String {
    let total: i64 = dropped.iter().map(|d| d.malformed_count).sum();
    if total == 0 {
        return "Dropped rows:  0".to_string();
    }
    let detail = dropped
        .iter()
        .map(|d| format!("{} {}", d.account_name, d.malformed_count))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Dropped rows:  {total} ({detail}) — run `nigel imports rejects <id>` to see them"
    )
}
```

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p nigel -- --test-threads=1`
Expected: PASS — `test result: ok`, 0 failed.

- [ ] **Step 5: Check the command by hand**

```bash
cargo run -- imports --help
```
Expected: usage listing `list` and `rejects <ID>`, exit status 0.

- [ ] **Step 6: Format, lint, and commit**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test -- --test-threads=1
git add -A && git commit -m "Surface dropped rows in the CLI history, rejects and status"
```
Expected: clippy clean, `test result: ok`, hook exits 0.

---

### Task 8: The web says what was dropped

**Files:**
- Modify: `web/packages/ui/src/components/wc-import-history.ts`
- Modify: `web/packages/ui/src/components/wc-import-history.preview.ts`
- Modify: `web/packages/ui/src/components/wc-import-history.test.ts`
- Modify: `web/apps/app/src/api/types.ts`, `web/apps/app/src/api/client.ts`, `web/apps/app/src/__mocks__/fake-api-client.ts`
- Modify: `web/apps/app/src/screens/undo-data.ts`, `web/apps/app/src/screens/undo-data.test.ts`, `web/apps/app/src/screens/undo.test.ts`
- Modify: `web/apps/app/src/screens/dashboard-data.ts`, `web/apps/app/src/screens/dashboard-data.test.ts`, `web/apps/app/src/state/dashboard-store.ts`, `web/apps/app/src/screens/dashboard.ts`

The zero-row refusal needs no SPA change: it arrives as `400 bad_request`, and `routeImportError`'s `bad_request` branch already puts the message on the format field (or the mapping field when the request used one), which `import-data.test.ts` already covers.

**Interfaces:**
- Consumes: `GET /api/imports` with `malformedCount`, `GET /api/imports/{id}/rejects` (Task 6).
- Produces:
  ```ts
  // @nigel/ui — wc-import-history.ts
  export interface ImportHistoryRow {
    id: number; filename: string; accountName: string;
    importDate: string; transactionCount: number; malformedCount: number;
  }
  export function droppedCountLabel(count: number): string;

  // apps/app — api/types.ts
  export interface ImportListItem { …; malformedCount: number }
  export interface ImportReject { id: number; rowNumber: number; content: string; reason: string }
  // api/client.ts
  getImportRejects(id: number): Promise<ImportReject[]>;
  // screens/dashboard-data.ts
  export function droppedRowsNotice(imports: ImportListItem[]): string | null;
  ```

- [ ] **Step 1: Write the failing tests**

In `web/packages/ui/src/components/wc-import-history.test.ts`, add `malformedCount` to both fixtures (`12` → `2`, `9` → `0`) and add:

```ts
describe('droppedCountLabel', () => {
  it('says nothing was dropped as a dash, so a whole column reads at a glance', () => {
    expect(droppedCountLabel(0)).toBe('—');
    expect(droppedCountLabel(1)).toBe('1 dropped');
    expect(droppedCountLabel(7)).toBe('7 dropped');
  });
});
```

and inside `describe('wc-import-history')`:

```ts
  it('shows what each import dropped beside what it kept', async () => {
    const el = await mount();
    const rows = [...(el.shadowRoot?.querySelectorAll('tbody tr') ?? [])];

    expect(rows[0].querySelector('.dropped')?.textContent?.trim()).toBe('2 dropped');
    expect(rows[1].querySelector('.dropped')?.textContent?.trim()).toBe('—');
  });
```

with `droppedCountLabel` added to the import list at the top of the file.

In `web/apps/app/src/screens/dashboard-data.test.ts`:

```ts
import { droppedRowsNotice } from './dashboard-data.js';
import type { ImportListItem } from '../api/types.js';

const IMPORT: ImportListItem = {
  id: 7,
  filename: 'march.csv',
  accountName: 'Cedar Systems Checking',
  importDate: '2026-03-02 09:14:11',
  transactionCount: 42,
  malformedCount: 0,
};

describe('droppedRowsNotice', () => {
  it('says nothing when every row landed', () => {
    expect(droppedRowsNotice([IMPORT])).toBeNull();
    expect(droppedRowsNotice([])).toBeNull();
  });

  it('names the accounts whose books are incomplete', () => {
    const notice = droppedRowsNotice([
      { ...IMPORT, malformedCount: 2 },
      {
        ...IMPORT,
        id: 8,
        accountName: 'Globex Card',
        filename: 'april.csv',
        malformedCount: 1,
      },
    ]);

    expect(notice).toBe(
      '3 rows could not be imported — Cedar Systems Checking (2), Globex Card (1). Their books are incomplete.',
    );
  });
});
```

In `web/apps/app/src/screens/undo-data.test.ts` and `web/apps/app/src/screens/undo.test.ts`, add `malformedCount` to the `ImportListItem` fixtures — `2` on the first, `0` on the rest. `undo-data.test.ts` asserts type parity in both directions (`const apiToUi: ImportHistoryRow = ITEMS[0]` and back), so the field must exist on both interfaces or that file will not compile; add `'malformedCount'` to the key list it checks.

- [ ] **Step 2: Run them and watch them fail**

Run: `cd web && npm test`
Expected: FAIL — `droppedCountLabel is not exported`, `droppedRowsNotice is not a function`, and typecheck errors on `malformedCount` not existing on `ImportHistoryRow`/`ImportListItem`.

- [ ] **Step 3: The component**

In `web/packages/ui/src/components/wc-import-history.ts`, add the field to the interface with a doc line, add the label helper, a `.dropped` cell style, and the column:

```ts
/** How many rows the parser refused; the rows are readable through the API. */
malformedCount: number;
```

```ts
/** "2 dropped", but an em dash when nothing was. */
export function droppedCountLabel(count: number): string {
  return count === 0 ? '—' : `${count} dropped`;
}
```

```css
      td.dropped,
      th.dropped {
        text-align: end;
        font-variant-numeric: tabular-nums;
        white-space: nowrap;
      }

      td.dropped[data-dropped='true'] {
        color: var(--nc-color-flagged);
      }
```

In `render()`, a header cell `<th scope="col" class="dropped">Dropped</th>` after `Transactions`; in `renderRow`, after the count cell:

```ts
        <td class="dropped" data-dropped=${item.malformedCount > 0 ? 'true' : 'false'}>
          ${droppedCountLabel(item.malformedCount)}
        </td>
```

In `wc-import-history.preview.ts`, add `malformedCount` to the three fixtures — `2`, `0`, `0` — so the populated state shows both readings. No new state is needed: the dropped column is part of every state the preview already declares, and `describePreviewA11y` covers them.

- [ ] **Step 4: The app**

`api/types.ts`: add `malformedCount: number` to `ImportListItem` with a doc line, and

```ts
/** `GET /api/imports/{id}/rejects` — one row the parser refused. */
export interface ImportReject {
  id: number;
  /** The line in the file, as the parser counted it. */
  rowNumber: number;
  content: string;
  reason: string;
}
```

`api/client.ts`: declare and implement

```ts
  /** The rows one import could not parse. */
  getImportRejects(id: number): Promise<ImportReject[]>;
```
```ts
  getImportRejects(id: number): Promise<ImportReject[]> {
    return this.request<ImportReject[]>('GET', `/imports/${id}/rejects`);
  }
```

`__mocks__/fake-api-client.ts`: add `rejects: Record<number, ImportReject[]> = {}` beside `imports`, implement `getImportRejects` from it, and give any seeded import fixtures `malformedCount: 0`.

`screens/undo-data.ts`: `toImportRows` maps `malformedCount: item.malformedCount`.

`screens/dashboard-data.ts`:

```ts
/**
 * What to say when imports dropped rows, or nothing when they did not.
 *
 * Named per account: "some rows were dropped" is not actionable, and the
 * account is what the reader re-imports a statement into.
 */
export function droppedRowsNotice(imports: ImportListItem[]): string | null {
  const byAccount = new Map<string, number>();
  for (const item of imports) {
    if (item.malformedCount > 0) {
      byAccount.set(
        item.accountName,
        (byAccount.get(item.accountName) ?? 0) + item.malformedCount,
      );
    }
  }
  if (byAccount.size === 0) return null;

  const total = [...byAccount.values()].reduce((sum, n) => sum + n, 0);
  const detail = [...byAccount.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([account, count]) => `${account} (${count})`)
    .join(', ');
  return `${total} rows could not be imported — ${detail}. Their books are incomplete.`;
}
```

`state/dashboard-store.ts`: add `imports: Fetched<ImportListItem[]>` to the interface, a `const imports = slot<ImportListItem[]>()`, `const reloadImports = () => fill(imports, () => client.getImports());`, include it in `load`'s `Promise.all` and in `busy`, expose it, and add `reloadImports` to the interface.

`screens/dashboard.ts`: render the notice above the toolbar, reusing the existing component:

```ts
  private renderDroppedNotice(store: DashboardStore) {
    const notice = droppedRowsNotice(store.imports.data.get() ?? []);
    if (!notice) return nothing;
    return html`
      <wc-notice-bar
        variant="warning"
        message=${notice}
        action-label="Review imports"
        @nc-notice-action=${() => {
          window.location.hash = '#/undo';
        }}
      ></wc-notice-bar>
    `;
  }
```

called from `render()` beside `renderUpdateNotice()`. `wc-notice-bar` declares `info`, `success`, `warning` and `danger`; `warning` is the one that says "your books are incomplete" without claiming the page failed to load.

- [ ] **Step 5: Run the tests and watch them pass**

```bash
cd web && npm run lint && npm run typecheck && npm test
```
Expected: lint clean, typecheck clean, vitest PASS across `@nigel/theme`, `@nigel/ui` and `@nigel/app` — including `describePreviewA11y` for `wc-import-history` with zero violations.

- [ ] **Step 6: Look at it**

```bash
cd web && npm run preview
```
Open http://localhost:9090, find "Import history" under Data, and confirm the Dropped column reads `2 dropped` on the first row and `—` on the others, in both light and dark.

- [ ] **Step 7: Commit**

```bash
cd web && npm run build
git add -A && git commit -m "Show dropped rows in the import history and on the dashboard"
```
Expected: build succeeds, hook exits 0.

---

### Task 9: Documentation

**Files:**
- Modify: `docs/api.md`
- Modify: `docs/architecture.md`
- Modify: `docs/importers.md`
- Modify: `docs/commands.md`

**Interfaces:** consumes everything above; produces no code.

- [ ] **Step 1: `docs/api.md`**

- The `GET /api/imports` entry in the read-side table and the bullet at line ~307: each row now carries `malformedCount` beside `transactionCount`.
- Add `GET /api/imports/{id}/rejects` to the read table: `ImportReject[]`, `404` for an unknown import.
- In "Running an import", the confirm section: the sequence is snapshot, then import and categorize **in one transaction** — a failure rolls back to the pre-import state, leaves no `imports` row, and spends no checksum, so the same `uploadId` can be retried once the cause is fixed.
- In "What is data and what is an error": rows that could not be parsed are counted in `malformed` **and recorded in `import_rejects`**, readable at `GET /api/imports/{id}/rejects`. Add a row to the "Genuine failures" table:

  | A file that parses to no rows at all | `400`, `details.reason` = `empty_import`, with `format`, `malformed` and the first reasons |

- [ ] **Step 2: `docs/architecture.md`**

- The importers bullet: parsers return `ParseOutcome { rows, rejects }`; `import_and_categorize` is the transactional unit of work both front ends run; `ImportResult`/`ImportOutcome` shapes.
- The data-flow line: `snapshot → [transaction: import → categorize] → commit`, with rejects written inside it.
- The `imports` data-layer entry: `list_rejects`, `dropped_rows_by_account`.

- [ ] **Step 3: `docs/importers.md`**

The authoring guide's parse contract: a parser returns `ParseOutcome`, and a row it cannot read is pushed to `rejects` with the file line, the raw row and a reason in the parser's own words — a silent `continue` is only for lines that are not transactions at all (blank rows, balance summaries).

- [ ] **Step 4: `docs/commands.md`**

Add `nigel imports list` and `nigel imports rejects <id>`, and note the `Dropped rows` line in `nigel status`.

- [ ] **Step 5: Check and commit**

```bash
./scripts/check-no-real-data.sh
git add -A && git commit -m "Document import integrity: transaction, refusal, rejects"
```
Expected: the script exits 0 (judge the exit status, not the text), hook exits 0.

---

## Verification

After the last task, the whole set, in CI's order:

```bash
./scripts/check-no-real-data.sh
cd web && npm run lint && npm run typecheck && npm test && npm run build && cd ..
cargo fmt --check
cargo clippy -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
cargo test --no-default-features --features serve -- --test-threads=1
cargo test -p nigel-core -- --test-threads=1
```

Expected: every command exits 0; each `cargo test` variant reports `test result: ok` with 0 failed.

Acceptance criteria, and where each is discharged:

| AC | Task | Test |
|---|---|---|
| 50#1 no imports row, no checksum for a zero-row import | 3 | `a_parse_that_reads_nothing_is_refused_and_the_file_stays_importable` |
| 50#2 the same file re-imports once corrected | 3 | same test, second half |
| 50#3 the CLI reports why, rather than success | 3 | `an_empty_import_says_the_format_the_count_and_the_first_reasons` + `main.rs` printing `Error: {e}` and exiting 1 |
| 50#4 regression test: import nothing, then import again | 3 | `a_parse_that_reads_nothing_is_refused_and_the_file_stays_importable` |
| 51#1 a failure leaves the database as it was | 1, 2 | `a_failure_during_categorization_leaves_nothing_behind`, `a_confirm_that_fails_partway_writes_nothing_and_can_be_retried` |
| 51#2 a partial import is not misreported as a duplicate | 1, 2 | `a_rolled_back_import_does_not_spend_the_checksum`, the retry half of the route test |
| 51#3 a test injects a failure between import and categorize | 1 | the `fail_categorize` trigger |
| 52#1 the count is persisted and visible in the history | 4, 5, 6, 7, 8 | `a_committed_import_records_the_rows_it_dropped`, `the_history_carries_the_count_of_what_each_import_dropped`, `shows what each import dropped beside what it kept` |
| 52#2 the rejected rows are recoverable | 5, 6, 7 | `an_imports_rejects_read_back_in_file_order`, `an_imports_rejects_are_readable_and_go_with_the_undo`, `a_reject_prints_its_line_its_reason_and_the_row_itself` |
| 52#3 `nigel status` or a report surfaces dropped rows | 7, 8 | `dropped_rows_name_the_accounts_they_came_from`, `droppedRowsNotice` |
| Migration on an existing database | 4 | `a_database_from_before_the_column_migrates_to_zero_dropped_rows` |
| Undo removes rejects with the import | 4, 6 | `deleting_an_import_takes_its_rejects_with_it`, the delete half of the route test |
