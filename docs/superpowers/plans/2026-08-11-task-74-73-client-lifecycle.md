# Client delete and archive Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `nigel client delete` and a TUI delete that refuse exactly what the web
already refuses, plus archive/unarchive answered consistently by the CLI list,
the TUI client manager and `GET /api/clients` — per
`docs/superpowers/specs/2026-08-11-task-74-73-client-lifecycle-design.md`.

**Architecture:** One migration adds `clients.archived_at`, a nullable
timestamp, following `voided_at`'s derived-state precedent.
`invoicing::clients` gains `ClientScope` (an explicit `Active`/`All` on
`list_clients`), `archive_client`, `unarchive_client` and
`ensure_client_active`, which `invoices::create_invoice` calls beside the
`ensure_client_exists` it already calls. Delete introduces **no** guard: the CLI
and the TUI call `delete_blocker`/`delete_client` and print their sentences. The
CLI's confirmation is `confirm_void`'s logic, extracted so both destructive
commands share one implementation and one refusal shape. The web gains
`?includeArchived`, `POST /api/clients/{id}/archive` and `…/unarchive`, and the
clients screen gains a toggle, a row action and a badge.

**Tech Stack:** Rust, rusqlite, clap (derive), ratatui/crossterm, axum (`serve`
feature), Lit + Web Awesome, vitest + axe.

## Global Constraints

- After every task: `cargo test -- --test-threads=1`,
  `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` clean.
- **Every task must also pass without the `pdf` feature** —
  `cargo test --no-default-features --features gusto -- --test-threads=1` — and
  with no default features: `cargo test --no-default-features -- --test-threads=1`.
- **No new refusal logic for delete.** Both new surfaces call
  `clients::delete_blocker` and `clients::delete_client`, and every refusal
  sentence is `NigelError`'s `Display`. A string literal containing "Cannot
  delete" anywhere outside `error.rs` fails review.
- **Archiving changes no figure.** `list_invoices`, `ar_aging_detail`,
  `client_summary`'s `outstanding` and every report are untouched, and a test
  pins that an archived client's name still appears in the invoice list and the
  aging report.
