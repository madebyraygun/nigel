# Invoice Engine Correctness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** TASK-69, TASK-71, TASK-63 and TASK-70 in one PR — dates that survive an ISO
string comparison, a status derivation that runs against the wall clock, an encrypted
database proven to unlock for invoice commands, and a recorded decision that
`clients.name` stays advisory — per
`docs/superpowers/specs/2026-08-11-task-69-71-63-70-invoice-correctness-design.md`.

**Architecture:** `invoices::validate_date` starts returning the *normalized*
`YYYY-MM-DD` string, mirroring `validate_currency` two functions below it, and the four
functions that write a date column (`create_invoice`, `update_invoice`, `record_payment`,
`void_invoice`) store what it returns. `record_payment` gains the date check it never
had. Migration v6 pads the dates already in the database. `update_invoice` takes `today`
as a parameter, the way every other date-sensitive function in `src/invoicing/` does, so
the module still never reads the clock. `tests/cli_dispatch.rs` gains an encrypted-database
pair for the invoice/client commands. TASK-70 lands as a decision: no schema change, a
pinning test in `import_invoiceshelf.rs`, and two paragraphs of documentation.

**Tech Stack:** Rust, rusqlite 0.31, chrono, clap (derive), assert_cmd/predicates/tempfile.

## Global Constraints

- After every task, all three test builds green — these are the repo's own CI matrix
  (`.github/workflows/ci.yml`), which is what the "every task must also pass without the
  `pdf` feature" rule means here. The exemplar plan's `--features gusto` spelling is
  **not** what this repo runs; `--no-default-features` alone is:

```bash
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1                  # no gusto, no pdf, no serve
cargo test --no-default-features --features serve -- --test-threads=1 # the server routes
```

- Plus `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean.
- `--test-threads=1` is not optional: the database password is a process global.
- **`src/server/` only compiles under the `serve` feature.** Any task touching a route must
  be verified with the third command above, or the change will not even be built.
- **`src/invoicing/` never reads the clock and never reaches into `src/cli/`.** The
  reference day always arrives as a parameter.
- **No visual surface changes.** No new command, flag, column, route, or printed line.
  `git diff --stat` must show nothing under `web/`, and no change to any table header or
  format string.
- The data layer stays the sole writer: normalization happens in `src/invoicing/`, never
  in a front end. `cli/invoice_manager.rs` must not need editing for TASK-69 — if it does,
  the normalization landed in the wrong place.

---

### Task 1: `validate_date` returns the normalized date

**Files:** modify `src/invoicing/invoices.rs` (the function, `create_invoice`,
`update_invoice`, `ar_aging_detail`, tests); `src/cli/invoice_manager.rs` (two call sites
that only need to compile).

**Interface produced** (consumed by Tasks 2–4):

```rust
/// `YYYY-MM-DD`, zero-padded, or an `Invalid` error naming the field.
///
/// Returns the re-formatted date rather than the caller's string: chrono accepts
/// `2026-8-7`, and a value stored that way is never `>` its own month in
/// `is_overdue`'s string comparison. Same shape as `validate_currency`, which has
/// always returned the normalized code.
pub fn validate_date(value: &str, what: &str) -> Result<String>;
```

- [ ] **Step 1: Write failing tests** in `src/invoicing/invoices.rs`'s `mod tests`
      (`test_conn`, `client_id`, `seed_draft`, `open_invoice`, `AGING_TODAY` all exist):

```rust
#[test]
fn validate_date_normalizes_to_zero_padded_iso() {
    assert_eq!(validate_date("2026-8-7", "issue").unwrap(), "2026-08-07");
    assert_eq!(validate_date("2026-08-07", "issue").unwrap(), "2026-08-07");
    assert_eq!(validate_date("2026-12-31", "due").unwrap(), "2026-12-31");
    for bad in ["March", "2026-13-01", "2026-08-32", "", "2026-08-07extra"] {
        assert!(validate_date(bad, "issue").is_err(), "accepted {bad:?}");
    }
}

#[test]
fn an_unpadded_issue_or_due_date_is_stored_padded() {
    let (_d, conn) = test_conn();
    let cid = client_id(&conn, "Acme");
    let items = vec![NewLineItem { description: "Work".into(), quantity: 1.0, unit_amount: 100.0 }];
    let id = create_invoice(&conn, cid, "2026-8-7", Some("2026-9-1"), "USD", &items, None, None).unwrap();

    let inv = get_invoice(&conn, id).unwrap();
    assert_eq!(inv.issue_date, "2026-08-07");
    assert_eq!(inv.due_date.as_deref(), Some("2026-09-01"));
}

#[test]
fn an_unpadded_date_edited_onto_a_draft_is_stored_padded() {
    let (_d, conn) = test_conn();
    let id = seed_draft(&conn);
    update_invoice(&conn, id, &InvoiceUpdate {
        issue_date: Some("2026-8-7".into()),
        due_date: Some(Some("2026-9-1".into())),
        ..Default::default()
    }).unwrap();

    let inv = get_invoice(&conn, id).unwrap();
    assert_eq!(inv.issue_date, "2026-08-07");
    assert_eq!(inv.due_date.as_deref(), Some("2026-09-01"));
}

/// AC #2, the half that is actually broken: `is_overdue` compares strings, and
/// "2026-08-20" > "2026-8-7" is false.
#[test]
fn overdue_derives_the_same_whether_the_due_date_was_typed_padded_or_not() {
    let (_d, conn) = test_conn();
    let padded = open_invoice(&conn, "Padded", "2026-07-01", Some("2026-07-05"), 100.0);
    let unpadded = open_invoice(&conn, "Unpadded", "2026-07-01", Some("2026-7-5"), 100.0);

    assert_eq!(refresh_status(&conn, padded, "2026-08-20").unwrap(), "overdue");
    assert_eq!(refresh_status(&conn, unpadded, "2026-08-20").unwrap(), "overdue");
}

/// AC #2, the other half. Aging already bucketed both correctly (chrono parses
/// `2026-7-5`); what it printed was the raw string. This pins both.
#[test]
fn aging_buckets_and_prints_the_same_whether_the_due_date_was_typed_padded_or_not() {
    let (_d, conn) = test_conn();
    open_invoice(&conn, "Padded", "2026-06-01", Some("2026-07-05"), 100.0);
    open_invoice(&conn, "Unpadded", "2026-06-01", Some("2026-7-5"), 100.0);

    let report = ar_aging_detail(&conn, AGING_TODAY).unwrap();
    let padded = report.invoices.iter().find(|i| i.client == "Padded").unwrap();
    let unpadded = report.invoices.iter().find(|i| i.client == "Unpadded").unwrap();
    assert_eq!(padded.bucket, unpadded.bucket);
    assert_eq!(padded.days_past_due, unpadded.days_past_due);
    assert_eq!(unpadded.due_date, "2026-07-05", "the stored date must already be padded");
}

#[test]
fn an_unpadded_as_of_date_is_reported_padded() {
    let (_d, conn) = test_conn();
    assert_eq!(ar_aging_detail(&conn, "2026-8-4").unwrap().as_of, "2026-08-04");
}
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib invoicing::invoices 2>&1 | tail -30`
      — compile errors on the `assert_eq!` against `()`, then real failures on the stored
      strings.

- [ ] **Step 3: Implement.** Replace the body of `validate_date`
      (`src/invoicing/invoices.rs:289`):

```rust
pub fn validate_date(value: &str, what: &str) -> Result<String> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        NigelError::Invalid(format!(
            "Invalid {what} date: {value} (expected YYYY-MM-DD)"
        ))
    })?;
    Ok(date.format("%Y-%m-%d").to_string())
}
```

The error sentence is unchanged, verbatim — three front ends and several tests assert on
it.

- [ ] **Step 4: Store what it returns.** In `create_invoice`, replace the two validation
      lines and use the results in the `INSERT` params:

```rust
    let issue_date = validate_date(issue_date, "issue")?;
    let due_date = match due_date {
        Some(due) => Some(validate_date(due, "due")?),
        None => None,
    };
```

(`due_date` becomes `Option<String>`; `rusqlite::params![…, due_date, …]` binds it
unchanged.) In `update_invoice`, normalize before the parameter vector is built:

```rust
    let issue_date = match update.issue_date {
        Some(ref d) => Some(validate_date(d, "issue")?),
        None => None,
    };
    let due_date = match update.due_date {
        Some(Some(ref d)) => Some(Some(validate_date(d, "due")?)),
        Some(None) => Some(None),
        None => None,
    };