- Every visual change ships through `@nigel/ui` with a co-located preview and an
  axe run (CLAUDE.md's Component-First UI Workflow).
- **Migration numbering — read before Task 1.** `MIGRATIONS` is at v5 on
  `main` today; Stream 1 has a v6 in flight (date normalization, TASK-69) and
  has recommended *against* the `clients.name` `UNIQUE` index, so nothing else
  will touch `clients`. A gap is *not* safe: the
  runner applies `version > current`, so a database stamped v7 would skip a v6
  that merged afterwards. Therefore: write this migration as the **next free
  number on your branch's base**, and if another stream's migration has landed
  by the time you rebase, **renumber yours to be last** — the diff is the version
  literal, the `LATEST_VERSION` assertion in the tests, and nothing else. Never
  reuse a number and never leave a gap.

---

### Task 1: The column, the model, and the scope

**Files:** modify `src/migrations.rs`, `src/models.rs`,
`src/invoicing/clients.rs`.

**Interface produced** (consumed by every later task):

```rust
pub struct Client { /* … */ pub archived_at: Option<String> }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientScope { Active, All }

pub fn list_clients(conn: &Connection, scope: ClientScope) -> Result<Vec<Client>>;
```

- [ ] **Step 1: Write failing tests.** In `migrations.rs`'s `mod tests`, beside
  the existing version tests:

```rust
#[test]
fn clients_gain_an_archived_at_column() {
    let (_dir, conn) = test_db();
    run_migrations(&conn).unwrap();
    let has: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('clients') WHERE name = 'archived_at'",
        [], |r| r.get(0)).unwrap();
    assert!(has);
    assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
}

#[test]
fn every_existing_client_starts_active() {
    // insert a client at v5, run migrations, assert archived_at IS NULL
}

#[test]
fn the_archive_migration_is_replayable() {
    // run_migrations twice; the second is a no-op and does not error
}
```

  and in `clients.rs`'s `mod tests`:

```rust
#[test]
fn the_default_scope_hides_archived_clients_and_all_shows_them() {
    let (_d, conn) = test_conn();
    let acme = add_client(&conn, "Acme Co", None, None, None).unwrap();
    add_client(&conn, "Globex", None, None, None).unwrap();
    archive_client(&conn, acme, "2026-08-11").unwrap();

    let active = list_clients(&conn, ClientScope::Active).unwrap();
    assert_eq!(active.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["Globex"]);

    let all = list_clients(&conn, ClientScope::All).unwrap();
    assert_eq!(all.len(), 2);
    // Order is by name in both scopes: an archived row does not move.
    assert_eq!(all[0].name, "Acme Co");
    assert_eq!(all[0].archived_at.as_deref(), Some("2026-08-11"));
    assert!(all[1].archived_at.is_none());
}

#[test]
fn get_client_answers_an_archived_client_normally() {
    // archive, then get_client succeeds and carries the timestamp — archive is
    // not a soft delete and nothing stops reading the row
}
```

- [ ] **Step 2: Verify they fail.**
  `cargo test --lib -- migrations clients 2>&1 | tail -20`

- [ ] **Step 3: Implement the migration.** Append to `MIGRATIONS` with the next
  free version (see the constraint above), probing first exactly as v5 does:

```rust
Migration {
    version: 6, // renumber on rebase if another stream's migration landed first
    description: "add archived_at to clients so a finished client can leave the list",
    up: |conn| {
        let has_column: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('clients') WHERE name = 'archived_at'",
            [], |r| r.get(0))?;
        if !has_column {
            conn.execute_batch("ALTER TABLE clients ADD COLUMN archived_at TEXT")?;
        }
        Ok(())
    },
},
```

  No backfill: every existing client is active, which `NULL` already says.

- [ ] **Step 4: Implement the model and the reads.** `Client.archived_at:
  Option<String>` (serde gives `archivedAt` from the existing
  `rename_all = "camelCase"`); add the column to `get_client`'s and
  `list_clients`' `SELECT` lists and row mappings; add `ClientScope` with a
  private `fn where_clause(self) -> &'static str` so the two arms are one line
  each and the `ORDER BY name` is shared.

- [ ] **Step 5: Fix every call site** so the tree compiles: `cli/client.rs`,
  `cli/client_manager.rs` (`new` and `reload`), `server/routes/clients.rs`,
  `src/cli/demo.rs`, `src/server/testutil.rs`, and any test constructing a
  `Client` literal — `grep -rn "list_clients(\|Client {" src/ web/` finds them.
  Each takes the scope its surface wants; nothing gets a default.

- [ ] **Step 6: Verify.** All three feature builds, clippy, fmt.

---

### Task 2: Archive, unarchive, and the refusal a new invoice gets

**Files:** modify `src/invoicing/clients.rs`, `src/invoicing/invoices.rs`.

**Interface produced:**

```rust
pub fn archive_client(conn: &Connection, id: i64, on: &str) -> Result<()>;
pub fn unarchive_client(conn: &Connection, id: i64) -> Result<()>;
pub fn ensure_client_active(conn: &Connection, id: i64) -> Result<()>;
```

- [ ] **Step 1: Write failing tests** in `clients.rs`:

```rust
#[test]
fn archiving_is_idempotent_and_keeps_the_first_timestamp() {
    // archive on 2026-08-11, archive again on 2026-09-01
    // → archived_at is still 2026-08-11
}

#[test]
fn unarchiving_clears_the_timestamp() { /* … */ }

#[test]
fn archiving_touches_nothing_but_the_flag() {
    // seed a client with 2 invoices and a payment, archive, then assert:
    // list_invoices still returns both rows with the client's name,
    // client_summary's outstanding is unchanged, and delete_blocker still
    // counts 2 — AC #4 in one test
}

#[test]
fn archiving_a_missing_client_is_not_found() {
    assert_eq!(archive_client(&conn, 99, "2026-08-11").unwrap_err().to_string(),
               "Client not found: id 99");
}
```

  and in `invoices.rs`:

```rust
#[test]
fn a_new_invoice_for_an_archived_client_is_refused_naming_the_reason() {
    let (_d, conn) = test_conn();
    let id = add_client(&conn, "Acme Co", None, None, None).unwrap();
    archive_client(&conn, id, "2026-08-11").unwrap();

    let err = create_invoice(&conn, id, "2026-08-12", None, "USD", &items(), None, None)
        .map(|_| ()).unwrap_err();
    assert_eq!(conflict_code(&err), "client_archived");
    assert!(err.to_string().contains("Acme Co"), "got: {err}");
    assert_eq!(list_invoices(&conn, None, None).unwrap().len(), 0, "nothing was written");
}

#[test]
fn unarchiving_makes_the_client_invoiceable_again() { /* … */ }

#[test]
fn an_archived_clients_existing_invoices_stay_in_every_list() {
    // seed + publish two invoices, archive the client, then assert the name is
    // present in list_invoices and in ar_aging_detail — AC #3
}
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib invoicing 2>&1 | tail -20`

- [ ] **Step 3: Implement** in `clients.rs`:

```rust
/// Take a client out of the working list without touching a single invoice.
///
/// Idempotent by `AND archived_at IS NULL`: archiving twice keeps the date the
/// client actually stopped being billed rather than the date somebody pressed
/// the key again.
pub fn archive_client(conn: &Connection, id: i64, on: &str) -> Result<()> {
    ensure_client_exists(conn, id)?;
    conn.execute(
        "UPDATE clients SET archived_at = ?2 WHERE id = ?1 AND archived_at IS NULL",
        rusqlite::params![id, on],
    )?;
    Ok(())
}

/// A client that is not archived, or the refusal a new invoice gets.
///
/// A `Conflict` rather than an `Invalid` for `client_missing_email`'s reason:
/// it is a fact about the client record a screen can act on, so over HTTP it is
/// a 409 naming the client, not a 400 nobody can build a button from.
pub fn ensure_client_active(conn: &Connection, id: i64) -> Result<()> {
    let client = get_client(conn, id)?;
    if client.archived_at.is_none() { return Ok(()); }
    Err(NigelError::Conflict {
        code: "client_archived",
        message: format!("client '{}' is archived — unarchive it before invoicing", client.name),
    })
}
```

  `unarchive_client` is the same shape with `archived_at = NULL` and no date.

- [ ] **Step 4: Call the guard** in `create_invoice`, immediately after the
  existing `ensure_client_exists(conn, client_id)?` — before the transaction
  opens, so a refusal writes nothing.

- [ ] **Step 5: Verify.** All three feature builds, clippy, fmt.

---

### Task 3: The CLI

**Files:** modify `src/cli/mod.rs`, `src/cli/client.rs`, `src/main.rs`.

**Interfaces:**

```rust
// src/cli/mod.rs — shared by `invoice void` and `client delete`
pub(crate) fn confirm_or_refuse(question: &str, refusal: &str, yes: bool) -> Result<bool>;

// src/cli/client.rs
pub fn delete(id: i64, yes: bool) -> Result<()>;
pub fn archive(id: i64, today: &str) -> Result<()>;
pub fn unarchive(id: i64) -> Result<()>;
pub fn list(all: bool) -> Result<()>;
pub fn format_client_list(clients: &[Client]) -> String;  // unchanged signature
```

- [ ] **Step 1: Write failing tests** in `cli/client.rs`'s `mod tests` and
  `cli/invoice.rs`'s:

```rust
// cli/invoice.rs — the extraction must not move a byte of the void wording
#[test]
fn the_void_confirmation_wording_is_unchanged_by_the_shared_helper() {
    // non-TTY, yes = false → Err whose message is exactly
    // "Refusing to void invoice #1248 without confirmation. Pass --yes."
}

// cli/client.rs
#[test]
fn format_client_list_has_no_archived_column_when_nothing_is_archived() {
    // byte-for-byte identical to the existing assertion — the default list and
    // the committed clients.txt fixture must not move
}

#[test]
fn format_client_list_grows_an_archived_column_when_a_row_is_archived() {
    let out = format_client_list(&[client(1, "Acme Co", Some("ap@acme.test")),
                                   archived(2, "Globex", "2026-08-11")]);
    assert!(out.contains("Archived"));
    assert!(out.contains("2026-08-11"));
}
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib cli:: 2>&1 | tail -20`

- [ ] **Step 3: Extract the confirmation.** Move `confirm_void`'s body into
  `cli/mod.rs`:

```rust
/// Ask before something irreversible, or refuse when nobody can be asked.
///
/// `invoice void` and `client delete` share this so the two destructive
/// commands behave identically on a pipe: without `--yes` and without a
/// terminal, the command fails rather than defaulting either way.
pub(crate) fn confirm_or_refuse(question: &str, refusal: &str, yes: bool) -> Result<bool> {
    if yes { return Ok(true); }
    if !std::io::stdin().is_terminal() {
        return Err(NigelError::Other(refusal.to_string()));
    }
    print!("{question} ");
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(answer.trim().eq_ignore_ascii_case("y"))
}
```

  and rewrite `confirm_void` as a two-line caller passing `"Void it? [y/N]"` and
  its existing refusal sentence.

- [ ] **Step 4: Implement the three commands** in `cli/client.rs`:

```rust
pub fn delete(id: i64, yes: bool) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let client = get_client(&conn, id)?;

    // Asked before the prompt: a client that cannot be deleted is never offered
    // a confirmation, because there is nothing to confirm.
    if let Some(block) = delete_blocker(&conn, id)? {
        eprintln!("{}", NigelError::Blocked(block));
        eprintln!("Run `nigel client show {id}` to see them.");
        return Err(/* the same Blocked error, so the exit status is non-zero */);
    }

    println!("Delete client #{id} {}? This cannot be undone.", client.name);
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
```

  Build the `Blocked` error once and both print and return it, rather than
  formatting the sentence twice. `archive`/`unarchive` print
  `Archived client {id}: {name}` / `Restored client {id}: {name}`, and `show`
  prints an `Archived:` line in the label block when the timestamp is set.

- [ ] **Step 5: The list column.** `format_client_list` derives it:
  `let show_archived = clients.iter().any(|c| c.archived_at.is_some());`. When
  false the header and the rows are exactly what they are today — which is what
  keeps `clients.txt` and the byte-for-byte test valid.

- [ ] **Step 6: clap and dispatch.** In `ClientCommands`:

```rust
/// Delete a client. Refused while any invoice bills them, of any status.
Delete { id: i64, /// Skip the confirmation prompt
                  #[arg(long)] yes: bool },
/// Archive a client: hide it from the list without deleting anything.
Archive { id: i64 },
/// Bring an archived client back to the working list.
Unarchive { id: i64 },
/// List all clients.
List { /// Include archived clients
       #[arg(long)] all: bool },
```

  and the four arms in `main.rs`, `archive` taking `cli::today()` the way
  `invoice void` does.

- [ ] **Step 7: Verify.** All three feature builds, clippy, fmt, and
  `cargo run -- client delete --help`.

---

### Task 4: The TUI client manager

**Files:** modify `src/cli/client_manager.rs`.

- [ ] **Step 1: Write failing tests** in its `mod tests` (the module already has
  a `mgr`/`conn` harness and drives `handle_key` directly):

```rust
#[test] fn d_on_a_client_with_no_invoices_opens_the_confirmation() { /* Screen::ConfirmDelete */ }
#[test] fn y_deletes_and_reloads_the_list() { /* status "Deleted client: Globex", list empty */ }
#[test] fn n_cancels_and_the_client_is_still_there() { /* … */ }

#[test]
fn d_on_a_client_with_invoices_never_opens_the_confirmation() {
    // seed an invoice, press 'd'
    // status_message == "Cannot delete: client has 1 invoice"
    // and the screen is still the list — the account_manager precedent
}

#[test]
fn x_archives_and_unarchives_the_selected_client() { /* status names the client both ways */ }

#[test]
fn archived_clients_are_hidden_until_shift_a() {
    // archive one of two, list shows one; press 'A', list shows two and the
    // archived row is marked; press 'A' again, back to one
}
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib client_manager 2>&1 | tail -20`

- [ ] **Step 3: Implement.** `Screen::ConfirmDelete` beside `List`/`Add`/`Edit`,
  drawn as the account manager draws it (the list underneath, a confirmation
  line, footer `y=confirm  n=cancel`); a `show_archived: bool` on
  `ClientManager` that `reload` reads into the `ClientScope`; the three keys in
  `handle_list_key`; `client_row` appending ` (archived)` after the name; the
  footer string.

- [ ] **Step 4: Verify.** All three feature builds, clippy, fmt, and a manual
  pass through `cargo run -- ` → `k`.

---

### Task 5: The HTTP API

**Files:** modify `src/server/routes/clients.rs`.

- [ ] **Step 1: Write failing tests** in its test module:

```rust
#[tokio::test] async fn the_client_list_hides_archived_clients_by_default() { /* … */ }
#[tokio::test] async fn include_archived_shows_them_with_the_timestamp() { /* archivedAt non-null */ }
#[tokio::test] async fn an_unrecognised_include_archived_value_is_a_400() { /* ?includeArchived=maybe */ }
#[tokio::test] async fn archive_and_unarchive_answer_the_refreshed_client() { /* 200, archivedAt set/null */ }
#[tokio::test] async fn archiving_an_unknown_client_is_404_with_a_reason() { /* client_not_found */ }
#[tokio::test]
async fn an_archived_client_cannot_be_given_a_new_invoice() {
    // POST /api/invoices → 409, details.reason == "client_archived"
}
```

- [ ] **Step 2: Verify they fail.**
  `cargo test --features serve routes::clients 2>&1 | tail -30`

- [ ] **Step 3: Implement.**

```rust
.route("/clients/{id}/archive", post(archive))
.route("/clients/{id}/unarchive", post(unarchive))
```

  and a `#[derive(Deserialize)] struct ListQuery { include_archived: Option<String> }`
  parsed the strict way the report routes parse parameters: `"true"`/`"false"`
  only, anything else a 400 in the error envelope. Both POST handlers go through
  `with_conn_api` with `not_found_because(e, "client_not_found")` and answer
  `get_client`'s refreshed row.

- [ ] **Step 4: Verify.** `cargo test --features serve -- --test-threads=1`.

---

### Task 6: The SPA

**Files:** modify `web/apps/app/src/api/{types,client}.ts`,
`web/apps/app/src/screens/{clients,invoicing-errors}.ts`,
`web/packages/ui/src/components/` (the badge), and the invoicing fixtures.

- [ ] **Step 1: The api seam.** `Client.archivedAt: string | null`;
  `getClients(includeArchived?: boolean)`; `archiveClient(id)`;
  `unarchiveClient(id)`. The guard test that forbids `/api/` literals outside
  `src/api` means the two new paths belong in `client.ts` and nowhere else.

- [ ] **Step 2: The badge component** in `@nigel/ui` — `wc-archived-badge` (or a
  `badge` cell variant on `wc-manager-table` if that is the smaller change), with
  a co-located `.preview.ts` covering its states and a `.test.ts` calling
  `describePreviewA11y(preview)`. Non-negotiable per CLAUDE.md.

- [ ] **Step 3: The screen.** A "Show archived" toggle above the table writing
  `#/clients?archived=1` (a filtered list is a URL); an `archive`/`unarchive`
  row action in `ACTIONS` whose label depends on the row; the badge in the name
  cell; `invoicingGuardrailMessage` gaining `client_archived`. Every mutation
  refetches the list — no optimistic splicing, the managers' rule.

- [ ] **Step 4: Tests.** A screen test per behaviour with `FakeApiClient`
  (default list hides archived; the toggle refetches with the parameter; the
  action calls the right method and refetches; the badge renders). Then the
  a11y run comes free from the preview.

- [ ] **Step 5: Recapture the invoicing fixtures.** `Client` gained a field, so
  the committed JSON is now out of step and the non-ignored guard test fails:

```bash
cargo test --features serve capture_web_invoicing_fixtures -- --ignored
```

  Seed one archived client in `server::testutil::seed_invoicing` first, so the
  fixture actually covers the new state. `clients.txt` should come back
  **unchanged** — if it did not, `format_client_list` grew a column on a list
  with nothing archived and Task 3 Step 5 is wrong.

- [ ] **Step 6: Verify.** From `web/`: `npm run typecheck`, `npm test`,
  `npm run lint`, `npm run build`. Then `cargo test -- --test-threads=1`.

---

### Task 7: End-to-end and documentation

**Files:** modify `tests/cli_dispatch.rs`, `docs/invoicing.md`, `CLAUDE.md`,
`README.md`.

- [ ] **Step 1: Integration tests.**

```rust
#[test] fn client_delete_removes_a_client_with_no_invoices() { /* --yes */ }
#[test] fn client_delete_refuses_a_client_with_invoices_and_points_at_them() {
    // stderr contains "Cannot delete: client has 1 invoice" and "nigel client show"
}
#[test] fn client_delete_without_yes_on_a_pipe_refuses_rather_than_guessing() { /* … */ }
#[test] fn archived_clients_are_hidden_from_client_list_and_shown_by_all() { /* … */ }
#[test] fn a_new_invoice_for_an_archived_client_is_refused_on_the_cli() { /* … */ }
```

- [ ] **Step 2: `docs/invoicing.md`** — extend the Clients section with delete
  (what it refuses and why every status counts) and a new "Archiving a client"
  subsection: the two commands, `list --all`, that archived clients keep
  appearing wherever their invoices do, that a new invoice for one is refused,
  and that archiving is not deletion.

- [ ] **Step 3: `CLAUDE.md`** — the **Client Manager** architecture bullet is
  now wrong in two places (it says there is no delete, and gives the reasoning
  for not having one): rewrite it to describe delete-with-confirmation, the
  archive keys, and the hidden-by-default list. Add a Key Design Constraints
  bullet: *archive is a timestamp on `clients`, hidden by default on all three
  lists, invisible to every invoice query and every report; delete is refused
  while any invoice of any status names the client, and all three front ends
  print `delete_blocker`'s sentence.* Add the new commands to the Commands block.

- [ ] **Step 4: `README.md`** — the invoicing command list gains
  `nigel client delete`, `archive`, `unarchive`.

- [ ] **Step 5: Verify.** `git diff --stat` shows all three docs touched.

---

## Final verification

- [ ] `cargo test -- --test-threads=1`
- [ ] `cargo test --no-default-features --features gusto -- --test-threads=1`
- [ ] `cargo test --no-default-features -- --test-threads=1`
- [ ] `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`
- [ ] From `web/`: `npm run typecheck`, `npm test`, `npm run lint`, `npm run build`
- [ ] `git diff web/apps/app/src/__fixtures__/invoicing/clients.txt` is empty.
- [ ] `grep -rn "Cannot delete" src/ | grep -v error.rs` returns nothing.
- [ ] `git diff src/reports.rs src/invoicing/invoices.rs` shows no change to any
      query that sums or lists money — only the `ensure_client_active` call.
- [ ] Rebase onto `main` and re-check the migration version against
      `LATEST_VERSION`; renumber if another stream's migration landed first.
- [ ] Manual: `cargo run -- ` → `k`, archive a client, confirm it disappears,
      press `A`, confirm it comes back marked, press `d` on a client with
      invoices and read the status line.

## Acceptance criteria mapping

| Task | AC | Verified by |
|---|---|---|
| 74 | #1 delete a client with no invoices | Task 7 `client_delete_removes_a_client_with_no_invoices` |
| 74 | #2 refuses any status, same words, names the count | Task 7 `client_delete_refuses_a_client_with_invoices_and_points_at_them`; the existing `a_void_or_paid_invoice_still_blocks_the_delete` |
| 74 | #3 the refusal points at the invoices | same test (`nigel client show`) |
| 74 | #4 TUI delete with a confirmation, same terms | Task 4's four delete tests |
| 74 | #5 no new guard logic | the `grep -rn "Cannot delete"` check; both surfaces call `delete_blocker`/`delete_client` |
| 74 | #6 confirmation on the CLI, consistent with the others | Task 3 `the_void_confirmation_wording_is_unchanged_by_the_shared_helper`; Task 7 `client_delete_without_yes_on_a_pipe_refuses_rather_than_guessing` |
| 73 | #1 archive and unarchive | Task 2's idempotence and clearing tests |
| 73 | #2 hidden by default on CLI, TUI and web | Task 1 `the_default_scope_hides_archived_clients_and_all_shows_them`; Task 4 `archived_clients_are_hidden_until_shift_a`; Task 5 `the_client_list_hides_archived_clients_by_default` |
| 73 | #3 still visible wherever their invoices are | Task 2 `an_archived_clients_existing_invoices_stay_in_every_list` |
| 73 | #4 not deletion | Task 2 `archiving_touches_nothing_but_the_flag` |
| 73 | #5 no new invoice, or the refusal names the reason | Task 2 `a_new_invoice_for_an_archived_client_is_refused_naming_the_reason`; Task 5's 409; Task 7's CLI test |
| 73 | #6 the list can include archived | Task 3's `--all`, Task 5's `includeArchived`, Task 4's `A` |