```

and push these locals instead of `update.issue_date` / `update.due_date`. **Do not change
`InvoiceUpdate`** — it is a public request struct the API deserializes into.

In `ar_aging_detail`, use the returned value for `as_of` and drop the second parse:

```rust
    let as_of = validate_date(today, "as-of")?;
    let today = NaiveDate::parse_from_str(&as_of, "%Y-%m-%d").expect("validate_date just parsed it");
```

- [ ] **Step 5: Fix the two TUI call sites so the tree compiles**
      (`src/cli/invoice_manager.rs:334`, `:336`, `:1389`). They discard the value — the
      form validates only to attribute a failure to a field, and the data layer re-runs
      and stores. No behavior change, no test change there.

- [ ] **Step 6: Update the stale comment** at `src/server/routes/invoices.rs:270`:
      "…which accepts `2026-4-1`" → "…which accepts **and normalizes** `2026-4-1`". Nothing
      else in that module moves.

- [ ] **Step 7: Verify.** All three test builds, clippy, fmt. The comment on
      `a_malformed_as_of_date_is_invalid_not_other` (`invoices.rs:2240`) still reads true —
      it says zero-padding is not *checked*, which is still the case; it is normalized.

---

### Task 2: `record_payment` validates its date; `void_invoice` normalizes its own

**Files:** modify `src/invoicing/invoices.rs`.

- [ ] **Step 1: Write failing tests** in the same `mod tests`:

```rust
#[test]
fn record_payment_stores_an_unpadded_date_padded() {
    let (_d, conn) = test_conn();
    let id = seed_draft(&conn);
    record_payment(&conn, id, 50.0, "2026-8-9", "ach", None).unwrap();

    let dates: Vec<String> = payments(&conn, id).unwrap().into_iter().map(|p| p.paid_date).collect();
    assert_eq!(dates, vec!["2026-08-09".to_string()]);
}

/// `nigel invoice pay --date March` wrote "March" into the column and then handed
/// it to `refresh_status` as the reference day. The CLI never checked; the TUI did.
#[test]
fn record_payment_refuses_a_malformed_date_instead_of_storing_it() {
    let (_d, conn) = test_conn();
    let id = seed_draft(&conn);

    let err = record_payment(&conn, id, 50.0, "March", "ach", None).unwrap_err();
    assert!(matches!(err, NigelError::Invalid(_)), "{err:?}");
    assert_eq!(err.to_string(), "Invalid payment date: March (expected YYYY-MM-DD)");
    assert_eq!(paid_amount(&conn, id).unwrap(), 0.0, "a refused payment writes no row");
}

#[test]
fn void_invoice_stores_an_unpadded_date_padded() {
    let (_d, conn) = test_conn();
    let id = seed_draft(&conn);
    void_invoice(&conn, id, "2026-8-6").unwrap();

    assert_eq!(get_invoice(&conn, id).unwrap().voided_at.as_deref(), Some("2026-08-06"));
}
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib invoicing::invoices 2>&1 | tail -30`

- [ ] **Step 3: Implement.** In `record_payment`, beside the method check — the position
      matters, both guards belong to the data layer so a caller reaching it directly
      cannot get past them:

```rust
    validate_payment_method(method)?;
    let paid_date = validate_date(paid_date, "payment")?;
    ensure_not_void(&get_invoice(conn, invoice_id)?, "paid")?;
```

and use `&paid_date` in the `INSERT` and in the `refresh_status` call that follows. In
`void_invoice`, first line after the guard:

```rust
    let voided_on = validate_date(voided_on, "void")?;
```

using `&voided_on` for the `UPDATE` and the `refresh_status` call.

- [ ] **Step 4: Verify.** All three builds. Watch for existing tests that hand
      `record_payment` or `void_invoice` something unparseable — `src/server/testutil.rs`,
      `src/invoicing/sync.rs` and `src/invoicing/send.rs` all pass ISO literals today, so
      none should break. If one does, the fixture was writing junk and the fixture is what
      changes.

---

### Task 3: Migration v6 — pad the dates already in the database

**Files:** modify `src/migrations.rs`.

Normalizing on write does nothing for a row written before it. AC #2 is a claim about a
database, not a function.

- [ ] **Step 1: Write failing tests** in `src/migrations.rs`'s `mod tests` (`test_db()`
      exists and already runs `init_db`, so the schema is at `LATEST_VERSION`; rewind the
      version to force a re-run, the way `downgrade_to_v2` does in the integration tests):

```rust
#[test]
fn v6_pads_stored_invoice_dates_and_leaves_unparseable_ones_alone() {
    let (_dir, conn) = test_db();
    conn.execute_batch(
        "INSERT INTO clients (id, name) VALUES (1, 'Acme');
         INSERT INTO invoices (id, number, client_id, issue_date, due_date, published_at,
                               voided_at, status, currency, total, token)
             VALUES (1, 1248, 1, '2026-8-7', '2026-9-1', '2026-8-8', NULL, 'sent', 'USD', 100, 't1'),
                    (2, 1249, 1, '2026-08-07', NULL, NULL, '2026-8-9', 'void', 'USD', 100, 't2'),
                    (3, 1250, 1, 'March',     NULL, NULL, NULL,       'draft','USD', 100, 't3');
         INSERT INTO invoice_payments (invoice_id, amount, paid_date, method)
             VALUES (1, 50, '2026-8-10', 'ach');
         UPDATE metadata SET value = '5' WHERE key = 'schema_version';",
    )
    .unwrap();

    run_migrations(&conn).unwrap();

    let date = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, Option<String>>(0)).unwrap();
    assert_eq!(date("SELECT issue_date   FROM invoices WHERE id = 1").as_deref(), Some("2026-08-07"));
    assert_eq!(date("SELECT due_date     FROM invoices WHERE id = 1").as_deref(), Some("2026-09-01"));
    assert_eq!(date("SELECT published_at FROM invoices WHERE id = 1").as_deref(), Some("2026-08-08"));
    assert_eq!(date("SELECT voided_at    FROM invoices WHERE id = 2").as_deref(), Some("2026-08-09"));
    assert_eq!(date("SELECT paid_date    FROM invoice_payments WHERE invoice_id = 1").as_deref(), Some("2026-08-10"));
    assert_eq!(
        date("SELECT issue_date FROM invoices WHERE id = 3").as_deref(),
        Some("March"),
        "a migration that rewrites what it cannot parse is guessing"
    );
    assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
}

#[test]
fn v6_is_idempotent_on_already_padded_dates() {
    let (_dir, conn) = test_db();
    conn.execute_batch(
        "INSERT INTO clients (id, name) VALUES (1, 'Acme');
         INSERT INTO invoices (id, number, client_id, issue_date, status, currency, total, token)
             VALUES (1, 1248, 1, '2026-08-07', 'draft', 'USD', 100, 't1');
         UPDATE metadata SET value = '5' WHERE key = 'schema_version';",
    )
    .unwrap();

    run_migrations(&conn).unwrap();
    run_migrations(&conn).unwrap();
    let issue: String = conn.query_row("SELECT issue_date FROM invoices WHERE id = 1", [], |r| r.get(0)).unwrap();
    assert_eq!(issue, "2026-08-07");
}
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib migrations 2>&1 | tail -20`

- [ ] **Step 3: Implement** — append to `MIGRATIONS` (version 6; `LATEST_VERSION` derives
      from the array's last entry, so nothing else needs touching):

```rust
    Migration {
        version: 6,
        description: "normalize stored invoice dates to zero-padded YYYY-MM-DD",
        up: |conn| {
            // `validate_date` pads on the way in from now on; these are the rows
            // written before it did. Parsing is chrono's, not SQL's, so the rule is
            // exactly the one the data layer applies — and a value that does not
            // parse is left untouched rather than guessed at.
            for (table, column) in [
                ("invoices", "issue_date"),
                ("invoices", "due_date"),
                ("invoices", "published_at"),
                ("invoices", "voided_at"),
                ("invoice_payments", "paid_date"),
            ] {
                normalize_date_column(conn, table, column)?;
            }
            Ok(())
        },
    },
```

and the helper beside `apply_migrations`:

```rust
/// Rewrite every parseable value in a date column to zero-padded `YYYY-MM-DD`.
fn normalize_date_column(conn: &Connection, table: &str, column: &str) -> Result<()> {
    let mut stmt =
        conn.prepare(&format!("SELECT id, {column} FROM {table} WHERE {column} IS NOT NULL"))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);

    for (id, value) in rows {
        let Ok(date) = chrono::NaiveDate::parse_from_str(&value, "%Y-%m-%d") else {
            continue;
        };
        let padded = date.format("%Y-%m-%d").to_string();
        if padded != value {
            conn.execute(
                &format!("UPDATE {table} SET {column} = ?1 WHERE id = ?2"),
                rusqlite::params![padded, id],
            )?;
        }
    }
    Ok(())
}
```

`table`/`column` are compile-time literals from the loop above, never user input, so the
`format!` is not an injection seam. The whole migration runs in the runner's savepoint.

- [ ] **Step 4: Verify.** All three builds. `cargo test --lib migrations` also re-proves
      `test_fresh_install_at_latest_version` and `test_idempotent_rerun` against the new
      `LATEST_VERSION` of 6.

---

### Task 4: End-to-end date regressions through the real binary

**Files:** modify `tests/cli_dispatch.rs`.

AC #1 says "entered anywhere (new/edit/pay, CLI or TUI)". The TUI half is covered by
Task 1 by construction — `invoice_manager` hands raw strings to `create_invoice` and
`record_payment`, which now pad — so this task is the CLI half, driven through the real
binary.

- [ ] **Step 1: Write the tests.** `init_with_client_and_invoice(&env)` already seeds
      client 1 and draft #1248 (`tests/cli_dispatch.rs:923`).

```rust
/// TASK-69 AC #1 through the binary: nothing between clap and the column pads
/// these but the data layer.
#[test]
fn unpadded_dates_round_trip_through_new_edit_and_pay_as_padded() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "new", "--client", "1", "--issue", "2026-8-7",
               "--due", "2026-9-1", "--item", "Consulting:1:100"])
        .assert()
        .success();

    let row = |sql: &str| -> Option<String> {
        env.db().query_row(sql, [], |r| r.get(0)).unwrap()
    };
    assert_eq!(row("SELECT issue_date FROM invoices WHERE number = 1249").as_deref(), Some("2026-08-07"));
    assert_eq!(row("SELECT due_date   FROM invoices WHERE number = 1249").as_deref(), Some("2026-09-01"));

    env.cmd()
        .args(["invoice", "edit", "1249", "--due", "2026-9-30"])
        .assert()
        .success();
    assert_eq!(row("SELECT due_date FROM invoices WHERE number = 1249").as_deref(), Some("2026-09-30"));

    env.cmd()
        .args(["invoice", "pay", "1249", "--amount", "25", "--date", "2026-9-2"])
        .assert()
        .success();
    assert_eq!(
        row("SELECT paid_date FROM invoice_payments WHERE invoice_id =
             (SELECT id FROM invoices WHERE number = 1249)").as_deref(),
        Some("2026-09-02")
    );

    // And it reads back padded everywhere a user looks.
    env.cmd().args(["invoice", "show", "1249"]).assert().success()
        .stdout(predicate::str::contains("2026-08-07").and(predicate::str::contains("2026-8-7").not()));
}

/// `invoice pay` never checked its date; the column took whatever was typed and
/// `refresh_status` used it as the reference day.
#[test]
fn invoice_pay_refuses_a_malformed_date() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "pay", "1248", "--amount", "10", "--date", "March"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid payment date"));

    let payments: i64 = env.db()
        .query_row("SELECT COUNT(*) FROM invoice_payments", [], |r| r.get(0))
        .unwrap();
    assert_eq!(payments, 0);
}
```

- [ ] **Step 2: Verify they fail against a stashed Task 1–3** (or reason it through: the
      assertions are on padded columns, which only Tasks 1–2 produce). Then verify they
      pass.

- [ ] **Step 3: Verify.** `cargo test --test cli_dispatch -- --test-threads=1` and the
      same with `--no-default-features`.

---

### Task 5: TASK-71 — thread the wall-clock day through `update_invoice`

**Files:** modify `src/invoicing/invoices.rs`, `src/cli/invoice.rs`, `src/main.rs`,
`src/server/routes/invoices.rs`.

**Interface produced:**

```rust
pub fn update_invoice(
    conn: &Connection,
    invoice_id: i64,
    update: &InvoiceUpdate,
    today: &str,
) -> Result<()>;
```

- [ ] **Step 1: Write the failing test** in `src/invoicing/invoices.rs`'s `mod tests`:

```rust
/// `update_invoice` passed the invoice's own issue date to `refresh_status` as
/// "today", so a due-date edit derived overdue against the wrong day.
///
/// `published_at` is set by hand because this is the only shape where an
/// *editable* invoice can reach the overdue branch at all: `mark_published`
/// would move the status off `draft` and `ensure_editable` would then refuse the
/// edit. That is exactly why the bug was invisible — and why it is a trap.
#[test]
fn a_due_date_edit_derives_status_against_today_not_the_issue_date() {
    let (_d, conn) = test_conn();
    let id = seed_draft(&conn); // issued 2026-08-04, still `draft`
    conn.execute("UPDATE invoices SET published_at = '2026-08-05' WHERE id = ?1", [id]).unwrap();

    update_invoice(
        &conn,
        id,
        &InvoiceUpdate { due_date: Some(Some("2026-08-06".into())), ..Default::default() },
        "2026-08-20",
    )
    .unwrap();

    assert_eq!(
        get_invoice(&conn, id).unwrap().status,
        "overdue",
        "derived against the issue date (2026-08-04), which is not past the due date"
    );
}
```

- [ ] **Step 2: Verify it fails** — compile error first (arity), then `sent` vs `overdue`
      once the parameter exists but is still ignored.

- [ ] **Step 3: Implement.** Add the parameter and replace the tail of `update_invoice`
      (`src/invoicing/invoices.rs:454-458`):

```rust
    tx.commit()?;
    refresh_status(conn, invoice_id, today)?;
    Ok(())
```

deleting the `let issue_date = update.issue_date.clone().unwrap_or(...)` block entirely.

- [ ] **Step 4: Update the two production callers.**

`src/cli/invoice.rs` — `edit` takes `today: &str` (added last, after `items`, matching
`void(number, yes, today)`), and passes it:

```rust
    update_invoice(&conn, invoice.id, &update, today)?;
```

`src/main.rs` — the `InvoiceCommands::Edit` arm passes `&cli::today()`, the way `Void`,
`Send`, `Sync` and `Aging` already do:

```rust
            } => cli::invoice::edit(
                number, issue_date, due_date, clear_due, currency, notes, terms, &items,
                &cli::today(),
            ),
```

`src/server/routes/invoices.rs`'s `update` handler — resolve the day before
`with_conn_api` and move it into the closure, the shape `sync` uses at line 419:

```rust
    let today = crate::cli::today();
    let detail = with_conn_api(&state, move |conn| {
        …
        inv::update_invoice(conn, invoice.id, &update, &today)
            .map_err(|e| enrich_conflict(e, &invoice, paid))?;
        …
    })
    .await?;
```

- [ ] **Step 5: Update the in-module test call sites.** ~18 calls in
      `src/invoicing/invoices.rs` gain a trailing literal. Use `"2026-08-11"` unless the
      test is about a specific day; `cargo test --lib` names every one that still does not
      compile.

- [ ] **Step 6: Verify.** All three builds — the server route only compiles under
      `--features serve`, so the third command is the one that proves that call site.
      Clippy, fmt.

---

### Task 6: TASK-63 — invoice commands against an encrypted database

**Files:** modify `tests/cli_dispatch.rs`.

`TestEnv::encrypt()` asserts the database really is unreadable afterwards, `TEST_TIMEOUT`
(60s) bounds a run that reaches the `rpassword` prompt, and `cmd()` clears all nine
`NIGEL_*` invoicing variables, so the launch sync cannot reach Stripe.

- [ ] **Step 1: Write the tests**, beside `recategorize_works_on_encrypted_db_via_env_password`:

```rust
/// AC #1. A read *and* a write: unlocking for a SELECT and unlocking for an
/// INSERT are the same key, but a read-only regression would slip past a test
/// that only lists.
#[test]
fn invoice_and_client_commands_work_on_encrypted_db_via_env_password() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    env.encrypt("hunter2");

    env.cmd()
        .args(["invoice", "list"])
        .env("NIGEL_DB_PASSWORD", "hunter2")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("1248").and(predicate::str::contains("Acme Co")));

    env.cmd()
        .args(["client", "add", "Globex", "--email", "ap@globex.test"])
        .env("NIGEL_DB_PASSWORD", "hunter2")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    env.cmd()
        .args(["client", "list"])
        .env("NIGEL_DB_PASSWORD", "hunter2")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("Globex").and(predicate::str::contains("Acme Co")));
}

/// AC #2. The stderr predicate is the assertion that matters: reaching the prompt
/// with no terminal errors with ENXIO, which satisfies `.failure()` on its own.
/// The timeout is only a backstop for a run that inherits a tty and blocks.
#[test]
fn invoice_list_fails_fast_on_wrong_env_password() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    env.encrypt("hunter2");

    env.cmd()
        .args(["invoice", "list"])
        .env("NIGEL_DB_PASSWORD", "wrong-password")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(predicate::str::contains("NIGEL_DB_PASSWORD"));
}
```

- [ ] **Step 2: Verify they pass** — they are characterization tests for a path that
      already works; nothing in `src/` changes for this task. If either fails, that is the
      bug TASK-63 suspected and it is fixed here rather than in a follow-up.

- [ ] **Step 3: Verify** both `cargo test --test cli_dispatch -- --test-threads=1` and the
      `--no-default-features` build.

---

### Task 7: TASK-70 — record the decision that `clients.name` stays advisory

**Files:** modify `src/invoicing/import_invoiceshelf.rs` (test only), `docs/invoicing.md`,
`CLAUDE.md`.

No schema change. The spec's "Decision 5" is the argument; this task is the part of it
that lives in the repo.

- [ ] **Step 1: Write the pinning test** in `src/invoicing/import_invoiceshelf.rs`'s
      `mod tests`, using the existing `dest_conn()` and `empty_source()` helpers:

```rust
/// The InvoiceShelf import inserts customers with raw SQL, deliberately
/// bypassing `add_client`'s duplicate-name check: it is a faithful copy of
/// another system's customer table, and that system does not guarantee unique
/// names. This is the test a `UNIQUE` index on `clients.name` would break, which
/// is why it is here — see the TASK-70 note in CLAUDE.md.
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
        .query_row("SELECT COUNT(*) FROM clients WHERE name = 'Acme Co'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(same_name, 2, "the import mirrors its source; it does not merge or rename");
}
```

- [ ] **Step 2: Verify it passes** (characterization — it is the current behavior, pinned).

- [ ] **Step 3: Document in `docs/invoicing.md`**, in the "Clients" section, directly under
      the existing paragraph beginning "A name is required and must be unique":

> That rule lives in the data layer (`add_client`/`update_client`), not in the schema:
> `clients.name` carries no `UNIQUE` index, matching `accounts.name` and
> `categories.name`. Two requests racing each other in the web UI can therefore both
> pass the check and both insert; the result is two clients with one name, which
> nothing resolves by name and which you can fix by renaming one on the clients
> screen. The InvoiceShelf import deliberately does not apply the rule at all — it
> copies your old customer list as it stands.

- [ ] **Step 4: Document in `CLAUDE.md`**, as a new Key Design Constraints bullet:

> - `clients.name` uniqueness is **advisory**: `clients::name_taken` refuses a duplicate in
>   `add_client`/`update_client`, and the column carries no `UNIQUE` index (TASK-70,
>   decided). The schema constrains machine-generated identity (`invoices.number`,
>   `invoices.token`, `invoice_payments.stripe_checkout_session_id`, `csv_profiles.name`)
>   and leaves user-typed names to the data layer, as `accounts.name` and
>   `categories.name` already do — a `UNIQUE` index there would be wrong for categories,
>   which soft-delete. Nothing resolves a client by name, and
>   `import_invoiceshelf` intentionally mirrors a source that does not guarantee unique
>   names, so a constraint would abort a one-time migration over a cosmetic duplicate.
>   Two racing web writes can still both insert; the cure is a rename on the clients
>   screen.

- [ ] **Step 5: Verify.** `cargo test --lib import_invoiceshelf`, and re-read the
      `docs/invoicing.md` paragraph in place so it does not contradict the sentence above
      it.

---

### Task 8: Documentation for the date and clock changes

Per CLAUDE.md's Documentation Policy the work is not complete until these land. No
`README.md` change: no command, flag, or user-visible output is added.

- [ ] **Step 1: `CLAUDE.md` Architecture, the Invoicing bullet** — amend the parenthetical
      listing the data layer's validators so `validate_date` is described where someone
      will look for it, beside `validate_payment_method` and `validate_items`:
      *`validate_date` (called by `create_invoice`, `update_invoice`, `record_payment` and
      `void_invoice`, returning the **normalized** zero-padded `YYYY-MM-DD` the way
      `validate_currency` returns the uppercased code — chrono accepts `2026-8-7`, and
      `is_overdue` compares ISO strings, so an unpadded date would never read as past
      due)*.

- [ ] **Step 2: `CLAUDE.md` Architecture, the Migrations bullet** — append
      `; v6 normalizes stored invoice dates (issue/due/published/voided/paid) to
      zero-padded ISO, leaving unparseable values untouched`.

- [ ] **Step 3: `CLAUDE.md` Key Design Constraints** — add:
      *Every invoicing date is normalized by the writer, never by the caller:
      `validate_date` returns the padded form and the four functions that write a date
      column store that, so the CLI, the TUI and the API cannot disagree, and
      `refresh_status`'s string comparison stays correct. `record_payment` validates its
      own `paid_date` for the reason it validates its own method. The HTTP API stays
      stricter than the CLI — `routes::reports::parse_date` requires ten characters, so
      `2026-4-1` is a 400 over HTTP and a normalized `2026-04-01` at a terminal.*

- [ ] **Step 4: `CLAUDE.md` Key Design Constraints** — add:
      *`update_invoice` takes the reference day as a parameter like `void_invoice` and
      `record_payment`; nothing under `src/invoicing/` reads the clock, so every derived
      status is deterministic in tests and correct against the wall clock in production.*

- [ ] **Step 5: `docs/invoicing.md`** — two small edits: in "Recording payments", note that
      `--date` must be `YYYY-MM-DD` and that an unpadded date is stored padded; in
      "Status", note that overdue is derived against the day the command runs.

- [ ] **Step 6: Verify.** `git diff --stat` shows `CLAUDE.md` and `docs/invoicing.md`
      touched and nothing under `web/`.

---

## Final verification

- [ ] `cargo test -- --test-threads=1`
- [ ] `cargo test --no-default-features -- --test-threads=1`
- [ ] `cargo test --no-default-features --features serve -- --test-threads=1`
- [ ] `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`
- [ ] `git diff --stat` — no file under `web/`, no new file under `src/`, and
      `src/cli/invoice_manager.rs` changed only where the `validate_date` return value is
      discarded.
- [ ] Manual, against a scratch data dir:

```bash
cargo run -- init --data-dir /tmp/nigel-69 && cargo run -- client add "Acme Co"
cargo run -- invoice new --client 1 --issue 2026-8-7 --due 2026-9-1 --item "Work:1:100"
cargo run -- invoice show 1248            # both dates padded
cargo run -- invoice pay 1248 --amount 10 --date March   # refused, names the field
sqlite3 /tmp/nigel-69/nigel.db "SELECT issue_date, due_date FROM invoices;"
```

- [ ] Migration smoke test on a copy of a real database: `SELECT schema_version` reads 6,
      and no date column holds an unpadded value that chrono can parse.

## Acceptance criteria mapping

| Task | AC | Verified by |
|---|---|---|
| 69 | #1 unpadded date entered anywhere is stored zero-padded | Task 1 `an_unpadded_issue_or_due_date_is_stored_padded`, `an_unpadded_date_edited_onto_a_draft_is_stored_padded`; Task 2 `record_payment_stores_an_unpadded_date_padded`, `void_invoice_stores_an_unpadded_date_padded`; Task 4 `unpadded_dates_round_trip_through_new_edit_and_pay_as_padded` (CLI); the TUI inherits it — the forms hand raw strings to the data layer, which is now the padder |
| 69 | #2 `refresh_status` and `ar_aging` behave identically padded or unpadded | Task 1 `overdue_derives_the_same_whether_the_due_date_was_typed_padded_or_not`, `aging_buckets_and_prints_the_same_whether_the_due_date_was_typed_padded_or_not`; Task 3 for rows written before the fix |
| 71 | #1 `refresh_status` always called with the wall-clock today, pinned on the due-date-edit path | Task 5 `a_due_date_edit_derives_status_against_today_not_the_issue_date`, plus the two production call sites passing `cli::today()` |
| 63 | #1 an invoice/client command against an encrypted DB via `NIGEL_DB_PASSWORD` | Task 6 `invoice_and_client_commands_work_on_encrypted_db_via_env_password` |
| 63 | #2 a wrong password fails with the documented error rather than hanging | Task 6 `invoice_list_fails_fast_on_wrong_env_password` (stderr names the variable; `TEST_TIMEOUT` catches a hang) |
| 70 | #1 either a `UNIQUE` index with a migration, or advisory-only documented as deliberate | Task 7 — advisory-only, with the rationale in `CLAUDE.md`, the user-facing consequence in `docs/invoicing.md`, and `two_source_customers_with_the_same_name_import_as_two_clients` pinning the case a constraint would break |
