# Invoice duplication and recurring schedules Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Duplicate any invoice into a fresh draft from the CLI, the TUI and the
browser, and let a stored schedule do the same on a cycle through one idempotent
command built for cron and launchd — drafting by default, sending only where a
schedule opts in.

**Architecture:** One shape reader in the data layer,
`invoices::invoice_shape`, answers what an invoice *is* independent of what
state it is in. `invoices::duplicate_invoice` is that shape written back
through `create_invoice`, so a duplicate is validated exactly as a hand-created
invoice is. `invoicing::schedules` stores the same shape against a client and a
cadence in three new tables and walks periods forward from `next_period`;
each generation is one transaction that writes the invoice, its provenance row
and the advanced `next_period` together, and `UNIQUE(schedule_id, period)` is
what makes a second run a no-op. Sending is layered on top through the
`send_with`/`void_with` seam — the branding and the three clients are injected
by the front end, because nothing under `src/invoicing/` may read settings or
the clock.

**Tech Stack:** Rust, rusqlite/SQLCipher, chrono, clap (derive),
ratatui/crossterm, axum (`serve` feature), comfy-table, Lit + Web Awesome,
vitest.

**Spec:** `docs/superpowers/specs/2026-08-20-invoice-recurrence-design.md`

**Tasks:** TASK-7 (duplicate, 7 ACs) and TASK-81 (schedules, 10 ACs). Read both
with `backlog task 7 --plain` / `backlog task 81 --plain`. **Never edit a task
file directly**, and task files are committed to `main`, never onto this branch.

### Spec-vs-spec and spec-vs-code notes

Recorded here rather than silently deviated from:

1. **"Two tables" vs three.** Spec §2 opens "Two tables (one migration,
   renumber-aware)" and then describes three: `invoice_schedules`,
   `invoice_schedule_items`, `invoice_schedule_runs`. The three named tables
   win — the run table is load-bearing for AC #3, and the sentence is a
   miscount. Migration v12 creates three.
2. **`create_invoice` cannot be called inside a transaction.**  Spec §3 says
   "Each generation is one transaction: duplicate-shaped `create_invoice` from
   the schedule's items, insert the run row, advance `next_period`."
   `create_invoice` opens its own `conn.unchecked_transaction()`
   (`crates/nigel-core/src/invoicing/invoices.rs:112`), and SQLite has no
   nested `BEGIN`. Task 6 therefore extracts `insert_invoice` — the same body
   with no transaction of its own — and `create_invoice` becomes that plus the
   `BEGIN`/`COMMIT`. Behaviour is unchanged for every existing caller; the
   generator gets the one transaction the spec asks for.
3. **The offset rule and `net_days` are the same arithmetic.** Spec §1's
   issue→due offset and spec §2's per-schedule "net-days for the due date" are
   one helper, `invoices::plus_days`, used by both. Written down because a
   reviewer will otherwise expect two.
4. **Migration numbering.** `LATEST_VERSION` is **9** on this branch's base.
   Two open pull requests both claim **v10**: #38 `feat/account-classification`
   (TASK-9.1) and #37 `feat/import-integrity` (TASK-50/51/52). This branch's
   migration is therefore **v12**, the next number after both, and expects a
   renumber at merge if either lands elsewhere in the order. The renumber diff
   is the version literal in `MIGRATIONS`, the `LATEST_VERSION` assertions in
   the tests, and nothing else. **Never reuse a number and never leave a gap** —
   the runner applies `version > current`, so a database stamped v12 would skip
   a v10 that merged afterwards. Put this note in the PR body.
5. **Out of scope, per spec §4:** no TUI and no web surface for managing
   schedules. The schedule surface is CLI-complete. The *duplicate* action does
   ship on all three surfaces.

---

## Global Constraints

Every task's requirements implicitly include this section.

- **⛔ Public repository — no real book data.** Fixtures use the fictional cast
  only: **Acme, Cedar Systems, Juniper Labs, Harbor & Vale, Globex, Initech**,
  with invented amounts and `*.test` email addresses. No real client, vendor or
  person's name, no figure read off live books, in code, tests, docs, task
  notes or commit messages. Before every commit:
  `./scripts/check-no-real-data.sh --staged` — **judge it by its exit status,
  never by grepping its output.**
- **The pre-commit hook is judged by exit status and never bypassed.** No
  `--no-verify`, no `-n`. A refused commit is fixed, not forced.
- **Tests are serial, and both feature variants must pass**, after every task:
  ```bash
  cargo test -- --test-threads=1
  cargo test --no-default-features -- --test-threads=1
  ```
  The database password is a process global; a parallel run corrupts it.
- **Lint and format clean after every task:**
  ```bash
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  ```
- **The web suite runs from `web/`** for the one task that touches the SPA
  (Task 4): `npm test`, `npm run typecheck`, `npm run lint`.
- **Component-first only where a component is needed.** Task 4 adds a plain
  `<wa-button>` inside the existing `.actions` block of
  `web/apps/app/src/screens/invoices.ts`, alongside Send/Pay/Edit/Void/Delete.
  That is not a new visual element and needs no `@nigel/ui` component. **If the
  implementation grows anything beyond a plain button** — a dialog, a badge, a
  new layout — it ships through `web/packages/ui/src/components/wc-*.ts` with a
  co-located `.preview.ts` and a `describePreviewA11y` test, per CLAUDE.md's
  Component-First UI Workflow.
- **No provenance comments.** No "added in", "was formerly", "renamed
  because", "don't change this back", no migration-history prose in docs.
  `git log` and `backlog/decisions/` carry history. Rationale goes in the
  commit message or the function's own doc comment.
- **Nothing under `crates/nigel-core/src/invoicing/` reads the clock or the
  settings.** Every derivation takes its reference day as a parameter — the
  schedule runner's `today` is a parameter throughout, and so is
  `duplicate_invoice`'s `issue_date`. The CLI passes `cli::today()`, the HTTP
  layer passes `crate::clock::today()`, and the collaborators an autosend needs
  are injected by the caller. A `crate::clock::` or `crate::settings::`
  reference inside `src/invoicing/schedules.rs` fails review.
- **Migration renumber note:** see header note 4. v12 on this branch.
- **The crate boundary holds.** `nigel-core` never names `crate::cli::` or
  `nigel::`; `crates/nigel/tests/layering.rs` enforces it.

---

### Task 1: The invoice shape and `duplicate_invoice`

**Files:**
- Modify: `crates/nigel-core/src/invoicing/invoices.rs`
- Test: `crates/nigel-core/src/invoicing/invoices.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `create_invoice`, `get_invoice`, `line_items`, `parse_date`,
  `validate_date`, `NewLineItem` — all already in this file.
- Produces:
  ```rust
  pub struct InvoiceShape {
      pub client_id: i64,
      pub currency: String,
      pub notes: Option<String>,
      pub terms: Option<String>,
      pub items: Vec<NewLineItem>,
      pub net_days: Option<i64>,
  }
  pub fn invoice_shape(conn: &Connection, invoice_id: i64) -> Result<InvoiceShape>;
  pub fn plus_days(date: &str, days: i64) -> Result<String>;
  pub fn duplicate_invoice(conn: &Connection, source_id: i64, issue_date: &str) -> Result<i64>;
  ```

- [ ] **Step 1: Write the failing tests.** Append to `mod tests` at the bottom
  of `crates/nigel-core/src/invoicing/invoices.rs`:

```rust
#[test]
fn duplicating_copies_the_shape_and_regenerates_the_identity() {
    let (_d, conn) = test_conn();
    let client = add_client(&conn, "Cedar Systems", Some("ops@cedar.test"), None, None).unwrap();
    let items = vec![
        NewLineItem { description: "Retainer".into(), quantity: 1.0, unit_amount: 2_400.0 },
        NewLineItem { description: "Hosting".into(), quantity: 3.0, unit_amount: 45.0 },
    ];
    let source_id = create_invoice(
        &conn, client, "2026-06-01", Some("2026-06-15"), "EUR", &items,
        Some("Thanks for the quarter."), Some("Net 14."),
    )
    .unwrap();
    mark_published(&conn, source_id, "2026-06-02").unwrap();
    set_payment_link(&conn, source_id, "plink_1", "https://pay.example.test/1").unwrap();
    let source = get_invoice(&conn, source_id).unwrap();

    let copy_id = duplicate_invoice(&conn, source_id, "2026-09-01").unwrap();
    let copy = get_invoice(&conn, copy_id).unwrap();

    // Copied.
    assert_eq!(copy.client_id, source.client_id);
    assert_eq!(copy.currency, "EUR");
    assert_eq!(copy.notes.as_deref(), Some("Thanks for the quarter."));
    assert_eq!(copy.terms.as_deref(), Some("Net 14."));
    assert_eq!(copy.subtotal, source.subtotal);
    assert_eq!(copy.total, source.total);
    let copied: Vec<(String, f64, f64)> = line_items(&conn, copy_id)
        .unwrap()
        .into_iter()
        .map(|i| (i.description, i.quantity, i.unit_amount))
        .collect();
    assert_eq!(
        copied,
        vec![
            ("Retainer".to_string(), 1.0, 2_400.0),
            ("Hosting".to_string(), 3.0, 45.0),
        ]
    );

    // Regenerated.
    assert_eq!(copy.number, source.number + 1);
    assert_ne!(copy.token, source.token);
    assert_eq!(copy.status, "draft");
    assert_eq!(copy.published_at, None);
    assert_eq!(copy.voided_at, None);
    assert_eq!(copy.stripe_payment_link_id, None);
    assert_eq!(copy.stripe_payment_link_url, None);
}

#[test]
fn duplicating_preserves_the_issue_to_due_offset_in_days() {
    let (_d, conn) = test_conn();
    let client = add_client(&conn, "Globex", Some("ap@globex.test"), None, None).unwrap();
    let items = vec![NewLineItem { description: "Audit".into(), quantity: 1.0, unit_amount: 500.0 }];

    // Net 14 duplicates as Net 14, across a month boundary and a leap February.
    let net14 = create_invoice(&conn, client, "2026-01-20", Some("2026-02-03"), "USD", &items, None, None).unwrap();
    let copy = get_invoice(&conn, duplicate_invoice(&conn, net14, "2028-02-20").unwrap()).unwrap();
    assert_eq!(copy.issue_date, "2028-02-20");
    assert_eq!(copy.due_date.as_deref(), Some("2028-03-05"));

    // No due date on the source means none on the copy.
    let open = create_invoice(&conn, client, "2026-01-20", None, "USD", &items, None, None).unwrap();
    let copy = get_invoice(&conn, duplicate_invoice(&conn, open, "2026-09-01").unwrap()).unwrap();
    assert_eq!(copy.due_date, None);

    // A same-day due date stays a same-day due date rather than becoming none.
    let same = create_invoice(&conn, client, "2026-01-20", Some("2026-01-20"), "USD", &items, None, None).unwrap();
    let copy = get_invoice(&conn, duplicate_invoice(&conn, same, "2026-09-01").unwrap()).unwrap();
    assert_eq!(copy.due_date.as_deref(), Some("2026-09-01"));
}

#[test]
fn any_source_state_duplicates_because_duplication_reads_a_shape() {
    let (_d, conn) = test_conn();
    let client = add_client(&conn, "Juniper Labs", Some("ap@juniper.test"), None, None).unwrap();
    let items = vec![NewLineItem { description: "Workshop".into(), quantity: 1.0, unit_amount: 800.0 }];

    let draft = create_invoice(&conn, client, "2026-05-01", Some("2026-05-31"), "USD", &items, None, None).unwrap();

    let sent = create_invoice(&conn, client, "2026-05-01", Some("2026-05-31"), "USD", &items, None, None).unwrap();
    mark_published(&conn, sent, "2026-05-01").unwrap();
    refresh_status(&conn, sent, "2026-05-02").unwrap();

    let paid = create_invoice(&conn, client, "2026-05-01", Some("2026-05-31"), "USD", &items, None, None).unwrap();
    mark_published(&conn, paid, "2026-05-01").unwrap();
    record_payment(&conn, paid, 800.0, "2026-05-10", "direct_deposit", None).unwrap();

    let voided = create_invoice(&conn, client, "2026-05-01", Some("2026-05-31"), "USD", &items, None, None).unwrap();
    void_invoice(&conn, voided, "2026-05-04").unwrap();
    refresh_status(&conn, voided, "2026-05-05").unwrap();

    for (label, source) in [("draft", draft), ("sent", sent), ("paid", paid), ("void", voided)] {
        let copy_id = duplicate_invoice(&conn, source, "2026-08-20")
            .unwrap_or_else(|e| panic!("{label} source refused: {e}"));
        let copy = get_invoice(&conn, copy_id).unwrap();
        assert_eq!(copy.status, "draft", "{label} duplicated into a non-draft");
        assert_eq!(copy.total, 800.0, "{label}");
    }
}

#[test]
fn duplicating_for_an_archived_client_refuses_the_way_create_invoice_does() {
    let (_d, conn) = test_conn();
    let client = add_client(&conn, "Harbor & Vale", Some("ap@harborvale.test"), None, None).unwrap();
    let items = vec![NewLineItem { description: "Retainer".into(), quantity: 1.0, unit_amount: 1_000.0 }];
    let source = create_invoice(&conn, client, "2026-05-01", None, "USD", &items, None, None).unwrap();

    crate::invoicing::clients::archive_client(&conn, client, "2026-06-01").unwrap();

    let err = duplicate_invoice(&conn, source, "2026-08-20").unwrap_err();
    assert!(
        matches!(err, NigelError::Conflict { code: "client_archived", .. }),
        "got: {err:?}"
    );
    assert!(err.to_string().contains("Harbor & Vale"), "got: {err}");
}

#[test]
fn duplicating_a_missing_invoice_is_not_found_and_reserves_no_number() {
    let (_d, conn) = test_conn();
    let before = next_number(&conn).unwrap();
    let err = duplicate_invoice(&conn, 404, "2026-08-20").unwrap_err();
    assert!(matches!(err, NigelError::NotFound(_)), "got: {err:?}");
    assert_eq!(next_number(&conn).unwrap(), before);
}
```

  The test module already has `test_conn()`; check its `use super::*;` block
  and add whatever these need — `crate::invoicing::clients::add_client`,
  `void_invoice`, `mark_published`, `set_payment_link`, `record_payment`,
  `refresh_status` are all in scope through `super::*` except `add_client`.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p nigel-core invoicing::invoices::tests::duplicating -- --test-threads=1`
Expected: FAIL — `cannot find function 'duplicate_invoice' in this scope`.

- [ ] **Step 3: Write the implementation.** In
  `crates/nigel-core/src/invoicing/invoices.rs`, after `create_invoice` and
  before `row_to_invoice`:

```rust
/// What an invoice *is*, independent of what state it is in.
///
/// The one reader both duplication and a schedule seed go through, so
/// "duplicate this invoice" and "bill this shape every month" cannot drift
/// apart about what gets carried across.
#[derive(Debug, Clone)]
pub struct InvoiceShape {
    pub client_id: i64,
    pub currency: String,
    pub notes: Option<String>,
    pub terms: Option<String>,
    pub items: Vec<NewLineItem>,
    /// The source's issue-to-due gap in days, or `None` when it had no due
    /// date. Days rather than a date, because the shape outlives the calendar
    /// it was first written against.
    pub net_days: Option<i64>,
}

pub fn invoice_shape(conn: &Connection, invoice_id: i64) -> Result<InvoiceShape> {
    let invoice = get_invoice(conn, invoice_id)?;
    let net_days = match invoice.due_date.as_deref() {
        Some(due) => {
            let issued = parse_date(&invoice.issue_date, "issue")?;
            Some((parse_date(due, "due")? - issued).num_days())
        }
        None => None,
    };
    let items = line_items(conn, invoice.id)?
        .into_iter()
        .map(|item| NewLineItem {
            description: item.description,
            quantity: item.quantity,
            unit_amount: item.unit_amount,
        })
        .collect();
    Ok(InvoiceShape {
        client_id: invoice.client_id,
        currency: invoice.currency,
        notes: invoice.notes,
        terms: invoice.terms,
        items,
        net_days,
    })
}

/// `date` shifted by `days`, as a zero-padded `YYYY-MM-DD`.
///
/// The one place a term in days becomes a due date: a duplicate's preserved
/// offset and a schedule's `net_days` are the same arithmetic, and two copies
/// of it would eventually disagree at a month boundary.
pub fn plus_days(date: &str, days: i64) -> Result<String> {
    let parsed = parse_date(date, "issue")?;
    parsed
        .checked_add_signed(chrono::Duration::days(days))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .ok_or_else(|| {
            NigelError::Invalid(format!("{date} plus {days} days is not a real date."))
        })
}

/// Duplicate an invoice into a fresh draft.
///
/// **Copied:** client, currency, notes, terms, and every line item's
/// description, quantity and unit amount.
///
/// **Regenerated:** the number (a fresh `next_number`), the token, and
/// `status = 'draft'`. `published_at`, `voided_at` and the Stripe link fields
/// start empty — a duplicate has published nothing and been paid for nothing.
///
/// **Dates:** `issue_date` is the caller's, because nothing here reads the
/// clock. When the source carries a due date the new draft **preserves the
/// source's issue-to-due offset in days** — a Net-14 invoice duplicates as
/// Net-14 — and a source with no due date yields a draft with none.
///
/// Any source duplicates, whatever state it is in: duplication reads a shape,
/// not a state. It goes through `create_invoice`, so an archived client refuses
/// exactly as it would for a hand-created invoice.
pub fn duplicate_invoice(conn: &Connection, source_id: i64, issue_date: &str) -> Result<i64> {
    let shape = invoice_shape(conn, source_id)?;
    let issue_date = validate_date(issue_date, "issue")?;
    let due_date = shape
        .net_days
        .map(|days| plus_days(&issue_date, days))
        .transpose()?;
    create_invoice(
        conn,
        shape.client_id,
        &issue_date,
        due_date.as_deref(),
        &shape.currency,
        &shape.items,
        shape.notes.as_deref(),
        shape.terms.as_deref(),
    )
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p nigel-core invoicing::invoices -- --test-threads=1`
Expected: PASS, every test in the module including the five new ones.

- [ ] **Step 5: Full check**

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
```
Expected: all clean, `test result: ok` on both variants.

- [ ] **Step 6: Commit**

```bash
./scripts/check-no-real-data.sh --staged; echo "exit=$?"
git add crates/nigel-core/src/invoicing/invoices.rs
git commit -m "Duplicate an invoice into a fresh draft, preserving its term"
```
Expected: `exit=0` before the commit, and the pre-commit hook passes.

---

### Task 2: `nigel invoice duplicate <number>`

**Files:**
- Modify: `crates/nigel/src/cli/mod.rs` (the `InvoiceCommands` enum)
- Modify: `crates/nigel/src/cli/invoice.rs`
- Modify: `crates/nigel/src/main.rs` (the `InvoiceCommands` match arm)
- Test: `crates/nigel/tests/cli_dispatch.rs`

**Interfaces:**
- Consumes: `duplicate_invoice(conn, source_id, issue_date) -> Result<i64>` from Task 1.
- Produces: `cli::invoice::duplicate(number: i64, issue_date: &str) -> Result<()>`,
  and the subcommand `InvoiceCommands::Duplicate { number, issue_date }`.

- [ ] **Step 1: Write the failing test.** Append to
  `crates/nigel/tests/cli_dispatch.rs`:

```rust
#[test]
fn invoice_duplicate_creates_a_draft_carrying_the_source_shape() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "duplicate", "1248", "--issue", "2026-09-01"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Duplicated invoice #1248 as draft #1249",
        ));

    env.cmd()
        .args(["invoice", "show", "1249"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Acme Co")
                .and(predicate::str::contains("Consulting"))
                .and(predicate::str::contains("2026-09-01"))
                .and(predicate::str::contains("[draft]")),
        );
}

#[test]
fn invoice_duplicate_without_an_issue_date_uses_today() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "duplicate", "1248"])
        .assert()
        .success()
        .stdout(predicate::str::contains("as draft #1249"));

    let issued: String = env
        .db()
        .query_row("SELECT issue_date FROM invoices WHERE number = 1249", [], |r| r.get(0))
        .expect("the duplicate exists");
    assert_eq!(issued, chrono::Local::now().format("%Y-%m-%d").to_string());
}

#[test]
fn invoice_duplicate_names_a_number_that_is_not_there() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "duplicate", "9999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No invoice #9999"));
}
```

  `chrono` is already a dependency of the `nigel` crate; if the integration
  test target cannot see it, add `chrono` to `[dev-dependencies]` in
  `crates/nigel/Cargo.toml`.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p nigel --test cli_dispatch invoice_duplicate -- --test-threads=1`
Expected: FAIL — `unrecognized subcommand 'duplicate'` on stderr, so `.success()` fails.

- [ ] **Step 3: Add the subcommand.** In `crates/nigel/src/cli/mod.rs`, inside
  `pub enum InvoiceCommands`, immediately after the `New { … }` variant:

```rust
    /// Copy an invoice into a fresh draft: same client, items, notes and terms,
    /// a new number, and the source's issue-to-due term.
    Duplicate {
        /// Invoice number to copy (shown in `nigel invoice list`)
        number: i64,
        /// Issue date for the new draft: YYYY-MM-DD (default: today)
        #[arg(long = "issue")]
        issue_date: Option<String>,
    },
```

- [ ] **Step 4: Write the handler.** In `crates/nigel/src/cli/invoice.rs`,
  after `pub fn new(…)`:

```rust
pub fn duplicate(number: i64, issue_date: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let source = find_invoice(&conn, number)?;
    let id = duplicate_invoice(&conn, source.id, issue_date)?;
    let draft = get_invoice(&conn, id)?;
    println!(
        "Duplicated invoice #{number} as draft #{} for {:.2} {}",
        draft.number, draft.total, draft.currency
    );
    Ok(())
}
```

  Add `duplicate_invoice` to the existing
  `use nigel_core::invoicing::invoices::{…}` list at the top of the file.

- [ ] **Step 5: Wire the dispatch.** In `crates/nigel/src/main.rs`, in the
  `Commands::Invoice { command } => match command {` block, after the
  `InvoiceCommands::New { … }` arm:

```rust
            InvoiceCommands::Duplicate { number, issue_date } => cli::invoice::duplicate(
                number,
                issue_date.as_deref().unwrap_or(&cli::today()),
            ),
```

  If the borrow of the temporary from `cli::today()` is rejected, bind it
  first:

```rust
            InvoiceCommands::Duplicate { number, issue_date } => {
                let today = cli::today();
                cli::invoice::duplicate(number, issue_date.as_deref().unwrap_or(&today))
            }
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p nigel --test cli_dispatch invoice_duplicate -- --test-threads=1`
Expected: PASS, 3 passed.

- [ ] **Step 7: Full check and commit**

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
./scripts/check-no-real-data.sh --staged; echo "exit=$?"
git add crates/nigel/src/cli/mod.rs crates/nigel/src/cli/invoice.rs crates/nigel/src/main.rs crates/nigel/tests/cli_dispatch.rs
git commit -m "Add nigel invoice duplicate"
```
Expected: clean, `exit=0`, hook passes.

---

### Task 3: The TUI duplicate action

**Files:**
- Modify: `crates/nigel/src/cli/invoice_manager.rs`
- Test: `crates/nigel/src/cli/invoice_manager.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `duplicate_invoice` from Task 1.
- Produces: `Screen::ConfirmDuplicate`, `c` on the invoice detail screen, and
  `InvoiceManager::perform_duplicate(&mut self, conn: &Connection, today: &str)`.

- [ ] **Step 1: Write the failing tests.** Append to the `mod tests` block at
  the bottom of `crates/nigel/src/cli/invoice_manager.rs`:

```rust
fn open_duplicate(mgr: &mut InvoiceManager, conn: &Connection) {
    mgr.handle_key(KeyCode::Enter, conn);
    mgr.handle_key(KeyCode::Char('c'), conn);
}

#[test]
fn c_opens_the_duplicate_confirmation_naming_the_invoice_and_client() {
    let (_d, conn) = test_conn();
    seed_invoice(&conn, "Acme Co", 1_250.0);
    let mut mgr = manager(&conn);
    open_duplicate(&mut mgr, &conn);

    assert!(matches!(mgr.screen, Screen::ConfirmDuplicate));
    let screen = rendered(&mut mgr);
    assert!(
        screen.contains("Duplicate invoice #1248 for Acme Co ($1,250.00)?"),
        "{screen}"
    );
    assert!(screen.contains("y=duplicate  n=cancel"), "{screen}");
}

#[test]
fn declining_the_duplicate_changes_nothing() {
    let (_d, conn) = test_conn();
    seed_invoice(&conn, "Cedar Systems", 400.0);
    for key in [KeyCode::Char('n'), KeyCode::Esc] {
        let mut mgr = manager(&conn);
        open_duplicate(&mut mgr, &conn);
        mgr.handle_key(key, &conn);
        assert!(matches!(mgr.screen, Screen::Detail), "{key:?}");
        assert_eq!(invoice_count(&conn), 1, "{key:?}");
    }
}

#[test]
fn confirming_the_duplicate_lands_on_the_new_draft() {
    let (_d, conn) = test_conn();
    seed_invoice(&conn, "Globex", 900.0);
    let mut mgr = manager(&conn);
    open_duplicate(&mut mgr, &conn);
    mgr.handle_key(KeyCode::Char('y'), &conn);

    assert!(matches!(mgr.screen, Screen::Detail));
    assert_eq!(invoice_count(&conn), 2);

    let detail = mgr.detail.as_ref().expect("the new draft is open");
    assert_eq!(detail.invoice.number, 1249, "the detail is the copy, not the source");
    assert_eq!(detail.invoice.status, "draft");
    assert_eq!(detail.invoice.total, 900.0);
    // The cursor moved with the screen, so Esc lands beside the draft.
    assert_eq!(mgr.rows[mgr.selection].number, 1249);
    let status = mgr.status_message.as_deref().unwrap_or_default();
    assert!(
        status.contains("Duplicated invoice #1248 as draft #1249"),
        "got: {status}"
    );
}

#[test]
fn the_duplicate_takes_the_reference_day_it_is_given() {
    let (_d, conn) = test_conn();
    let source = seed_invoice(&conn, "Juniper Labs", 600.0);
    let mut mgr = manager(&conn);
    mgr.handle_key(KeyCode::Enter, &conn);
    mgr.perform_duplicate(&conn, "2027-01-15");

    let detail = mgr.detail.as_ref().expect("the new draft is open");
    assert_eq!(detail.invoice.issue_date, "2027-01-15");
    // `seed_invoice_for` issues 2026-07-16 due 2026-08-15 — thirty days.
    assert_eq!(detail.invoice.due_date.as_deref(), Some("2027-02-14"));
    assert_ne!(detail.invoice.id, source);
}

#[test]
fn a_refused_duplicate_lands_on_the_status_line_with_the_invoice_still_open() {
    let (_d, conn) = test_conn();
    let source = seed_invoice(&conn, "Harbor & Vale", 750.0);
    let client_id: i64 = conn
        .query_row("SELECT client_id FROM invoices WHERE id = ?1", [source], |r| r.get(0))
        .unwrap();
    nigel_core::invoicing::clients::archive_client(&conn, client_id, "2026-08-01").unwrap();

    let mut mgr = manager(&conn);
    open_duplicate(&mut mgr, &conn);
    mgr.handle_key(KeyCode::Char('y'), &conn);

    assert!(matches!(mgr.screen, Screen::Detail));
    assert_eq!(invoice_count(&conn), 1);
    let status = mgr.status_message.as_deref().unwrap_or_default();
    assert!(status.contains("archived"), "got: {status}");
    let detail = mgr.detail.as_ref().expect("the source is still open");
    assert_eq!(detail.invoice.number, 1248);
}

#[test]
fn the_detail_footer_advertises_duplicate_for_every_state() {
    let (_d, conn) = test_conn();
    let id = seed_invoice(&conn, "Initech", 300.0);
    void_invoice(&conn, id, "2026-07-20").unwrap();
    let mut mgr = manager(&conn);
    mgr.handle_key(KeyCode::Enter, &conn);

    let screen = rendered(&mut mgr);
    assert!(screen.contains("c=duplicate"), "a void invoice still copies: {screen}");
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p nigel duplicate -- --test-threads=1`
Expected: FAIL — `no variant named 'ConfirmDuplicate' found for enum 'Screen'`.

- [ ] **Step 3: Add the screen and the key.** Four edits in
  `crates/nigel/src/cli/invoice_manager.rs`.

  **3a.** In `enum Screen`, after `ConfirmDelete`:

```rust
    /// Duplicate, awaiting its answer. `ConfirmDelete`'s shape rather than
    /// send's: one local transaction, nothing to reach, nothing to warn about.
    ConfirmDuplicate,
```

  **3b.** In `draw`, extend the detail arm:

```rust
            Screen::Detail
            | Screen::ConfirmVoid
            | Screen::ConfirmDelete
            | Screen::ConfirmDuplicate
            | Screen::ConfirmSend => self.draw_detail(frame),
```

  **3c.** In `draw_detail`, after the `if let Screen::ConfirmDelete` block that
  pushes lines:

```rust
        if let Screen::ConfirmDuplicate = &self.screen {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!(
                    "   Duplicate invoice #{} for {} ({})?",
                    invoice.number,
                    detail.client_name(),
                    money(invoice.total)
                ),
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from(Span::styled(
                "   A new draft with the same items and term, under a new number.",
                Style::default().fg(Color::Yellow),
            )));
        }
```

  and in the hints chain, after the `ConfirmDelete` arm:

```rust
        } else if let Screen::ConfirmDuplicate = &self.screen {
            frame.render_widget(
                Paragraph::new(" y=duplicate  n=cancel").style(FOOTER_STYLE),
                hints_area,
            );
```

  and in the `else` arm's hint strings, add `c=duplicate` to all three — it is
  the one verb every invoice accepts:

```rust
            let hint = if is_void(invoice) {
                " c=duplicate  Up/Down=scroll  Esc=back  q=quit"
            } else if detail.deletable {
                " s=send  p=record payment  c=duplicate  v=void  d=delete  Up/Down=scroll  Esc=back  q=quit"
            } else {
                " s=send  p=record payment  c=duplicate  v=void  Up/Down=scroll  Esc=back  q=quit"
            };
```

  **3d.** In `handle_key`'s screen match, beside `Screen::ConfirmDelete`:

```rust
            Screen::ConfirmDuplicate => self.handle_duplicate_key(code, conn),
```

- [ ] **Step 4: Write the behaviour.** In
  `crates/nigel/src/cli/invoice_manager.rs`, next to `handle_delete_key` and
  `perform_delete`:

```rust
    fn handle_duplicate_key(&mut self, code: KeyCode, conn: &Connection) -> InvoiceAction {
        match code {
            KeyCode::Char('y') => {
                self.perform_duplicate(conn, &crate::cli::today());
                InvoiceAction::Continue
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.screen = Screen::Detail;
                InvoiceAction::Continue
            }
            _ => InvoiceAction::Continue,
        }
    }

    /// Copy the open invoice and land on the copy.
    ///
    /// `perform_delete`'s shape — one local transaction, so it runs on the key
    /// rather than through `InvoiceAction::Perform`. `today` is a parameter
    /// because the reference day is the caller's everywhere else too.
    fn perform_duplicate(&mut self, conn: &Connection, today: &str) {
        let Some(detail) = &self.detail else {
            return;
        };
        let (source_id, source_number) = (detail.invoice.id, detail.invoice.number);
        match duplicate_invoice(conn, source_id, today) {
            Ok(new_id) => {
                self.reload_list(conn);
                self.select_invoice(new_id);
                match self.load_detail(conn, new_id) {
                    Ok(()) => {
                        self.detail_scroll = 0;
                        self.screen = Screen::Detail;
                        let number = self
                            .detail
                            .as_ref()
                            .map(|d| d.invoice.number)
                            .unwrap_or_default();
                        self.set_status(format!(
                            "Duplicated invoice #{source_number} as draft #{number}."
                        ));
                    }
                    Err(e) => {
                        self.detail = None;
                        self.screen = Screen::List;
                        self.set_status(e.to_string());
                    }
                }
            }
            Err(e) => {
                // `perform_delete`'s reasoning: the refusal came from the write,
                // so the row may have moved under this screen. Reload it before
                // the sentence lands beside it.
                self.after_mutation(conn, source_id);
                self.set_status(e.to_string());
            }
        }
    }

    /// Put the cursor on one invoice by row id, if it is in the list.
    fn select_invoice(&mut self, invoice_id: i64) {
        if let Some(idx) = self.rows.iter().position(|row| row.id == invoice_id) {
            self.selection = idx;
        }
    }
```

  In `handle_detail_key`, beside the other verbs:

```rust
            KeyCode::Char('c') => self.open_confirmation(Screen::ConfirmDuplicate),
```

  Add `duplicate_invoice` to the file's
  `use nigel_core::invoicing::invoices::{…}` list.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p nigel duplicate -- --test-threads=1`
Expected: PASS, 6 new tests plus the untouched manager suite.

- [ ] **Step 6: Full check and commit**

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
./scripts/check-no-real-data.sh --staged; echo "exit=$?"
git add crates/nigel/src/cli/invoice_manager.rs
git commit -m "Duplicate an invoice from the TUI detail screen"
```
Expected: clean, `exit=0`, hook passes.

---

### Task 4: `POST /api/invoices/{number}/duplicate` and the Duplicate button

**Files:**
- Modify: `crates/nigel-core/src/server/routes/invoices.rs`
- Modify: `crates/nigel-core/src/server/testutil.rs` (`WRITE_ROUTES`)
- Modify: `web/apps/app/src/api/client.ts`
- Modify: `web/apps/app/src/__mocks__/fake-api-client.ts`
- Modify: `web/apps/app/src/screens/invoices.ts`
- Test: `crates/nigel-core/src/server/routes/invoices.rs` (`mod tests`),
  `web/apps/app/src/screens/invoices.test.ts`

**Interfaces:**
- Consumes: `duplicate_invoice` (Task 1), `detail_for`, `find_invoice`,
  `with_conn_api`, `ApiPath` — all already in `routes/invoices.rs`.
- Produces: `POST /api/invoices/{number}/duplicate` → `201` with the new
  draft's `InvoiceDetail`; TypeScript
  `duplicateInvoice(number: number): Promise<InvoiceDetail>` on `ApiClient`.

- [ ] **Step 1: Write the failing route tests.** Append to `mod tests` at the
  bottom of `crates/nigel-core/src/server/routes/invoices.rs`:

```rust
#[tokio::test]
async fn duplicating_answers_the_new_draft_at_201() {
    let (_dir, db_path) = seeded_db();
    let (app, token) = app_for(&db_path);

    let source = ok_json(&app, "/api/invoices/1251", &token).await;

    let (status, body) = post_json(&app, "/api/invoices/1251/duplicate", &token, &json!({})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["number"], 1253, "{body}");
    assert_eq!(body["status"], "draft", "{body}");
    assert_eq!(body["clientId"], source["clientId"], "{body}");
    assert_eq!(body["currency"], source["currency"], "{body}");
    assert_eq!(body["total"], source["total"], "{body}");
    assert_eq!(body["publishedAt"], serde_json::Value::Null, "{body}");
    assert_eq!(body["voidedAt"], serde_json::Value::Null, "{body}");
    assert_eq!(body["publicUrl"], serde_json::Value::Null, "{body}");
    assert_eq!(body["canEdit"], true, "{body}");

    let source_items = source["items"].as_array().unwrap();
    let copied = body["items"].as_array().unwrap();
    assert_eq!(copied.len(), source_items.len(), "{body}");
    for (from, to) in source_items.iter().zip(copied) {
        assert_eq!(to["description"], from["description"]);
        assert_eq!(to["quantity"], from["quantity"]);
        assert_eq!(to["unitAmount"], from["unitAmount"]);
    }
}

#[tokio::test]
async fn duplicating_an_invoice_that_is_not_there_is_a_404() {
    let (_dir, db_path) = seeded_db();
    let (app, token) = app_for(&db_path);

    let (status, body) = post_json(&app, "/api/invoices/9999/duplicate", &token, &json!({})).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["error"]["details"]["reason"], "invoice_not_found", "{body}");
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p nigel-core --features serve duplicating -- --test-threads=1`
Expected: FAIL — `405 Method Not Allowed` (axum's bodyless response), so the
`CREATED` assertion fails.

- [ ] **Step 3: Add the route.** In
  `crates/nigel-core/src/server/routes/invoices.rs`, in `routes()`, after the
  `/invoices/{number}/send` line:

```rust
        .route("/invoices/{number}/duplicate", post(duplicate))
```

  and the handler, next to `create`:

```rust
/// `POST /api/invoices/{number}/duplicate` — copy an invoice into a fresh draft.
///
/// Answers `201` with the whole new `InvoiceDetail`, the way `POST /api/invoices`
/// does, so a browser can navigate straight to it. The server's own today is the
/// new issue date, for `void`'s reason: the data layer takes its reference day as
/// a parameter and this is where that day comes from.
async fn duplicate(
    State(state): State<AppState>,
    ApiPath(number): ApiPath<i64>,
) -> ApiResult<(StatusCode, Json<InvoiceDetail>)> {
    let today = crate::clock::today();
    let detail = with_conn_api(&state, move |conn| {
        let source = find_invoice(conn, number)?;
        let id = inv::duplicate_invoice(conn, source.id, &today)?;
        detail_for(conn, inv::get_invoice(conn, id)?)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(detail)))
}
```

  In `crates/nigel-core/src/server/testutil.rs`, add the route to
  `WRITE_ROUTES` — it is guarded by the locked and session middleware like
  every other write — and bump the array length:

```rust
pub const WRITE_ROUTES: [(&str, &str, &str); 35] = [
```

```rust
    ("POST", "/api/invoices/1252/duplicate", "{}"),
```

  (put it beside the other `/api/invoices/1252/…` entries).

- [ ] **Step 4: Run the route tests to verify they pass**

Run: `cargo test -p nigel-core --features serve -- --test-threads=1`
Expected: PASS, including the two new tests and the `WRITE_ROUTES` guard tests
in `crates/nigel-core/src/server/mod.rs`.

- [ ] **Step 5: Write the failing web test.** In
  `web/apps/app/src/screens/invoices.test.ts`, beside the delete tests:

```ts
  it('duplicates an invoice and navigates to the new draft', async () => {
    const fake = client();
    const toasts: string[] = [];
    const onToast = (event: Event) =>
      toasts.push((event as CustomEvent<{ message: string }>).detail.message);
    window.addEventListener('nc-toast', onToast);

    const { el, routes } = await mount('number=1251', fake);
    button(el, '[data-duplicate]').click();
    await settle(el);
    window.removeEventListener('nc-toast', onToast);

    expect(fake.calls).toContain('duplicateInvoice:1251');
    expect(routes.at(-1)).toEqual({ screen: 'invoices', params: 'number=1253' });
    expect(toasts).toEqual(['Duplicated invoice #1251 as draft #1253.']);
  });

  it('offers Duplicate whatever state the invoice is in', async () => {
    const fake = client();
    fake.invoiceDetails[1252] = detail({
      number: 1252,
      status: 'void',
      canEdit: false,
      canSend: false,
      canVoid: false,
      canPay: false,
      canDelete: false,
    });
    const { el } = await mount('number=1252', fake);
    expect(button(el, '[data-duplicate]').hasAttribute('disabled')).toBe(false);
  });

  it('renders a refused duplicate beside the invoice', async () => {
    const fake = client();
    fake.duplicateInvoiceError = conflictError('client_archived', {
      message: "client 'Umbrella Corp' is archived — unarchive it before invoicing",
    });
    const { el, routes } = await mount('number=1251', fake);
    const before = routes.length;

    button(el, '[data-duplicate]').click();
    await settle(el);

    const notice = el.shadowRoot?.querySelector('[data-action-error]');
    expect(notice?.getAttribute('message')).toContain('archived');
    expect(routes.length).toBe(before);
  });
```

  `1253` is `FakeApiClient.nextInvoiceNumber`'s initial value
  (`web/apps/app/src/__mocks__/fake-api-client.ts:962`), which the fake's
  `duplicateInvoice` hands out and then increments — the same counter
  `createInvoice` uses.

- [ ] **Step 6: Run it to verify it fails**

Run (from `web/`): `npm test -- invoices`
Expected: FAIL — `duplicateInvoice is not a function`, and
`button(el, '[data-duplicate]')` finds nothing.

- [ ] **Step 7: Add the client method and the fake.** In
  `web/apps/app/src/api/client.ts`, in the `ApiClient` interface after
  `updateInvoice`:

```ts
  /**
   * Copy an invoice into a fresh draft — same client, items, notes, terms and
   * issue-to-due term, a new number, nothing published or paid.
   *
   * Any state duplicates: this reads the invoice's shape, not its status.
   */
  duplicateInvoice(number: number): Promise<InvoiceDetail>;
```

  and in the implementing class, beside `voidInvoice`:

```ts
  duplicateInvoice(number: number): Promise<InvoiceDetail> {
    return this.request<InvoiceDetail>('POST', `/invoices/${number}/duplicate`, {});
  }
```

  In `web/apps/app/src/__mocks__/fake-api-client.ts`, add the error hook beside
  `voidInvoiceError`:

```ts
  duplicateInvoiceError: Error | null = null;
```

  and the method beside `deleteInvoice`:

```ts
  async duplicateInvoice(number: number): Promise<InvoiceDetail> {
    this.calls.push(`duplicateInvoice:${number}`);
    if (this.duplicateInvoiceError) throw this.duplicateInvoiceError;

    const source = this.detail(number);
    const created: InvoiceDetail = {
      ...source,
      id: this.nextInvoiceNumber,
      number: this.nextInvoiceNumber,
      status: 'draft',
      publishedAt: null,
      voidedAt: null,
      stripePaymentLinkId: null,
      stripePaymentLinkUrl: null,
      publicUrl: null,
      payments: [],
      paid: 0,
      balance: source.total,
      canEdit: true,
      canSend: source.client?.email != null,
      canVoid: true,
      canPay: true,
      canDelete: true,
    };
    this.nextInvoiceNumber += 1;
    this.invoiceDetails[created.number] = created;
    return created;
  }
```

- [ ] **Step 8: Add the button.** In
  `web/apps/app/src/screens/invoices.ts`, in the `.actions` block, between the
  `data-edit` and `data-void` buttons:

```ts
        <wa-button
          data-duplicate
          appearance="outlined"
          ?disabled=${this.busy}
          @click=${this.handleDuplicate}
        >
          Duplicate
        </wa-button>
```

  and the handler, beside `handleDelete`:

```ts
  private handleDuplicate = async (): Promise<void> => {
    const detail = this.detail;
    if (!detail) return;

    this.busy = true;
    try {
      const created = await this.client.duplicateInvoice(detail.number);
      // The route change reloads, so the toast is what survives to say the
      // copy happened — dispatched while this element is still in the tree.
      dispatchNcToast(this, {
        variant: 'success',
        message: `Duplicated invoice #${detail.number} as draft #${created.number}.`,
      });
      this.actionError = null;
      this.go({ number: String(created.number), edit: null });
    } catch (error) {
      this.actionError = invoicingGuardrailMessage(error, 'invoice');
      await this.refresh(true);
    } finally {
      this.busy = false;
    }
  };
```

  No confirmation dialog: duplicating publishes nothing, tells nobody, and the
  draft it makes can be deleted. It is not a destructive verb.

- [ ] **Step 9: Run the web suite**

Run (from `web/`):
```bash
npm test
npm run typecheck
npm run lint
```
Expected: all pass; the three new invoice-screen tests green.

- [ ] **Step 10: Full check and commit**

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
./scripts/check-no-real-data.sh --staged; echo "exit=$?"
git add crates/nigel-core/src/server crates/nigel-core/src/server/testutil.rs web/apps/app/src
git commit -m "Duplicate an invoice from the browser"
```
Expected: clean, `exit=0`, hook passes.

---

### Task 5: Migration v12 and the schedules data layer

**Files:**
- Create: `crates/nigel-core/src/invoicing/schedules.rs`
- Modify: `crates/nigel-core/src/invoicing/mod.rs`
- Modify: `crates/nigel-core/src/migrations.rs`
- Test: both files' `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `invoices::{validate_items, validate_currency, validate_date,
  NewLineItem}`, `clients::ensure_client_active`.
- Produces:
  ```rust
  pub enum Cadence { Monthly, Quarterly, Yearly }
  impl Cadence {
      pub fn as_str(self) -> &'static str;
      pub fn parse(value: &str) -> Result<Self>;
      pub fn months(self) -> u32;
  }
  pub enum ScheduleScope { Active, All }
  pub struct NewSchedule {
      pub client_id: i64, pub cadence: Cadence, pub anchor_day: u32,
      pub start_period: String, pub net_days: Option<i64>, pub currency: String,
      pub notes: Option<String>, pub terms: Option<String>, pub autosend: bool,
      pub items: Vec<NewLineItem>,
  }
  pub struct Schedule {
      pub id: i64, pub client_id: i64, pub cadence: String, pub anchor_day: u32,
      pub next_period: String, pub net_days: Option<i64>, pub currency: String,
      pub notes: Option<String>, pub terms: Option<String>, pub autosend: bool,
      pub paused: bool, pub ended_at: Option<String>,
  }
  pub struct ScheduleUpdate {
      pub anchor_day: Option<u32>, pub net_days: Option<Option<i64>>,
      pub currency: Option<String>, pub notes: Option<Option<String>>,
      pub terms: Option<Option<String>>, pub autosend: Option<bool>,
      pub items: Option<Vec<NewLineItem>>,
  }
  pub struct ScheduleRun { pub period: String, pub invoice_id: i64, pub number: i64, pub generated_at: String }
  pub fn add_schedule(conn: &Connection, new: &NewSchedule) -> Result<i64>;
  pub fn get_schedule(conn: &Connection, id: i64) -> Result<Schedule>;
  pub fn list_schedules(conn: &Connection, scope: ScheduleScope) -> Result<Vec<Schedule>>;
  pub fn schedule_items(conn: &Connection, schedule_id: i64) -> Result<Vec<NewLineItem>>;
  pub fn update_schedule(conn: &Connection, id: i64, update: &ScheduleUpdate) -> Result<()>;
  pub fn pause_schedule(conn: &Connection, id: i64) -> Result<()>;
  pub fn resume_schedule(conn: &Connection, id: i64) -> Result<()>;
  pub fn end_schedule(conn: &Connection, id: i64, on: &str) -> Result<()>;
  pub fn schedule_runs(conn: &Connection, schedule_id: i64) -> Result<Vec<ScheduleRun>>;
  pub fn clamp_day(year: i32, month: u32, day: u32) -> u32;
  pub fn advance_period(cadence: Cadence, anchor_day: u32, period: &str) -> Result<String>;
  ```

- [ ] **Step 1: Write the failing migration test.** In
  `crates/nigel-core/src/migrations.rs`'s `mod tests`:

```rust
#[test]
fn v12_creates_the_three_schedule_tables_and_the_period_uniqueness() {
    let (_dir, conn) = test_db();
    assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
    assert_eq!(LATEST_VERSION, 12);

    for table in [
        "invoice_schedules",
        "invoice_schedule_items",
        "invoice_schedule_runs",
    ] {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |r| r.get(0),
            )
            .unwrap();
        assert!(exists, "{table} is missing");
    }

    conn.execute_batch(
        "INSERT INTO clients (name) VALUES ('Acme Co');
         INSERT INTO invoice_schedules
             (client_id, cadence, anchor_day, next_period, currency)
         VALUES (1, 'monthly', 1, '2026-01-01', 'USD');
         INSERT INTO invoices (number, client_id, issue_date, token)
         VALUES (1248, 1, '2026-01-01', 'tok-a');
         INSERT INTO invoice_schedule_runs (schedule_id, period, invoice_id, generated_at)
         VALUES (1, '2026-01-01', 1, '2026-01-01');",
    )
    .unwrap();

    let second = conn.execute(
        "INSERT INTO invoice_schedule_runs (schedule_id, period, invoice_id, generated_at)
         VALUES (1, '2026-01-01', 1, '2026-02-01')",
        [],
    );
    assert!(second.is_err(), "a second row for the same period must be refused");
}

#[test]
fn v12_is_replayable() {
    let (_dir, conn) = test_db();
    set_metadata(&conn, "schema_version", "11").unwrap();
    run_migrations(&conn).unwrap();
    assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p nigel-core migrations::tests::v12 -- --test-threads=1`
Expected: FAIL — `assertion 'left == right' failed: 9 != 12`.

- [ ] **Step 3: Add the migration.** In
  `crates/nigel-core/src/migrations.rs`, at the end of the `MIGRATIONS` slice
  after v9. **v10 and v11 are deliberately absent on this branch** — see header
  note 4; renumber to be last if the order changes at merge.

```rust
    Migration {
        version: 12,
        description: "add recurring invoice schedules, their items and their run history",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS invoice_schedules (
                    id INTEGER PRIMARY KEY,
                    client_id INTEGER NOT NULL,
                    cadence TEXT NOT NULL CHECK (cadence IN ('monthly','quarterly','yearly')),
                    anchor_day INTEGER NOT NULL CHECK (anchor_day BETWEEN 1 AND 31),
                    next_period TEXT NOT NULL,
                    net_days INTEGER,
                    currency TEXT NOT NULL DEFAULT 'USD',
                    notes TEXT,
                    terms TEXT,
                    autosend INTEGER NOT NULL DEFAULT 0,
                    paused INTEGER NOT NULL DEFAULT 0,
                    ended_at TEXT,
                    created_at TEXT DEFAULT (datetime('now')),
                    FOREIGN KEY (client_id) REFERENCES clients(id)
                );
                 CREATE TABLE IF NOT EXISTS invoice_schedule_items (
                    id INTEGER PRIMARY KEY,
                    schedule_id INTEGER NOT NULL,
                    description TEXT NOT NULL,
                    quantity REAL NOT NULL DEFAULT 1,
                    unit_amount REAL NOT NULL DEFAULT 0,
                    position INTEGER NOT NULL DEFAULT 0,
                    FOREIGN KEY (schedule_id) REFERENCES invoice_schedules(id) ON DELETE CASCADE
                );
                 CREATE TABLE IF NOT EXISTS invoice_schedule_runs (
                    id INTEGER PRIMARY KEY,
                    schedule_id INTEGER NOT NULL,
                    period TEXT NOT NULL,
                    invoice_id INTEGER NOT NULL,
                    generated_at TEXT NOT NULL,
                    FOREIGN KEY (schedule_id) REFERENCES invoice_schedules(id),
                    FOREIGN KEY (invoice_id) REFERENCES invoices(id)
                );
                 -- Provenance and idempotency in one row: a rerun for a period
                 -- already generated finds this and writes nothing.
                 CREATE UNIQUE INDEX IF NOT EXISTS idx_invoice_schedule_runs_period
                     ON invoice_schedule_runs(schedule_id, period);",
            )?;
            Ok(())
        },
    },
```

  No line total on a schedule item: the figure is arithmetic over the shape,
  and storing it would let a stored total disagree with the items beside it.

- [ ] **Step 4: Run the migration tests to verify they pass**

Run: `cargo test -p nigel-core migrations -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Write the failing data-layer tests.** Create
  `crates/nigel-core/src/invoicing/schedules.rs` with **only** this test module
  for now (the implementation follows in Step 6):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::invoicing::clients::{add_client, archive_client};
    use crate::migrations::run_migrations;

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    fn items() -> Vec<NewLineItem> {
        vec![NewLineItem {
            description: "Hosting & maintenance".into(),
            quantity: 1.0,
            unit_amount: 450.0,
        }]
    }

    fn seed(conn: &Connection) -> (i64, i64) {
        let client = add_client(conn, "Cedar Systems", Some("ops@cedar.test"), None, None).unwrap();
        let schedule = add_schedule(
            conn,
            &NewSchedule {
                client_id: client,
                cadence: Cadence::Monthly,
                anchor_day: 1,
                start_period: "2026-01-01".into(),
                net_days: Some(30),
                currency: "USD".into(),
                notes: Some("Monthly hosting.".into()),
                terms: Some("Net 30.".into()),
                autosend: false,
                items: items(),
            },
        )
        .unwrap();
        (client, schedule)
    }

    #[test]
    fn a_schedule_stores_its_shape_and_starts_at_its_first_period() {
        let (_d, conn) = test_conn();
        let (client, id) = seed(&conn);

        let schedule = get_schedule(&conn, id).unwrap();
        assert_eq!(schedule.client_id, client);
        assert_eq!(schedule.cadence, "monthly");
        assert_eq!(schedule.anchor_day, 1);
        assert_eq!(schedule.next_period, "2026-01-01");
        assert_eq!(schedule.net_days, Some(30));
        assert_eq!(schedule.currency, "USD");
        assert_eq!(schedule.notes.as_deref(), Some("Monthly hosting."));
        assert_eq!(schedule.terms.as_deref(), Some("Net 30."));
        assert!(!schedule.autosend);
        assert!(!schedule.paused);
        assert_eq!(schedule.ended_at, None);

        let stored = schedule_items(&conn, id).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].description, "Hosting & maintenance");
        assert_eq!(stored[0].unit_amount, 450.0);
    }

    #[test]
    fn a_schedule_refuses_what_an_invoice_would_refuse() {
        let (_d, conn) = test_conn();
        let client = add_client(&conn, "Globex", Some("ap@globex.test"), None, None).unwrap();
        let base = NewSchedule {
            client_id: client,
            cadence: Cadence::Monthly,
            anchor_day: 1,
            start_period: "2026-01-01".into(),
            net_days: None,
            currency: "USD".into(),
            notes: None,
            terms: None,
            autosend: false,
            items: items(),
        };

        let empty = NewSchedule { items: vec![], ..base.clone() };
        assert!(add_schedule(&conn, &empty).is_err(), "no line items");

        let bad_currency = NewSchedule { currency: "DOLLARS".into(), ..base.clone() };
        assert!(add_schedule(&conn, &bad_currency).is_err(), "currency");

        let bad_date = NewSchedule { start_period: "26-1-1".into(), ..base.clone() };
        assert!(add_schedule(&conn, &bad_date).is_err(), "start period");

        let bad_anchor = NewSchedule { anchor_day: 32, ..base.clone() };
        assert!(add_schedule(&conn, &bad_anchor).is_err(), "anchor day");

        archive_client(&conn, client, "2026-02-01").unwrap();
        let err = add_schedule(&conn, &base).unwrap_err();
        assert!(
            matches!(err, NigelError::Conflict { code: "client_archived", .. }),
            "got: {err:?}"
        );
    }

    #[test]
    fn cadence_round_trips_through_its_stored_word() {
        for (word, cadence, months) in [
            ("monthly", Cadence::Monthly, 1),
            ("quarterly", Cadence::Quarterly, 3),
            ("yearly", Cadence::Yearly, 12),
        ] {
            assert_eq!(Cadence::parse(word).unwrap(), cadence);
            assert_eq!(cadence.as_str(), word);
            assert_eq!(cadence.months(), months);
        }
        assert!(Cadence::parse("weekly").is_err());
    }

    #[test]
    fn a_monthly_anchor_clamps_in_short_months_and_returns_to_the_anchor() {
        // AC #6: the anchor is remembered, never the clamped day it produced.
        let walk = [
            ("2026-01-31", "2026-02-28"),
            ("2026-02-28", "2026-03-31"),
            ("2026-03-31", "2026-04-30"),
            ("2026-04-30", "2026-05-31"),
            ("2026-11-30", "2026-12-31"),
            ("2026-12-31", "2027-01-31"),
            // A leap February takes the 29th.
            ("2028-01-31", "2028-02-29"),
        ];
        for (from, to) in walk {
            assert_eq!(advance_period(Cadence::Monthly, 31, from).unwrap(), to, "{from}");
        }

        assert_eq!(advance_period(Cadence::Quarterly, 30, "2026-01-30").unwrap(), "2026-04-30");
        assert_eq!(advance_period(Cadence::Quarterly, 31, "2026-11-30").unwrap(), "2027-02-28");
        assert_eq!(advance_period(Cadence::Yearly, 29, "2028-02-29").unwrap(), "2029-02-28");
    }

    #[test]
    fn clamp_day_answers_the_last_valid_day_of_the_month() {
        assert_eq!(clamp_day(2026, 2, 31), 28);
        assert_eq!(clamp_day(2028, 2, 31), 29);
        assert_eq!(clamp_day(2026, 4, 31), 30);
        assert_eq!(clamp_day(2026, 12, 31), 31);
        assert_eq!(clamp_day(2026, 1, 15), 15);
    }

    #[test]
    fn pausing_resuming_and_ending_leave_the_row_and_its_history_alone() {
        // AC #8.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);

        pause_schedule(&conn, id).unwrap();
        assert!(get_schedule(&conn, id).unwrap().paused);
        assert!(list_schedules(&conn, ScheduleScope::Active).unwrap().is_empty());
        assert_eq!(list_schedules(&conn, ScheduleScope::All).unwrap().len(), 1);

        resume_schedule(&conn, id).unwrap();
        assert!(!get_schedule(&conn, id).unwrap().paused);
        assert_eq!(list_schedules(&conn, ScheduleScope::Active).unwrap().len(), 1);

        end_schedule(&conn, id, "2026-08-20").unwrap();
        let ended = get_schedule(&conn, id).unwrap();
        assert_eq!(ended.ended_at.as_deref(), Some("2026-08-20"));
        assert!(list_schedules(&conn, ScheduleScope::Active).unwrap().is_empty());
        // Nothing is deleted: the row and its items are still readable.
        assert_eq!(schedule_items(&conn, id).unwrap().len(), 1);
        assert_eq!(list_schedules(&conn, ScheduleScope::All).unwrap().len(), 1);
    }

    #[test]
    fn editing_replaces_only_what_is_given() {
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);

        update_schedule(
            &conn,
            id,
            &ScheduleUpdate {
                net_days: Some(Some(14)),
                autosend: Some(true),
                items: Some(vec![NewLineItem {
                    description: "Hosting & maintenance".into(),
                    quantity: 1.0,
                    unit_amount: 495.0,
                }]),
                ..ScheduleUpdate::default()
            },
        )
        .unwrap();

        let schedule = get_schedule(&conn, id).unwrap();
        assert_eq!(schedule.net_days, Some(14));
        assert!(schedule.autosend);
        assert_eq!(schedule.currency, "USD", "an omitted field is left alone");
        assert_eq!(schedule.notes.as_deref(), Some("Monthly hosting."));
        assert_eq!(schedule.next_period, "2026-01-01", "editing never moves the cycle");

        let stored = schedule_items(&conn, id).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].unit_amount, 495.0);

        // `null` clears, which is what a nested `Some(None)` means.
        update_schedule(
            &conn,
            id,
            &ScheduleUpdate { net_days: Some(None), notes: Some(None), ..ScheduleUpdate::default() },
        )
        .unwrap();
        let schedule = get_schedule(&conn, id).unwrap();
        assert_eq!(schedule.net_days, None);
        assert_eq!(schedule.notes, None);

        let empty = update_schedule(&conn, id, &ScheduleUpdate::default());
        assert!(empty.is_err(), "an edit with nothing in it is a refusal");
    }

    #[test]
    fn a_schedule_that_is_not_there_is_not_found() {
        let (_d, conn) = test_conn();
        for result in [
            get_schedule(&conn, 99).map(|_| ()),
            pause_schedule(&conn, 99),
            resume_schedule(&conn, 99),
            end_schedule(&conn, 99, "2026-08-20"),
        ] {
            assert!(matches!(result.unwrap_err(), NigelError::NotFound(_)));
        }
    }
}
```

- [ ] **Step 6: Run it to verify it fails**

Run: `cargo test -p nigel-core invoicing::schedules -- --test-threads=1`
Expected: FAIL to compile — the module is not declared and nothing it names
exists.

- [ ] **Step 7: Write the module.** Add `pub mod schedules;` to
  `crates/nigel-core/src/invoicing/mod.rs` (alphabetically, between `render_html`
  and `send` — the list is `republish`, `schedules`, `send`), then put this
  above the test module in `crates/nigel-core/src/invoicing/schedules.rs`:

```rust
//! Recurring invoice schedules: a stored shape, a cadence, and the history of
//! what it has already produced.
//!
//! Nothing here reads the clock or the settings. A run's reference day is a
//! parameter, and the collaborators an autosend needs are injected by the
//! caller — the rule the whole of `src/invoicing/` keeps.

use chrono::NaiveDate;
use rusqlite::Connection;
use serde::Serialize;

use crate::error::{NigelError, Result};
use crate::invoicing::clients::ensure_client_active;
use crate::invoicing::invoices::{
    validate_currency, validate_date, validate_items, NewLineItem,
};

/// How often a schedule bills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cadence {
    Monthly,
    Quarterly,
    Yearly,
}

impl Cadence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Monthly => "monthly",
            Self::Quarterly => "quarterly",
            Self::Yearly => "yearly",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "monthly" => Ok(Self::Monthly),
            "quarterly" => Ok(Self::Quarterly),
            "yearly" => Ok(Self::Yearly),
            other => Err(NigelError::Invalid(format!(
                "Unknown cadence: {other} (expected monthly, quarterly or yearly)"
            ))),
        }
    }

    /// How many months one cycle covers.
    pub fn months(self) -> u32 {
        match self {
            Self::Monthly => 1,
            Self::Quarterly => 3,
            Self::Yearly => 12,
        }
    }
}

/// Which schedules a listing wants. `Active` is what a run walks: not paused
/// and not ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleScope {
    Active,
    All,
}

/// A schedule being created.
#[derive(Debug, Clone)]
pub struct NewSchedule {
    pub client_id: i64,
    pub cadence: Cadence,
    /// The day of the month the cycle is anchored on, `1..=31`. Remembered
    /// rather than clamped, so a short month does not permanently move it.
    pub anchor_day: u32,
    /// The first period's issue date.
    pub start_period: String,
    pub net_days: Option<i64>,
    pub currency: String,
    pub notes: Option<String>,
    pub terms: Option<String>,
    pub autosend: bool,
    pub items: Vec<NewLineItem>,
}

/// A stored schedule.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    pub id: i64,
    pub client_id: i64,
    pub cadence: String,
    pub anchor_day: u32,
    pub next_period: String,
    pub net_days: Option<i64>,
    pub currency: String,
    pub notes: Option<String>,
    pub terms: Option<String>,
    pub autosend: bool,
    pub paused: bool,
    pub ended_at: Option<String>,
}

/// A partial edit. `Option<Option<T>>` is `InvoiceUpdate`'s shape and means the
/// same thing: absent leaves the field alone, `Some(None)` clears it.
#[derive(Debug, Clone, Default)]
pub struct ScheduleUpdate {
    pub anchor_day: Option<u32>,
    pub net_days: Option<Option<i64>>,
    pub currency: Option<String>,
    pub notes: Option<Option<String>>,
    pub terms: Option<Option<String>>,
    pub autosend: Option<bool>,
    pub items: Option<Vec<NewLineItem>>,
}

impl ScheduleUpdate {
    pub fn is_empty(&self) -> bool {
        self.anchor_day.is_none()
            && self.net_days.is_none()
            && self.currency.is_none()
            && self.notes.is_none()
            && self.terms.is_none()
            && self.autosend.is_none()
            && self.items.is_none()
    }
}

/// One invoice a schedule has already produced.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleRun {
    pub period: String,
    pub invoice_id: i64,
    pub number: i64,
    pub generated_at: String,
}

const SCHEDULE_COLS: &str = "id, client_id, cadence, anchor_day, next_period, net_days,
    currency, notes, terms, autosend, paused, ended_at";

fn row_to_schedule(r: &rusqlite::Row) -> rusqlite::Result<Schedule> {
    Ok(Schedule {
        id: r.get(0)?,
        client_id: r.get(1)?,
        cadence: r.get(2)?,
        anchor_day: r.get::<_, i64>(3)? as u32,
        next_period: r.get(4)?,
        net_days: r.get(5)?,
        currency: r.get(6)?,
        notes: r.get(7)?,
        terms: r.get(8)?,
        autosend: r.get(9)?,
        paused: r.get(10)?,
        ended_at: r.get(11)?,
    })
}

/// `1..=31`, the range the table's own CHECK enforces — refused here so the
/// sentence names the field rather than the constraint.
fn validate_anchor_day(day: u32) -> Result<u32> {
    if (1..=31).contains(&day) {
        return Ok(day);
    }
    Err(NigelError::Invalid(format!(
        "Anchor day must be between 1 and 31, got {day}."
    )))
}

/// Create a schedule and its line items in one transaction.
///
/// Validated with the invoice writers' own rules, so a schedule cannot be
/// created that would refuse on its first run: at least one line item, finite
/// figures, a total above zero, a real currency, a real start date, and a
/// client that is not archived.
pub fn add_schedule(conn: &Connection, new: &NewSchedule) -> Result<i64> {
    ensure_client_active(conn, new.client_id)?;
    validate_items(&new.items)?;
    let start_period = validate_date(&new.start_period, "start")?;
    let currency = validate_currency(&new.currency)?;
    let anchor_day = validate_anchor_day(new.anchor_day)?;

    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO invoice_schedules
            (client_id, cadence, anchor_day, next_period, net_days, currency, notes, terms, autosend)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            new.client_id,
            new.cadence.as_str(),
            anchor_day,
            start_period,
            new.net_days,
            currency,
            new.notes,
            new.terms,
            new.autosend,
        ],
    )?;
    let id = tx.last_insert_rowid();
    write_items(&tx, id, &new.items)?;
    tx.commit()?;
    Ok(id)
}

/// Rewrite a schedule's line items at dense positions `0..n-1`.
fn write_items(conn: &Connection, schedule_id: i64, items: &[NewLineItem]) -> Result<()> {
    conn.execute(
        "DELETE FROM invoice_schedule_items WHERE schedule_id = ?1",
        [schedule_id],
    )?;
    for (idx, item) in items.iter().enumerate() {
        conn.execute(
            "INSERT INTO invoice_schedule_items
                (schedule_id, description, quantity, unit_amount, position)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                schedule_id,
                item.description,
                item.quantity,
                item.unit_amount,
                idx as i64
            ],
        )?;
    }
    Ok(())
}

pub fn get_schedule(conn: &Connection, id: i64) -> Result<Schedule> {
    conn.query_row(
        &format!("SELECT {SCHEDULE_COLS} FROM invoice_schedules WHERE id = ?1"),
        [id],
        row_to_schedule,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            NigelError::NotFound(format!("Invoice schedule not found: id {id}"))
        }
        other => NigelError::Db(other),
    })
}

pub fn list_schedules(conn: &Connection, scope: ScheduleScope) -> Result<Vec<Schedule>> {
    let filter = match scope {
        ScheduleScope::Active => "WHERE paused = 0 AND ended_at IS NULL",
        ScheduleScope::All => "",
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT {SCHEDULE_COLS} FROM invoice_schedules {filter} ORDER BY id"
    ))?;
    let rows = stmt
        .query_map([], row_to_schedule)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The schedule's own line items, re-read per run — editing a schedule changes
/// future invoices and never past ones.
pub fn schedule_items(conn: &Connection, schedule_id: i64) -> Result<Vec<NewLineItem>> {
    let mut stmt = conn.prepare(
        "SELECT description, quantity, unit_amount
           FROM invoice_schedule_items WHERE schedule_id = ?1 ORDER BY position",
    )?;
    let rows = stmt
        .query_map([schedule_id], |r| {
            Ok(NewLineItem {
                description: r.get(0)?,
                quantity: r.get(1)?,
                unit_amount: r.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Apply a partial edit. `next_period` is never touched: editing a schedule
/// changes what it bills, not where it is in its cycle.
pub fn update_schedule(conn: &Connection, id: i64, update: &ScheduleUpdate) -> Result<()> {
    let _ = get_schedule(conn, id)?;
    if update.is_empty() {
        return Err(NigelError::Invalid(
            "Nothing to change. Pass at least one field to edit.".to_string(),
        ));
    }
    if let Some(items) = &update.items {
        validate_items(items)?;
    }
    let anchor_day = update.anchor_day.map(validate_anchor_day).transpose()?;
    let currency = update
        .currency
        .as_deref()
        .map(|c| validate_currency(c))
        .transpose()?;

    let tx = conn.unchecked_transaction()?;
    if let Some(day) = anchor_day {
        tx.execute(
            "UPDATE invoice_schedules SET anchor_day = ?2 WHERE id = ?1",
            rusqlite::params![id, day],
        )?;
    }
    if let Some(currency) = currency {
        tx.execute(
            "UPDATE invoice_schedules SET currency = ?2 WHERE id = ?1",
            rusqlite::params![id, currency],
        )?;
    }
    if let Some(net_days) = update.net_days {
        tx.execute(
            "UPDATE invoice_schedules SET net_days = ?2 WHERE id = ?1",
            rusqlite::params![id, net_days],
        )?;
    }
    if let Some(notes) = &update.notes {
        tx.execute(
            "UPDATE invoice_schedules SET notes = ?2 WHERE id = ?1",
            rusqlite::params![id, notes],
        )?;
    }
    if let Some(terms) = &update.terms {
        tx.execute(
            "UPDATE invoice_schedules SET terms = ?2 WHERE id = ?1",
            rusqlite::params![id, terms],
        )?;
    }
    if let Some(autosend) = update.autosend {
        tx.execute(
            "UPDATE invoice_schedules SET autosend = ?2 WHERE id = ?1",
            rusqlite::params![id, autosend],
        )?;
    }
    if let Some(items) = &update.items {
        write_items(&tx, id, items)?;
    }
    tx.commit()?;
    Ok(())
}

pub fn pause_schedule(conn: &Connection, id: i64) -> Result<()> {
    let _ = get_schedule(conn, id)?;
    conn.execute("UPDATE invoice_schedules SET paused = 1 WHERE id = ?1", [id])?;
    Ok(())
}

pub fn resume_schedule(conn: &Connection, id: i64) -> Result<()> {
    let _ = get_schedule(conn, id)?;
    conn.execute("UPDATE invoice_schedules SET paused = 0 WHERE id = ?1", [id])?;
    Ok(())
}

/// Stop a schedule for good. A timestamp rather than a delete, the way
/// `voided_at` and `archived_at` are: the invoices it produced keep their
/// provenance, and the history stays readable.
pub fn end_schedule(conn: &Connection, id: i64, on: &str) -> Result<()> {
    let _ = get_schedule(conn, id)?;
    let on = validate_date(on, "end")?;
    conn.execute(
        "UPDATE invoice_schedules SET ended_at = ?2 WHERE id = ?1",
        rusqlite::params![id, on],
    )?;
    Ok(())
}

/// What a schedule has already produced, oldest period first.
pub fn schedule_runs(conn: &Connection, schedule_id: i64) -> Result<Vec<ScheduleRun>> {
    let mut stmt = conn.prepare(
        "SELECT r.period, r.invoice_id, i.number, r.generated_at
           FROM invoice_schedule_runs r
           JOIN invoices i ON i.id = r.invoice_id
          WHERE r.schedule_id = ?1
          ORDER BY r.period",
    )?;
    let rows = stmt
        .query_map([schedule_id], |r| {
            Ok(ScheduleRun {
                period: r.get(0)?,
                invoice_id: r.get(1)?,
                number: r.get(2)?,
                generated_at: r.get(3)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Clamp a day to the last valid day of the given year and month.
pub fn clamp_day(year: i32, month: u32, day: u32) -> u32 {
    let first_of_next = NaiveDate::from_ymd_opt(year, month + 1, 1)
        .or_else(|| NaiveDate::from_ymd_opt(year + 1, 1, 1))
        .expect("valid year for date arithmetic");
    let last_day = first_of_next
        .pred_opt()
        .expect("predecessor of first-of-month is valid")
        .day();
    day.min(last_day)
}

/// The period after `period` for this cadence.
///
/// Anchored on `anchor_day` and clamped into short months, so a schedule
/// anchored on the 31st bills the 30th in April and the 31st again in May: the
/// **anchor** is what advances, never the clamped day it produced last time.
pub fn advance_period(cadence: Cadence, anchor_day: u32, period: &str) -> Result<String> {
    let anchor_day = validate_anchor_day(anchor_day)?;
    let current = crate::invoicing::invoices::parse_date(period, "period")?;
    let zero_based = (current.month() - 1) + cadence.months();
    let year = current.year() + (zero_based / 12) as i32;
    let month = zero_based % 12 + 1;
    let day = clamp_day(year, month, anchor_day);
    Ok(format!("{year:04}-{month:02}-{day:02}"))
}
```

  `advance_period` needs `use chrono::Datelike;` beside the `NaiveDate` import.

- [ ] **Step 8: Run the data-layer tests to verify they pass**

Run: `cargo test -p nigel-core invoicing::schedules -- --test-threads=1`
Expected: PASS, 8 tests.

- [ ] **Step 9: Full check and commit**

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
./scripts/check-no-real-data.sh --staged; echo "exit=$?"
git add crates/nigel-core/src/migrations.rs crates/nigel-core/src/invoicing/mod.rs crates/nigel-core/src/invoicing/schedules.rs
git commit -m "Store recurring invoice schedules, their items and their history"
```
Expected: clean, `exit=0`, hook passes.

---

### Task 6: The generator

**Files:**
- Modify: `crates/nigel-core/src/invoicing/invoices.rs` (extract `insert_invoice`)
- Modify: `crates/nigel-core/src/invoicing/schedules.rs`
- Test: `crates/nigel-core/src/invoicing/schedules.rs` (`mod tests`)

**Interfaces:**
- Consumes: Task 5's `Schedule`, `list_schedules`, `schedule_items`,
  `advance_period`; `invoices::plus_days` (Task 1); `send::send_invoice`,
  `send::require_email`; the `PaymentGateway`/`AssetPublisher`/`Mailer` traits.
- Produces:
  ```rust
  // invoices.rs
  pub(in crate::invoicing) fn insert_invoice(
      conn: &Connection, client_id: i64, issue_date: &str, due_date: Option<&str>,
      currency: &str, items: &[NewLineItem], notes: Option<&str>, terms: Option<&str>,
  ) -> Result<i64>;

  // schedules.rs
  pub struct Senders<'a, G, P, M> {
      pub branding: &'a Branding<'a>,
      pub gateway: &'a G,
      pub publisher: &'a P,
      pub mailer: &'a M,
  }
  pub struct Generated {
      pub schedule_id: i64, pub period: String, pub invoice_id: i64, pub number: i64,
      pub client_name: String, pub total: f64, pub currency: String,
      pub sent: bool, pub not_sent: Option<String>,
  }
  pub struct ScheduleFailure { pub schedule_id: i64, pub period: String, pub message: String }
  pub struct ScheduleRunReport { pub generated: Vec<Generated>, pub failures: Vec<ScheduleFailure> }
  impl ScheduleRunReport { pub fn has_failures(&self) -> bool; }
  pub fn run_due_schedules<G: PaymentGateway, P: AssetPublisher, M: Mailer>(
      conn: &Connection, today: &str, senders: Option<&Senders<'_, G, P, M>>,
  ) -> Result<ScheduleRunReport>;
  pub fn draft_due_schedules(conn: &Connection, today: &str) -> Result<ScheduleRunReport>;
  ```

- [ ] **Step 1: Write the failing generator tests.** Append to
  `mod tests` in `crates/nigel-core/src/invoicing/schedules.rs`:

```rust
    fn numbers(report: &ScheduleRunReport) -> Vec<i64> {
        report.generated.iter().map(|g| g.number).collect()
    }

    fn periods(report: &ScheduleRunReport) -> Vec<String> {
        report.generated.iter().map(|g| g.period.clone()).collect()
    }

    fn issued(conn: &Connection, number: i64) -> (String, Option<String>) {
        conn.query_row(
            "SELECT issue_date, due_date FROM invoices WHERE number = ?1",
            [number],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    }

    #[test]
    fn a_run_generates_every_missed_cycle_dated_by_its_own_period() {
        // AC #5: catch-up bills every missed cycle, in order, dated the period's
        // issue date rather than the run day.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);

        let report = draft_due_schedules(&conn, "2026-04-15").unwrap();
        assert_eq!(periods(&report), ["2026-01-01", "2026-02-01", "2026-03-01", "2026-04-01"]);
        assert_eq!(numbers(&report), [1248, 1249, 1250, 1251], "AC #7: sequential");

        assert_eq!(issued(&conn, 1248), ("2026-01-01".into(), Some("2026-01-31".into())));
        assert_eq!(issued(&conn, 1251), ("2026-04-01".into(), Some("2026-05-01".into())));

        assert_eq!(get_schedule(&conn, id).unwrap().next_period, "2026-05-01");
        for generated in &report.generated {
            assert!(!generated.sent, "AC #4: drafting is the default");
            assert_eq!(generated.not_sent, None);
            assert_eq!(generated.client_name, "Cedar Systems");
            assert_eq!(generated.total, 450.0);
        }
        assert!(report.failures.is_empty());
        assert!(!report.has_failures());
    }

    #[test]
    fn running_twice_for_the_same_period_generates_nothing_the_second_time() {
        // AC #3, by recorded provenance rather than date inference.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);

        let first = draft_due_schedules(&conn, "2026-02-15").unwrap();
        assert_eq!(numbers(&first), [1248, 1249]);

        let second = draft_due_schedules(&conn, "2026-02-15").unwrap();
        assert!(second.generated.is_empty(), "{:?}", second.generated);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM invoices", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);

        let runs = schedule_runs(&conn, id).unwrap();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].period, "2026-01-01");
        assert_eq!(runs[0].number, 1248);
        assert_eq!(runs[1].period, "2026-02-01");
        assert_eq!(runs[1].generated_at, "2026-02-15");
    }

    #[test]
    fn a_run_row_already_there_advances_the_cycle_without_billing_again() {
        // The row is the authority even if `next_period` was rewound by hand.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);
        draft_due_schedules(&conn, "2026-01-15").unwrap();

        conn.execute(
            "UPDATE invoice_schedules SET next_period = '2026-01-01' WHERE id = ?1",
            [id],
        )
        .unwrap();

        let again = draft_due_schedules(&conn, "2026-01-15").unwrap();
        assert!(again.generated.is_empty());
        assert_eq!(get_schedule(&conn, id).unwrap().next_period, "2026-02-01");
    }

    #[test]
    fn a_monthly_schedule_anchored_at_month_end_bills_the_short_months() {
        // AC #6, end to end through a run.
        let (_d, conn) = test_conn();
        let client = add_client(&conn, "Initech", Some("ap@initech.test"), None, None).unwrap();
        add_schedule(
            &conn,
            &NewSchedule {
                client_id: client,
                cadence: Cadence::Monthly,
                anchor_day: 31,
                start_period: "2026-01-31".into(),
                net_days: None,
                currency: "USD".into(),
                notes: None,
                terms: None,
                autosend: false,
                items: items(),
            },
        )
        .unwrap();

        let report = draft_due_schedules(&conn, "2026-05-15").unwrap();
        assert_eq!(
            periods(&report),
            ["2026-01-31", "2026-02-28", "2026-03-31", "2026-04-30"]
        );
        assert_eq!(issued(&conn, 1251), ("2026-04-30".into(), None));
    }

    #[test]
    fn paused_and_ended_schedules_generate_nothing() {
        let (_d, conn) = test_conn();
        let (_client, paused) = seed(&conn);
        pause_schedule(&conn, paused).unwrap();
        assert!(draft_due_schedules(&conn, "2026-06-01").unwrap().generated.is_empty());

        resume_schedule(&conn, paused).unwrap();
        end_schedule(&conn, paused, "2026-01-01").unwrap();
        assert!(draft_due_schedules(&conn, "2026-06-01").unwrap().generated.is_empty());
    }

    #[test]
    fn several_schedules_at_once_keep_the_numbering_sequential() {
        // AC #7 across schedules, not just within one.
        let (_d, conn) = test_conn();
        seed(&conn);
        let other = add_client(&conn, "Juniper Labs", Some("ap@juniper.test"), None, None).unwrap();
        add_schedule(
            &conn,
            &NewSchedule {
                client_id: other,
                cadence: Cadence::Quarterly,
                anchor_day: 1,
                start_period: "2026-01-01".into(),
                net_days: None,
                currency: "USD".into(),
                notes: None,
                terms: None,
                autosend: false,
                items: items(),
            },
        )
        .unwrap();

        let report = draft_due_schedules(&conn, "2026-04-15").unwrap();
        let all = numbers(&report);
        assert_eq!(all.len(), 6, "four monthly plus two quarterly: {all:?}");
        let mut sorted = all.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, (1248..=1253).collect::<Vec<_>>(), "{all:?}");
    }

    #[test]
    fn a_schedule_whose_client_was_archived_is_reported_and_does_not_stop_the_others() {
        let (_d, conn) = test_conn();
        let (archived_client, archived) = seed(&conn);
        let ok_client = add_client(&conn, "Globex", Some("ap@globex.test"), None, None).unwrap();
        add_schedule(
            &conn,
            &NewSchedule {
                client_id: ok_client,
                cadence: Cadence::Monthly,
                anchor_day: 1,
                start_period: "2026-01-01".into(),
                net_days: None,
                currency: "USD".into(),
                notes: None,
                terms: None,
                autosend: false,
                items: items(),
            },
        )
        .unwrap();
        archive_client(&conn, archived_client, "2026-01-01").unwrap();

        let report = draft_due_schedules(&conn, "2026-02-15").unwrap();
        assert_eq!(numbers(&report), [1248, 1249], "the healthy schedule still billed");
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].schedule_id, archived);
        assert!(report.failures[0].message.contains("archived"), "{:?}", report.failures[0]);
        assert!(report.has_failures());
    }

    #[test]
    fn an_autosend_schedule_with_nothing_to_send_through_still_drafts_and_says_why() {
        // AC #9: reported, never silently skipped, never half-sent.
        let (_d, conn) = test_conn();
        let (_client, id) = seed(&conn);
        update_schedule(
            &conn,
            id,
            &ScheduleUpdate { autosend: Some(true), ..ScheduleUpdate::default() },
        )
        .unwrap();

        let report = draft_due_schedules(&conn, "2026-01-15").unwrap();
        assert_eq!(numbers(&report), [1248]);
        let generated = &report.generated[0];
        assert!(!generated.sent);
        assert_eq!(
            generated.not_sent.as_deref(),
            Some("sending is not configured on this installation")
        );
        assert!(report.has_failures(), "cron has to see this in the exit status");

        let status: String = conn
            .query_row("SELECT status FROM invoices WHERE number = 1248", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "draft");
    }
```

  And a `pdf`-gated module for the sending half — sending renders a PDF, which
  is exactly why `send.rs`'s own tests are gated the same way. Put it at the
  bottom of the file:

```rust
#[cfg(all(test, feature = "pdf"))]
mod autosend_tests {
    use super::tests::*;
    use super::*;
    use crate::invoicing::gateway::{
        fake_logo_publishing, AssetPublisher, Mailer, PaidSession, PaymentGateway, PaymentLink,
    };
    use crate::invoicing::render_html::DEFAULT_TEMPLATE;
    use crate::models::{Client, Invoice};
    use std::cell::RefCell;

    struct Gateway;
    impl PaymentGateway for Gateway {
        fn create_payment_link(&self, invoice: &Invoice, _c: &Client) -> Result<PaymentLink> {
            Ok(PaymentLink {
                id: format!("plink_{}", invoice.number),
                url: format!("https://pay.example.test/{}", invoice.number),
            })
        }
        fn paid_sessions(&self, _id: &str) -> Result<Vec<PaidSession>> {
            Ok(Vec::new())
        }
        fn deactivate_payment_link(&self, _id: &str) -> Result<()> {
            Ok(())
        }
    }

    struct Publisher;
    impl AssetPublisher for Publisher {
        fn publish(&self, token: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
            Ok(format!("https://billing.example.test/i/{token}/index.html"))
        }
        fn publish_page(&self, token: &str, _h: &[u8]) -> Result<String> {
            Ok(format!("https://billing.example.test/i/{token}/index.html"))
        }
        fake_logo_publishing!("https://billing.example.test/i");
    }

    #[derive(Default)]
    struct Post {
        to: RefCell<Vec<String>>,
    }
    impl Mailer for Post {
        fn send_invoice(&self, to: &str, _cc: &[String], _s: &str, _h: &str, _p: &[u8]) -> Result<()> {
            self.to.borrow_mut().push(to.to_string());
            Ok(())
        }
    }

    #[test]
    fn an_autosend_schedule_sends_and_a_drafting_one_beside_it_does_not() {
        // AC #4: explicit per schedule, drafting the default.
        let (_d, conn) = test_conn();
        let (_client, drafting) = seed(&conn);
        let sending_client =
            crate::invoicing::clients::add_client(&conn, "Globex", Some("ap@globex.test"), None, None)
                .unwrap();
        let sending = add_schedule(
            &conn,
            &NewSchedule {
                client_id: sending_client,
                cadence: Cadence::Monthly,
                anchor_day: 1,
                start_period: "2026-01-01".into(),
                net_days: None,
                currency: "USD".into(),
                notes: None,
                terms: None,
                autosend: true,
                items: sample_items(),
            },
        )
        .unwrap();

        let branding = crate::invoicing::render_html::Branding {
            company: "Bluepeak",
            contact_email: "billing@bluepeak.test",
            ..crate::invoicing::render_html::Branding::with_template(DEFAULT_TEMPLATE)
        };
        let post = Post::default();
        let senders = Senders {
            branding: &branding,
            gateway: &Gateway,
            publisher: &Publisher,
            mailer: &post,
        };

        let report = run_due_schedules(&conn, "2026-01-15", Some(&senders)).unwrap();
        assert_eq!(report.generated.len(), 2);
        assert!(!report.has_failures(), "{report:?}");

        let drafted = report.generated.iter().find(|g| g.schedule_id == drafting).unwrap();
        assert!(!drafted.sent);
        assert_eq!(drafted.not_sent, None);

        let sent = report.generated.iter().find(|g| g.schedule_id == sending).unwrap();
        assert!(sent.sent);
        assert_eq!(post.to.borrow().as_slice(), ["ap@globex.test"]);

        let status: String = conn
            .query_row("SELECT status FROM invoices WHERE number = ?1", [sent.number], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "sent");
    }

    #[test]
    fn an_unsendable_client_still_gets_its_draft_and_the_run_says_why() {
        // AC #9, with sending fully configured: the refusal is about the client.
        let (_d, conn) = test_conn();
        let no_address =
            crate::invoicing::clients::add_client(&conn, "Harbor & Vale", None, None, None).unwrap();
        add_schedule(
            &conn,
            &NewSchedule {
                client_id: no_address,
                cadence: Cadence::Monthly,
                anchor_day: 1,
                start_period: "2026-01-01".into(),
                net_days: None,
                currency: "USD".into(),
                notes: None,
                terms: None,
                autosend: true,
                items: sample_items(),
            },
        )
        .unwrap();

        let branding = crate::invoicing::render_html::Branding {
            company: "Bluepeak",
            contact_email: "billing@bluepeak.test",
            ..crate::invoicing::render_html::Branding::with_template(DEFAULT_TEMPLATE)
        };
        let post = Post::default();
        let senders = Senders {
            branding: &branding,
            gateway: &Gateway,
            publisher: &Publisher,
            mailer: &post,
        };

        let report = run_due_schedules(&conn, "2026-01-15", Some(&senders)).unwrap();
        assert_eq!(report.generated.len(), 1);
        let generated = &report.generated[0];
        assert!(!generated.sent);
        assert!(
            generated.not_sent.as_deref().unwrap_or_default().contains("no email"),
            "got: {:?}",
            generated.not_sent
        );
        assert!(report.has_failures());
        assert!(post.to.borrow().is_empty(), "nothing was half-sent");

        let status: String = conn
            .query_row("SELECT status FROM invoices WHERE number = 1248", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "draft");
    }
}
```

  `autosend_tests` reaches into `mod tests` for `test_conn` and `seed`; make
  those `pub(super)` in `mod tests`, and rename the fixture-item helper to
  `pub(super) fn sample_items()` there (updating its two existing call sites)
  so the name does not collide with the `items` local variables above.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p nigel-core invoicing::schedules -- --test-threads=1`
Expected: FAIL to compile — `cannot find function 'draft_due_schedules'`.

- [ ] **Step 3: Extract `insert_invoice`.** In
  `crates/nigel-core/src/invoicing/invoices.rs`, replace the body of
  `create_invoice` (from `ensure_client_active` through `tx.commit()?; Ok(invoice_id)`)
  with:

```rust
#[allow(clippy::too_many_arguments)]
pub fn create_invoice(
    conn: &Connection,
    client_id: i64,
    issue_date: &str,
    due_date: Option<&str>,
    currency: &str,
    items: &[NewLineItem],
    notes: Option<&str>,
    terms: Option<&str>,
) -> Result<i64> {
    let tx = conn.unchecked_transaction()?;
    let id = insert_invoice(
        &tx, client_id, issue_date, due_date, currency, items, notes, terms,
    )?;
    tx.commit()?;
    Ok(id)
}

/// [`create_invoice`]'s body with no transaction of its own, for a caller that
/// already has one open.
///
/// SQLite has no nested `BEGIN`, so a generator that must write the invoice and
/// the row recording *why* it exists together cannot call `create_invoice`. The
/// validation still runs before the first insert, so a refusal writes nothing
/// here either.
#[allow(clippy::too_many_arguments)]
pub(in crate::invoicing) fn insert_invoice(
    conn: &Connection,
    client_id: i64,
    issue_date: &str,
    due_date: Option<&str>,
    currency: &str,
    items: &[NewLineItem],
    notes: Option<&str>,
    terms: Option<&str>,
) -> Result<i64> {
    // Before anything is written, so a refusal leaves nothing behind. This is
    // also the existence check: `ensure_client_active` reads the row, so a
    // missing client is its `NotFound` rather than a second query's.
    ensure_client_active(conn, client_id)?;
    validate_items(items)?;
    let issue_date = validate_date(issue_date, "issue")?;
    let due_date = match due_date {
        Some(due) => Some(validate_date(due, "due")?),
        None => None,
    };
    let currency = validate_currency(currency)?;

    let number = next_number(conn)?;
    let subtotal: f64 = items.iter().map(|i| i.quantity * i.unit_amount).sum();
    let tax = 0.0;
    let total = subtotal + tax;
    let token = gen_token();

    conn.execute(
        "INSERT INTO invoices
            (number, client_id, issue_date, due_date, status, currency, subtotal, tax, total, notes, terms, token)
         VALUES (?1, ?2, ?3, ?4, 'draft', ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            number, client_id, issue_date, due_date, currency, subtotal, tax, total, notes, terms, token
        ],
    )?;
    let invoice_id = conn.last_insert_rowid();

    for (idx, item) in items.iter().enumerate() {
        let line_total = item.quantity * item.unit_amount;
        conn.execute(
            "INSERT INTO invoice_line_items
                (invoice_id, description, quantity, unit_amount, line_total, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                invoice_id,
                item.description,
                item.quantity,
                item.unit_amount,
                line_total,
                idx as i64
            ],
        )?;
    }

    set_metadata(conn, NEXT_NUMBER_KEY, &(number + 1).to_string())?;
    Ok(invoice_id)
}
```

- [ ] **Step 4: Write the generator.** Append to
  `crates/nigel-core/src/invoicing/schedules.rs`, above the test modules:

```rust
/// The collaborators an autosend run needs.
///
/// Injected, never resolved here: `src/invoicing/` does not read settings, so
/// the branding and the three clients come from the front end — the same seam
/// `send_with` and `void_with` use.
pub struct Senders<'a, G, P, M> {
    pub branding: &'a Branding<'a>,
    pub gateway: &'a G,
    pub publisher: &'a P,
    pub mailer: &'a M,
}

/// One invoice a run produced, and what happened to it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Generated {
    pub schedule_id: i64,
    /// The cycle's scheduled issue date, which is also the invoice's.
    pub period: String,
    pub invoice_id: i64,
    pub number: i64,
    pub client_name: String,
    pub total: f64,
    pub currency: String,
    pub sent: bool,
    /// Why an autosend schedule's invoice is still a draft, or `None`. Never
    /// silently empty on a schedule that asked to send.
    pub not_sent: Option<String>,
}

/// A schedule whose walk stopped, and where.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleFailure {
    pub schedule_id: i64,
    pub period: String,
    pub message: String,
}

/// What one run did. Data rather than a print, so a terminal and a browser can
/// render the same run.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleRunReport {
    pub generated: Vec<Generated>,
    pub failures: Vec<ScheduleFailure>,
}

impl ScheduleRunReport {
    /// Anything a scheduled job must see in the exit status: a schedule that
    /// could not be generated, or a send that was asked for and did not happen.
    pub fn has_failures(&self) -> bool {
        !self.failures.is_empty() || self.generated.iter().any(|g| g.not_sent.is_some())
    }
}

/// The sentence an autosend schedule earns on an installation that cannot send.
const NOT_CONFIGURED: &str = "sending is not configured on this installation";

/// Generate every invoice currently due, drafting all of them.
///
/// `run_due_schedules` with nothing to send through: an autosend schedule still
/// gets its draft and is reported as unsent, which is what makes an
/// unconfigured installation honest rather than silent.
pub fn draft_due_schedules(conn: &Connection, today: &str) -> Result<ScheduleRunReport> {
    run_due_schedules::<
        crate::invoicing::stripe::StripeClient,
        crate::invoicing::r2::R2Publisher,
        crate::invoicing::mailgun::MailgunClient,
    >(conn, today, None)
}

/// Generate every invoice currently due, sending the ones whose schedule asked.
///
/// For each active schedule, periods are walked from `next_period` through
/// `today`: **each missed cycle generates its own invoice, dated that period's
/// issue date rather than the run day**, so catch-up bills every missed cycle in
/// order and a February invoice generated in March is already due.
///
/// Each generation is one transaction — the invoice, the run row recording which
/// schedule and which period produced it, and the advanced `next_period`
/// together. Sequential numbering falls out of `next_number` running inside it.
/// A rerun finds the `UNIQUE(schedule_id, period)` row and generates nothing.
///
/// Sending happens **after** that transaction commits, because it reaches the
/// network. A send that fails leaves the invoice the draft it already was, and
/// the report names it and why.
pub fn run_due_schedules<G: PaymentGateway, P: AssetPublisher, M: Mailer>(
    conn: &Connection,
    today: &str,
    senders: Option<&Senders<'_, G, P, M>>,
) -> Result<ScheduleRunReport> {
    let today = validate_date(today, "run")?;
    let mut report = ScheduleRunReport::default();

    for schedule in list_schedules(conn, ScheduleScope::Active)? {
        let mut cursor = schedule.next_period.clone();
        // ISO YYYY-MM-DD dates compare correctly as strings, and both sides are
        // padded by their own writers.
        while cursor <= today {
            match generate_period(conn, &schedule, &cursor, &today) {
                Ok(Some(invoice_id)) => {
                    report
                        .generated
                        .push(describe(conn, &schedule, &cursor, invoice_id)?);
                }
                Ok(None) => {}
                Err(e) => {
                    // One sick schedule does not stop the others: the rest of the
                    // book still has invoices that are due.
                    report.failures.push(ScheduleFailure {
                        schedule_id: schedule.id,
                        period: cursor.clone(),
                        message: e.to_string(),
                    });
                    break;
                }
            }
            cursor = advance_period(
                Cadence::parse(&schedule.cadence)?,
                schedule.anchor_day,
                &cursor,
            )?;
        }
    }

    for generated in &mut report.generated {
        if !autosend_for(conn, generated.schedule_id)? {
            continue;
        }
        match senders {
            None => generated.not_sent = Some(NOT_CONFIGURED.to_string()),
            Some(senders) => match crate::invoicing::send::send_invoice(
                conn,
                generated.invoice_id,
                &today,
                senders.branding,
                senders.gateway,
                senders.publisher,
                senders.mailer,
            ) {
                Ok(_) => generated.sent = true,
                Err(e) => generated.not_sent = Some(e.to_string()),
            },
        }
    }

    Ok(report)
}

fn autosend_for(conn: &Connection, schedule_id: i64) -> Result<bool> {
    Ok(get_schedule(conn, schedule_id)?.autosend)
}

/// One period, in one transaction. `Ok(None)` means this period was already
/// generated — the run row is the authority, so the cycle advances and nothing
/// is billed twice.
fn generate_period(
    conn: &Connection,
    schedule: &Schedule,
    period: &str,
    generated_at: &str,
) -> Result<Option<i64>> {
    let next = advance_period(
        Cadence::parse(&schedule.cadence)?,
        schedule.anchor_day,
        period,
    )?;
    let tx = conn.unchecked_transaction()?;

    let already: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM invoice_schedule_runs WHERE schedule_id = ?1 AND period = ?2)",
        rusqlite::params![schedule.id, period],
        |r| r.get(0),
    )?;
    if already {
        tx.execute(
            "UPDATE invoice_schedules SET next_period = ?2 WHERE id = ?1 AND next_period <= ?2",
            rusqlite::params![schedule.id, next],
        )?;
        tx.commit()?;
        return Ok(None);
    }

    let items = schedule_items(&tx, schedule.id)?;
    let due_date = schedule
        .net_days
        .map(|days| crate::invoicing::invoices::plus_days(period, days))
        .transpose()?;
    let invoice_id = crate::invoicing::invoices::insert_invoice(
        &tx,
        schedule.client_id,
        period,
        due_date.as_deref(),
        &schedule.currency,
        &items,
        schedule.notes.as_deref(),
        schedule.terms.as_deref(),
    )?;
    tx.execute(
        "INSERT INTO invoice_schedule_runs (schedule_id, period, invoice_id, generated_at)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![schedule.id, period, invoice_id, generated_at],
    )?;
    tx.execute(
        "UPDATE invoice_schedules SET next_period = ?2 WHERE id = ?1",
        rusqlite::params![schedule.id, next],
    )?;
    tx.commit()?;
    Ok(Some(invoice_id))
}

fn describe(
    conn: &Connection,
    schedule: &Schedule,
    period: &str,
    invoice_id: i64,
) -> Result<Generated> {
    let invoice = crate::invoicing::invoices::get_invoice(conn, invoice_id)?;
    let client = crate::invoicing::clients::get_client(conn, schedule.client_id)?;
    Ok(Generated {
        schedule_id: schedule.id,
        period: period.to_string(),
        invoice_id,
        number: invoice.number,
        client_name: client.name,
        total: invoice.total,
        currency: invoice.currency,
        sent: false,
        not_sent: None,
    })
}
```

  Extend the file's `use` block with what the generator names:

```rust
use crate::invoicing::gateway::{AssetPublisher, Mailer, PaymentGateway};
use crate::invoicing::render_html::Branding;
```

- [ ] **Step 5: Run the generator tests to verify they pass**

Run: `cargo test -p nigel-core invoicing::schedules -- --test-threads=1`
Expected: PASS, including `autosend_tests` (the default build has `pdf`).

- [ ] **Step 6: Prove the extraction changed nothing for existing callers**

Run: `cargo test -p nigel-core invoicing -- --test-threads=1`
Expected: PASS — the whole invoicing suite, `create_invoice`'s own tests
included.

- [ ] **Step 7: Full check and commit**

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
./scripts/check-no-real-data.sh --staged; echo "exit=$?"
git add crates/nigel-core/src/invoicing/invoices.rs crates/nigel-core/src/invoicing/schedules.rs
git commit -m "Generate the invoices a schedule has due, once per period"
```
Expected: clean, `exit=0`, hook passes. The `--no-default-features` run
compiles `autosend_tests` out — check it still reports `test result: ok`.

---

### Task 7: The `invoice schedule` command tree and the unattended path

**Files:**
- Modify: `crates/nigel/src/cli/mod.rs`
- Create: `crates/nigel/src/cli/invoice_schedule.rs`
- Modify: `crates/nigel/src/cli/invoice.rs` (re-export nothing; the new module
  is registered in `cli/mod.rs`'s module list)
- Modify: `crates/nigel/src/cli/password.rs`
- Modify: `crates/nigel/src/main.rs`
- Test: `crates/nigel/tests/cli_dispatch.rs`

**Interfaces:**
- Consumes: everything Task 5 and Task 6 produced, plus
  `invoices::invoice_shape` (Task 1) for `--from`, and
  `nigel_core::invoicing::wiring::{build_clients, company_profile,
  contact_email_for_preview}`.
- Produces: `InvoiceCommands::Schedule { command: InvoiceScheduleCommands }`
  with `Add`/`List`/`Show`/`Edit`/`Pause`/`Resume`/`End`/`Run`, and
  `cli::password::unlock_without_prompting(db_path: &Path) -> Result<()>`.

- [ ] **Step 1: Write the failing tests.** Append to
  `crates/nigel/tests/cli_dispatch.rs`:

```rust
/// A client and a monthly schedule seeded from explicit items.
fn init_with_schedule(env: &TestEnv) {
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();
    env.cmd()
        .args(["client", "add", "Cedar Systems", "--email", "ops@cedar.test"])
        .assert()
        .success();
    env.cmd()
        .args([
            "invoice", "schedule", "add",
            "--client", "1",
            "--cadence", "monthly",
            "--start", "2026-01-01",
            "--net-days", "30",
            "--item", "Hosting:1:450",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created schedule 1"));
}

#[test]
fn invoice_schedule_add_list_and_show_read_back_what_was_stored() {
    let env = TestEnv::new();
    init_with_schedule(&env);

    env.cmd()
        .args(["invoice", "schedule", "list"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Cedar Systems")
                .and(predicate::str::contains("monthly"))
                .and(predicate::str::contains("2026-01-01"))
                .and(predicate::str::contains("draft")),
        );

    env.cmd()
        .args(["invoice", "schedule", "show", "1"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Hosting")
                .and(predicate::str::contains("Net days: 30")),
        );
}

#[test]
fn invoice_schedule_add_can_be_seeded_from_an_existing_invoice() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args([
            "invoice", "schedule", "add",
            "--client", "1",
            "--cadence", "quarterly",
            "--start", "2026-01-01",
            "--from", "1248",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created schedule 1"));

    env.cmd()
        .args(["invoice", "schedule", "show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Consulting"));
}

#[test]
fn invoice_schedule_run_generates_drafts_and_a_second_run_generates_nothing() {
    let env = TestEnv::new();
    init_with_schedule(&env);

    env.cmd()
        .args(["invoice", "schedule", "run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Generated"));

    let before: i64 = env
        .db()
        .query_row("SELECT COUNT(*) FROM invoices", [], |r| r.get(0))
        .unwrap();
    assert!(before > 0, "the first run billed the missed cycles");

    env.cmd()
        .args(["invoice", "schedule", "run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Generated 0 invoice(s)"));

    let after: i64 = env
        .db()
        .query_row("SELECT COUNT(*) FROM invoices", [], |r| r.get(0))
        .unwrap();
    assert_eq!(after, before, "a rerun must not bill again");
}

#[test]
fn invoice_schedule_pause_resume_and_end_keep_the_schedule_and_its_history() {
    let env = TestEnv::new();
    init_with_schedule(&env);
    env.cmd().args(["invoice", "schedule", "run"]).assert().success();

    env.cmd().args(["invoice", "schedule", "pause", "1"]).assert().success();
    env.cmd()
        .args(["invoice", "schedule", "run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Generated 0 invoice(s)"));

    env.cmd().args(["invoice", "schedule", "resume", "1"]).assert().success();
    env.cmd()
        .args(["invoice", "schedule", "edit", "1", "--item", "Hosting:1:495"])
        .assert()
        .success();
    env.cmd().args(["invoice", "schedule", "end", "1"]).assert().success();

    env.cmd()
        .args(["invoice", "schedule", "list", "--all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ended"));

    // History survives: the run rows are still joined to their invoices.
    env.cmd()
        .args(["invoice", "schedule", "show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2026-01-01"));
}

/// TASK-81 AC #10. The stderr predicate is what matters: reaching the prompt
/// with no terminal errors with ENXIO, which satisfies a bare `.failure()`.
/// The timeout is a backstop for a run that inherits a tty and blocks.
#[test]
fn invoice_schedule_run_never_prompts_on_an_encrypted_database() {
    let env = TestEnv::new();
    init_with_schedule(&env);
    env.encrypt("hunter2");

    // With the password: it runs unattended.
    env.cmd()
        .args(["invoice", "schedule", "run"])
        .env("NIGEL_DB_PASSWORD", "hunter2")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("Generated"));

    // Without it: a clear sentence, immediately, and never a prompt.
    env.cmd()
        .args(["invoice", "schedule", "run"])
        .env_remove("NIGEL_DB_PASSWORD")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("NIGEL_DB_PASSWORD")
                .and(predicate::str::contains("never prompts")),
        );

    // A wrong password fails fast with the documented sentence.
    env.cmd()
        .args(["invoice", "schedule", "run"])
        .env("NIGEL_DB_PASSWORD", "wrong-password")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(predicate::str::contains("NIGEL_DB_PASSWORD"));
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p nigel --test cli_dispatch invoice_schedule -- --test-threads=1`
Expected: FAIL — `unrecognized subcommand 'schedule'`.

- [ ] **Step 3: Add the subcommand tree.** In `crates/nigel/src/cli/mod.rs`,
  add to `pub enum InvoiceCommands`, after `Template`:

```rust
    /// Manage recurring invoice schedules.
    Schedule {
        #[command(subcommand)]
        command: InvoiceScheduleCommands,
    },
```

  and the enum itself, beside `InvoiceTemplateCommands`:

```rust
#[derive(Subcommand)]
pub enum InvoiceScheduleCommands {
    /// Create a schedule. Line items as "desc:qty:unit", repeatable, or seed
    /// them from an existing invoice with --from.
    Add {
        /// Client ID (shown in `nigel client list`)
        #[arg(long)]
        client: i64,
        /// Cadence: monthly, quarterly, or yearly
        #[arg(long)]
        cadence: String,
        /// First period's issue date: YYYY-MM-DD
        #[arg(long)]
        start: String,
        /// Day of the month to bill on, clamped in short months (default: the start date's day)
        #[arg(long = "anchor-day")]
        anchor_day: Option<u32>,
        /// Days from issue to due on each generated invoice
        #[arg(long = "net-days")]
        net_days: Option<i64>,
        /// Currency code
        #[arg(long, default_value = "USD")]
        currency: String,
        /// Line item as "desc:qty:unit" (repeatable)
        #[arg(long = "item")]
        items: Vec<String>,
        /// Seed the items, currency, notes and terms from this invoice number
        #[arg(long = "from", conflicts_with = "items")]
        from: Option<i64>,
        /// Notes rendered on every generated invoice
        #[arg(long)]
        notes: Option<String>,
        /// Payment terms rendered on every generated invoice
        #[arg(long)]
        terms: Option<String>,
        /// Send each generated invoice instead of leaving it a draft
        #[arg(long)]
        autosend: bool,
    },
    /// List schedules.
    List {
        /// Include paused and ended schedules
        #[arg(long)]
        all: bool,
    },
    /// Show one schedule, its items, and what it has generated.
    Show {
        /// Schedule ID (shown in `nigel invoice schedule list`)
        id: i64,
    },
    /// Edit a schedule. Changes apply to future invoices, never past ones.
    Edit {
        /// Schedule ID
        id: i64,
        /// New anchor day
        #[arg(long = "anchor-day")]
        anchor_day: Option<u32>,
        /// New net days
        #[arg(long = "net-days")]
        net_days: Option<i64>,
        /// Drop the net days, so generated invoices carry no due date
        #[arg(long = "clear-net-days", conflicts_with = "net_days")]
        clear_net_days: bool,
        /// New currency code
        #[arg(long)]
        currency: Option<String>,
        /// Replace the notes
        #[arg(long)]
        notes: Option<String>,
        /// Replace the payment terms
        #[arg(long)]
        terms: Option<String>,
        /// Line item as "desc:qty:unit" (repeatable); replaces every existing line
        #[arg(long = "item")]
        items: Vec<String>,
        /// Send each generated invoice from now on
        #[arg(long)]
        autosend: bool,
        /// Leave each generated invoice a draft from now on
        #[arg(long = "no-autosend", conflicts_with = "autosend")]
        no_autosend: bool,
    },
    /// Stop generating from a schedule without ending it.
    Pause {
        /// Schedule ID
        id: i64,
    },
    /// Start generating from a paused schedule again.
    Resume {
        /// Schedule ID
        id: i64,
    },
    /// End a schedule for good. Its history is kept.
    End {
        /// Schedule ID
        id: i64,
    },
    /// Generate every invoice currently due. Built for cron and launchd — it
    /// never prompts.
    Run,
}
```

  Register the new module in `crates/nigel/src/cli/mod.rs`'s module list
  (beside `pub mod invoice_manager;`):

```rust
pub mod invoice_schedule;
```

- [ ] **Step 4: Write the command module.** Create
  `crates/nigel/src/cli/invoice_schedule.rs`:

```rust
use comfy_table::{Cell, Table};

use nigel_core::db::get_connection;
use nigel_core::error::{NigelError, Result};
use nigel_core::fmt::money;
use nigel_core::invoicing::clients::get_client;
use nigel_core::invoicing::invoices::{get_invoice_by_number, invoice_shape};
use nigel_core::invoicing::schedules::{
    add_schedule, draft_due_schedules, end_schedule, get_schedule, list_schedules, pause_schedule,
    resume_schedule, run_due_schedules, schedule_items, schedule_runs, update_schedule, Cadence,
    NewSchedule, Schedule, ScheduleRunReport, ScheduleScope, ScheduleUpdate, Senders,
};
use nigel_core::invoicing::render_html::load_template;
use nigel_core::invoicing::wiring::{build_clients, company_profile, contact_email_for_preview};
use nigel_core::settings::{get_data_dir, invoicing_config, invoicing_status};

use crate::cli::invoice::parse_items;

/// The word a listing prints for a schedule's state.
fn state(schedule: &Schedule) -> &'static str {
    if schedule.ended_at.is_some() {
        "ended"
    } else if schedule.paused {
        "paused"
    } else if schedule.autosend {
        "autosend"
    } else {
        "draft"
    }
}

#[allow(clippy::too_many_arguments)]
pub fn add(
    client_id: i64,
    cadence: &str,
    start: &str,
    anchor_day: Option<u32>,
    net_days: Option<i64>,
    currency: &str,
    items: &[String],
    from: Option<i64>,
    notes: Option<String>,
    terms: Option<String>,
    autosend: bool,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;

    // Either explicit items or an invoice's shape, never both — clap refuses the
    // combination, and this is the same duplication core reading the same shape.
    let (items, currency, notes, terms, net_days) = match from {
        Some(number) => {
            let invoice = get_invoice_by_number(&conn, number)?;
            let shape = invoice_shape(&conn, invoice.id)?;
            (
                shape.items,
                shape.currency,
                notes.or(shape.notes),
                terms.or(shape.terms),
                net_days.or(shape.net_days),
            )
        }
        None => (
            parse_items(items)?,
            currency.to_string(),
            notes,
            terms,
            net_days,
        ),
    };

    let anchor_day = match anchor_day {
        Some(day) => day,
        None => day_of(start)?,
    };

    let id = add_schedule(
        &conn,
        &NewSchedule {
            client_id,
            cadence: Cadence::parse(cadence)?,
            anchor_day,
            start_period: start.to_string(),
            net_days,
            currency,
            notes,
            terms,
            autosend,
            items,
        },
    )?;
    let client = get_client(&conn, client_id)?;
    println!(
        "Created schedule {id} for {}: {cadence} from {start}, {}",
        client.name,
        if autosend { "autosend" } else { "draft" }
    );
    Ok(())
}

/// The day-of-month a `YYYY-MM-DD` names, for the default anchor.
fn day_of(date: &str) -> Result<u32> {
    let day = nigel_core::invoicing::invoices::parse_date(date, "start")?;
    Ok(chrono::Datelike::day(&day))
}

pub fn list(all: bool) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let scope = if all { ScheduleScope::All } else { ScheduleScope::Active };
    let mut table = Table::new();
    table.set_header(vec!["ID", "Client", "Cadence", "Next", "Amount", "State"]);
    for schedule in list_schedules(&conn, scope)? {
        let total: f64 = schedule_items(&conn, schedule.id)?
            .iter()
            .map(|i| i.quantity * i.unit_amount)
            .sum();
        let client = get_client(&conn, schedule.client_id)
            .map(|c| c.name)
            .unwrap_or_else(|_| "\u{2014}".to_string());
        table.add_row(vec![
            Cell::new(schedule.id),
            Cell::new(client),
            Cell::new(&schedule.cadence),
            Cell::new(&schedule.next_period),
            Cell::new(money(total)),
            Cell::new(state(&schedule)),
        ]);
    }
    println!("Invoice schedules\n{table}");
    Ok(())
}

pub fn show(id: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let schedule = get_schedule(&conn, id)?;
    let client = get_client(&conn, schedule.client_id)?;

    println!("Schedule {id}  [{}]  {}", state(&schedule), schedule.cadence);
    println!("Client:    {}", client.name);
    println!("Next:      {}", schedule.next_period);
    println!("Anchor:    day {}", schedule.anchor_day);
    println!(
        "Net days:  {}",
        schedule
            .net_days
            .map(|d| d.to_string())
            .unwrap_or_else(|| "-".to_string())
    );
    println!("Currency:  {}", schedule.currency);
    if let Some(ended) = &schedule.ended_at {
        println!("Ended:     {ended}");
    }

    let items = schedule_items(&conn, id)?;
    let mut table = Table::new();
    table.set_header(vec!["Description", "Qty", "Unit", "Amount"]);
    for item in &items {
        table.add_row(vec![
            Cell::new(&item.description),
            Cell::new(format!("{:.2}", item.quantity)),
            Cell::new(money(item.unit_amount)),
            Cell::new(money(item.quantity * item.unit_amount)),
        ]);
    }
    println!("{table}");

    let runs = schedule_runs(&conn, id)?;
    if runs.is_empty() {
        println!("Nothing generated yet.");
        return Ok(());
    }
    let mut history = Table::new();
    history.set_header(vec!["Period", "Invoice", "Generated"]);
    for run in runs {
        history.add_row(vec![
            Cell::new(run.period),
            Cell::new(format!("#{}", run.number)),
            Cell::new(run.generated_at),
        ]);
    }
    println!("Generated\n{history}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn edit(
    id: i64,
    anchor_day: Option<u32>,
    net_days: Option<i64>,
    clear_net_days: bool,
    currency: Option<String>,
    notes: Option<String>,
    terms: Option<String>,
    items: &[String],
    autosend: bool,
    no_autosend: bool,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let net_days = if clear_net_days {
        Some(None)
    } else {
        net_days.map(Some)
    };
    let autosend = match (autosend, no_autosend) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    };
    let items = if items.is_empty() {
        None
    } else {
        Some(parse_items(items)?)
    };
    update_schedule(
        &conn,
        id,
        &ScheduleUpdate {
            anchor_day,
            net_days,
            currency,
            notes: notes.map(Some),
            terms: terms.map(Some),
            autosend,
            items,
        },
    )?;
    println!("Updated schedule {id}. Future invoices use the new figures.");
    Ok(())
}

pub fn pause(id: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    pause_schedule(&conn, id)?;
    println!("Paused schedule {id}. Nothing is generated until it is resumed.");
    Ok(())
}

pub fn resume(id: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    resume_schedule(&conn, id)?;
    let schedule = get_schedule(&conn, id)?;
    println!(
        "Resumed schedule {id}. The next run generates from {}.",
        schedule.next_period
    );
    Ok(())
}

pub fn end(id: i64, today: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    end_schedule(&conn, id, today)?;
    println!("Ended schedule {id}. Its invoices and its history are kept.");
    Ok(())
}

/// Generate everything due. The command a cron job or a launchd agent runs.
///
/// Sending is per schedule and only reachable when the full nine-key set is
/// configured; an installation that cannot send still generates every draft and
/// says so. The exit status is non-zero when anything a schedule asked for did
/// not happen, so a scheduled job surfaces it.
pub fn run(today: &str) -> Result<()> {
    let data_dir = get_data_dir();
    let conn = get_connection(&data_dir.join("nigel.db"))?;
    let cfg = invoicing_config();

    let report = if invoicing_status(&cfg).send_configured {
        let template = load_template(&data_dir)?;
        let profile = company_profile(&conn);
        let contact_email = contact_email_for_preview(&cfg).0;
        let branding = profile.branding(&template, &contact_email);
        let clients = build_clients(cfg, profile.name())?;
        for warning in clients.warnings() {
            eprintln!("notice: {warning}");
        }
        run_due_schedules(
            &conn,
            today,
            Some(&Senders {
                branding: &branding,
                gateway: clients.stripe(),
                publisher: clients.r2(),
                mailer: clients.mail(),
            }),
        )?
    } else {
        draft_due_schedules(&conn, today)?
    };

    print_report(&report);
    if report.has_failures() {
        return Err(NigelError::Other(
            "Some invoices were not sent. See the lines above.".to_string(),
        ));
    }
    Ok(())
}

fn print_report(report: &ScheduleRunReport) {
    println!("Generated {} invoice(s).", report.generated.len());
    for generated in &report.generated {
        let state = match (&generated.not_sent, generated.sent) {
            (Some(reason), _) => format!("draft — not sent: {reason}"),
            (None, true) => "sent".to_string(),
            (None, false) => "draft".to_string(),
        };
        println!(
            "  #{}  {}  {}  {}  {state}",
            generated.number,
            generated.client_name,
            money(generated.total),
            generated.period
        );
    }
    for failure in &report.failures {
        eprintln!(
            "notice: schedule {} stopped at {}: {}",
            failure.schedule_id, failure.period, failure.message
        );
    }
}
```

  `parse_items` is currently private in `crates/nigel/src/cli/invoice.rs`;
  change it to `pub(crate) fn parse_items` so both command modules share the one
  `desc:qty:unit` parser and its refusals.

- [ ] **Step 5: Add the non-prompting unlock.** In
  `crates/nigel/src/cli/password.rs`, beside `prompt_password_if_needed`:

```rust
/// Unlock from `NIGEL_DB_PASSWORD` only — the path for a command that has no
/// terminal to answer a prompt.
///
/// `prompt_password_if_needed`'s environment branch without its fallback: a
/// scheduled job that reached a prompt would hang until something killed it,
/// which is worse than failing with a sentence naming what to set.
pub fn unlock_without_prompting(db_path: &Path) -> Result<()> {
    if !is_encrypted(db_path)? {
        return Ok(());
    }
    match env_password_if_set(db_path)? {
        Some(pw) => {
            set_db_password(Some(pw));
            Ok(())
        }
        None => Err(nigel_core::error::NigelError::Other(format!(
            "{} is encrypted and this command never prompts. \
             Set NIGEL_DB_PASSWORD — see \"Automated backups\" in the README.",
            db_path.display()
        ))),
    }
}
```

- [ ] **Step 6: Wire the dispatch.** In `crates/nigel/src/main.rs`, above
  `fn dispatch`:

```rust
/// Commands built for cron and launchd, which must never reach a prompt.
fn is_unattended(command: &Commands) -> bool {
    matches!(
        command,
        Commands::Invoice {
            command: InvoiceCommands::Schedule {
                command: InvoiceScheduleCommands::Run
            }
        }
    )
}
```

  and add `InvoiceScheduleCommands` to the `use nigel::cli::{…}` list at the top
  of the file. Then replace the unlock block in `dispatch`:

```rust
    if needs_password && db_path.exists() {
        if is_unattended(&command) {
            nigel::cli::password::unlock_without_prompting(&db_path)?;
        } else {
            nigel::cli::password::prompt_password_if_needed(&db_path)?;
        }
    }
```

  and add the arm inside `Commands::Invoice { command } => match command {`,
  after `InvoiceCommands::Template`:

```rust
            InvoiceCommands::Schedule { command } => match command {
                InvoiceScheduleCommands::Add {
                    client,
                    cadence,
                    start,
                    anchor_day,
                    net_days,
                    currency,
                    items,
                    from,
                    notes,
                    terms,
                    autosend,
                } => cli::invoice_schedule::add(
                    client, &cadence, &start, anchor_day, net_days, &currency, &items, from,
                    notes, terms, autosend,
                ),
                InvoiceScheduleCommands::List { all } => cli::invoice_schedule::list(all),
                InvoiceScheduleCommands::Show { id } => cli::invoice_schedule::show(id),
                InvoiceScheduleCommands::Edit {
                    id,
                    anchor_day,
                    net_days,
                    clear_net_days,
                    currency,
                    notes,
                    terms,
                    items,
                    autosend,
                    no_autosend,
                } => cli::invoice_schedule::edit(
                    id, anchor_day, net_days, clear_net_days, currency, notes, terms, &items,
                    autosend, no_autosend,
                ),
                InvoiceScheduleCommands::Pause { id } => cli::invoice_schedule::pause(id),
                InvoiceScheduleCommands::Resume { id } => cli::invoice_schedule::resume(id),
                InvoiceScheduleCommands::End { id } => {
                    cli::invoice_schedule::end(id, &cli::today())
                }
                InvoiceScheduleCommands::Run => cli::invoice_schedule::run(&cli::today()),
            },
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p nigel --test cli_dispatch invoice_schedule -- --test-threads=1`
Expected: PASS, 6 tests. The encrypted-database test must **fail fast** — if it
hits `TEST_TIMEOUT`, the unattended branch is not being taken.

- [ ] **Step 8: Full check and commit**

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
./scripts/check-no-real-data.sh --staged; echo "exit=$?"
git add crates/nigel/src crates/nigel/tests/cli_dispatch.rs
git commit -m "Add nigel invoice schedule, unattended by design"
```
Expected: clean, `exit=0`, hook passes.

---

### Task 8: Documentation

**Files:**
- Modify: `docs/invoicing.md`
- Modify: `docs/commands.md`
- Modify: `docs/api.md`
- Modify: `README.md`

**Interfaces:**
- Consumes: every command and route Tasks 1–7 shipped. Nothing produces code
  here; the deliverable is documentation that matches what `nigel --help` and
  the router actually say.

- [ ] **Step 1: Verify the surfaces before writing about them**

```bash
cargo run -q -- invoice --help
cargo run -q -- invoice schedule --help
cargo run -q -- invoice schedule add --help
```
Expected: every flag named below appears in the output. **Write down what these
print, not what this plan predicted** — a doc that disagrees with `--help` is a
defect.

- [ ] **Step 2: Add the duplicate section to `docs/invoicing.md`.** Insert
  after the "Creating an invoice" section and before "Previewing":

````markdown
## Duplicating an invoice

```bash
nigel invoice duplicate 1248                      # issued today
nigel invoice duplicate 1248 --issue 2026-09-01   # issued on a given day
```

A duplicate is a fresh draft with a new number and a new token. It copies the
client, the currency, the notes, the terms and every line item. It copies
nothing about what happened to the source: no published date, no void date, no
Stripe payment link.

**The term travels, the dates do not.** A source with a due date fourteen days
after its issue date duplicates as a draft due fourteen days after *its* issue
date — a Net-14 invoice stays Net-14 in September. A source with no due date
yields a draft with none, which never goes overdue.

Any invoice duplicates, whatever state it is in — draft, sent, paid or void.
Duplication reads a shape, not a status, and last quarter's paid invoice is the
most useful thing to copy. The one refusal is an archived client: the new draft
goes through the same guard `nigel invoice new` does, and says so.

| Source | Duplicate |
| --- | --- |
| any state, active client | a fresh draft under the next number |
| archived client | refused — unarchive the client first |

The TUI's invoice detail offers `c=duplicate` and lands on the new draft. In the
browser, **Duplicate** sits in the invoice actions and navigates to the copy.
````

- [ ] **Step 3: Add the schedules section to `docs/invoicing.md`.** Insert
  after "Recording payments" and before "Trying it end to end in test mode":

````markdown
## Recurring schedules

Retainers and hosting are billed on a cycle. A schedule stores the shape once,
and one command generates whatever is due.

```bash
nigel invoice schedule add --client 1 --cadence monthly --start 2026-01-01 \
  --net-days 30 --item "Hosting & maintenance:1:450"

nigel invoice schedule add --client 1 --cadence quarterly --start 2026-01-01 \
  --from 1248          # seed the items, currency, notes and terms from an invoice

nigel invoice schedule list          # active schedules
nigel invoice schedule list --all    # including paused and ended
nigel invoice schedule show 1        # items, and every invoice it has generated
nigel invoice schedule run           # generate everything currently due
```

`--cadence` is `monthly`, `quarterly` or `yearly`. `--start` is the first
period's issue date, and `--anchor-day` is the day of the month the cycle is
anchored on — it defaults to the start date's day.

### What a run does

`nigel invoice schedule run` walks each active schedule's periods from where it
left off through today. **Each missed cycle generates its own invoice, dated
that period's issue date rather than the day the run happened.** A machine that
was asleep through February and March produces a February invoice dated
February and a March invoice dated March, in that order — and the February one
is honestly already overdue, which is the true statement about a bill that
should have gone out six weeks ago.

Each generation is one transaction: the invoice, the row recording which
schedule and which period produced it, and the schedule's advance. Numbering is
sequential through `next_invoice_number` even when several schedules generate at
once, because the counter is read and written inside that same transaction.

**A run is idempotent.** The recorded schedule-and-period pair is unique, so a
second run for a period that has already been generated writes nothing. This is
provenance, not a guess from dates: `nigel invoice schedule show` prints the
pairing, and a rerun after a cron misfire costs nothing.

### Drafting and sending

**A run drafts by default.** Sending is per schedule and opt-in:

```bash
nigel invoice schedule add … --autosend
nigel invoice schedule edit 1 --autosend      # or --no-autosend
```

Unattended sending means a wrong figure can reach a client with nobody
watching, so it is a decision made once per schedule rather than a default.

An autosend schedule sends through the same path `nigel invoice send` uses, and
only when all nine invoicing keys are configured. When it cannot send — the
configuration is incomplete, or the client has no email address — **the invoice
is still generated as a draft and the run says which one and why**:

```
Generated 2 invoice(s).
  #1253  Cedar Systems  $450.00  2026-08-01  sent
  #1254  Harbor & Vale  $900.00  2026-08-01  draft — not sent: client 'Harbor & Vale' has no email
```

The command exits non-zero when anything a schedule asked for did not happen, so
a scheduled job surfaces it instead of swallowing it. Nothing is ever
half-sent: a failed send leaves the invoice the draft it already was.

### Month end and short months

A monthly schedule anchored on the 31st bills the 28th in February, the 29th in
a leap February, the 30th in April — and the 31st again in May. The **anchor**
is what advances, never the clamped day it produced last time, so a short month
does not permanently move the billing date.

### Editing, pausing and ending

```bash
nigel invoice schedule edit 1 --item "Hosting & maintenance:1:495"
nigel invoice schedule edit 1 --net-days 14
nigel invoice schedule pause 1
nigel invoice schedule resume 1
nigel invoice schedule end 1
```

Line items are held at schedule level and re-read on every run, so **editing a
schedule changes future invoices and never past ones**. Editing never moves the
cycle: `next_period` stays where the last run left it.

Pausing stops generation without ending anything. Ending writes a date and stops
it for good. Neither deletes a row — the schedule, its items and every invoice
it produced stay readable through `nigel invoice schedule list --all` and
`nigel invoice schedule show`.

### Running it from cron or launchd

`nigel invoice schedule run` is built to run with nobody at the keyboard, so
**it never prompts**. On an encrypted database it takes the password from
`NIGEL_DB_PASSWORD` exactly as `nigel backup` does, and with no password
available it fails immediately with a sentence naming the variable rather than
waiting on a prompt no scheduled job can answer.

```bash
#!/bin/sh
# ~/bin/nigel-invoice-run.sh
set -eu
NIGEL_DB_PASSWORD="$(security find-generic-password -s nigel-db -w)" \
  nigel invoice schedule run
```

Put it in a wrapper script rather than directly in a launchd plist or a crontab
line, and read the password from a secret store rather than writing it into
either. Then schedule the wrapper:

```cron
# 07:00 on the first of every month
0 7 1 * * /Users/you/bin/nigel-invoice-run.sh >> /Users/you/Library/Logs/nigel-invoices.log 2>&1
```

The log and the exit status are the whole monitoring story: a run that could not
send says so on stderr and exits non-zero.
````

- [ ] **Step 4: Add the command lines to `docs/commands.md`.** In the invoice
  block, after the `nigel invoice edit 1248 --clear-due` line:

```
nigel invoice duplicate 1248                      # Copy into a fresh draft, issued today
nigel invoice duplicate 1248 --issue 2026-09-01   # Copy with a given issue date
```

  and after the `nigel invoice template path` line:

```
nigel invoice schedule add --client 1 --cadence monthly --start 2026-01-01 --item "Hosting:1:450"
nigel invoice schedule add --client 1 --cadence quarterly --start 2026-01-01 --from 1248  # Seed from an invoice
nigel invoice schedule list                       # Active schedules (--all includes paused and ended)
nigel invoice schedule show 1                     # Items, and every invoice it has generated
nigel invoice schedule edit 1 --item "Hosting:1:495"   # Applies to future invoices only
nigel invoice schedule pause 1                    # Stop generating without ending it
nigel invoice schedule resume 1                   # Start generating again
nigel invoice schedule end 1                      # End it for good; history is kept
nigel invoice schedule run                        # Generate everything due (cron/launchd; never prompts)
```

- [ ] **Step 5: Document the route in `docs/api.md`.** Add the row to the
  write table in "Changing data", after the `/api/invoices/:number` `DELETE`
  row:

```
| `/api/invoices/:number/duplicate` | `POST` | — | `InvoiceDetail` (`201`) |
```

  and a subsection after "#### Deleting a draft":

````markdown
#### Duplicating an invoice

`POST /api/invoices/:number/duplicate` takes no body and answers `201` with the
whole new `InvoiceDetail`, the way `POST /api/invoices` does — the browser
navigates straight to the copy.

The new draft copies the client, currency, notes, terms and line items, and
carries a new number and a new token. Nothing about the source's history comes
across: `publishedAt`, `voidedAt`, `stripePaymentLinkId`,
`stripePaymentLinkUrl` and `publicUrl` are all `null`, and `status` is `draft`.

The issue date is the server's today. When the source carries a due date the
copy preserves the source's issue-to-due offset in days; a source without one
yields a copy without one.

Any state duplicates — draft, sent, paid, void. The refusals are the ones
`POST /api/invoices` already gives: `404 invoice_not_found` for a number that is
not there, and `409 client_archived` when the source's client has been archived.
````

- [ ] **Step 6: Add the README section.** In `README.md`, after the "Automated
  backups" section and before "Configuration":

````markdown
## Scheduled invoices

`nigel invoice schedule run` generates every recurring invoice currently due and
is built for launchd or cron: it drafts by default, a rerun for a period it has
already billed does nothing, and **it never prompts**. On an encrypted database
it reads `NIGEL_DB_PASSWORD` the same way `nigel backup` does:

```bash
NIGEL_DB_PASSWORD="$(security find-generic-password -s nigel-db -w)" \
  nigel invoice schedule run
```

Put this in a wrapper script rather than directly in a launchd plist, and read
the password from a secret store rather than writing it into a script or a
plist. The command exits non-zero when a schedule that asked to send could not,
so the job's log and exit status are enough to notice. See
[docs/invoicing.md](docs/invoicing.md) for schedules, catch-up behaviour and the
autosend opt-in.
````

  Also add the two features to the README's Features list if one enumerates
  invoicing capabilities — check `grep -n "Invoicing" README.md` and match the
  surrounding style rather than inventing a new one.

- [ ] **Step 7: Check the docs against the code**

```bash
cargo run -q -- invoice schedule --help
grep -n "invoice schedule" docs/commands.md
./scripts/check-no-real-data.sh; echo "exit=$?"
```
Expected: every documented flag exists, and `exit=0` — the docs above use only
the fictional cast (Cedar Systems, Harbor & Vale) and invented amounts.

- [ ] **Step 8: Commit**

```bash
./scripts/check-no-real-data.sh --staged; echo "exit=$?"
git add docs/invoicing.md docs/commands.md docs/api.md README.md
git commit -m "Document invoice duplication and recurring schedules"
```
Expected: `exit=0`, hook passes.

---

## Acceptance criteria coverage

**TASK-7**

| AC | Covered by |
| --- | --- |
| #1 fresh draft copying client/currency/notes/terms/items, new number and token, no publish/void/Stripe state | Task 1 `duplicating_copies_the_shape_and_regenerates_the_identity`; Task 2 CLI |
| #2 issue-to-due offset preserved; none yields none | Task 1 `duplicating_preserves_the_issue_to_due_offset_in_days` |
| #3 any source state duplicates; archived client refuses as `create_invoice` does | Task 1 `any_source_state_duplicates_because_duplication_reads_a_shape`, `duplicating_for_an_archived_client_refuses_the_way_create_invoice_does` |
| #4 TUI detail action landing on the new draft; web Duplicate button behind `POST /invoices/{number}/duplicate` | Task 3 (six manager tests); Task 4 (two route tests, three screen tests) |
| #5 test coverage | Tasks 1–4, every step's test block |
| #6 documentation | Task 8 Steps 2, 4, 5 |
| #7 linting | Every task's `cargo fmt --check` + `cargo clippy -D warnings` step; Task 4 adds `npm run lint` |

**TASK-81**

| AC | Covered by |
| --- | --- |
| #1 a schedule against a client with line items and a cycle | Task 5 `a_schedule_stores_its_shape_and_starts_at_its_first_period`; Task 7 `invoice_schedule_add_list_and_show_read_back_what_was_stored` |
| #2 one command generates everything due, suitable for launchd or cron | Task 7 `invoice_schedule_run_generates_drafts_and_a_second_run_generates_nothing`; Task 8 wrapper-script docs |
| #3 a second run generates nothing; the invoice records its schedule and period | Task 6 `running_twice_for_the_same_period_generates_nothing_the_second_time`, `a_run_row_already_there_advances_the_cycle_without_billing_again`; the `UNIQUE(schedule_id, period)` index test in Task 5 |
| #4 draft-or-send explicit per schedule, drafting the default | Task 6 `an_autosend_schedule_sends_and_a_drafting_one_beside_it_does_not`; `--autosend`/`--no-autosend` in Task 7 |
| #5 catch-up defined and documented | Task 6 `a_run_generates_every_missed_cycle_dated_by_its_own_period`; Task 8 "What a run does" |
| #6 month end and short months | Task 5 `a_monthly_anchor_clamps_in_short_months_and_returns_to_the_anchor`, `clamp_day_answers_the_last_valid_day_of_the_month`; Task 6 `a_monthly_schedule_anchored_at_month_end_bills_the_short_months` |
| #7 numbering sequential when several generate at once | Task 6 `several_schedules_at_once_keep_the_numbering_sequential` and the `[1248, 1249, 1250, 1251]` assertion in the catch-up test |
| #8 pause, edit and end without deleting history | Task 5 `pausing_resuming_and_ending_leave_the_row_and_its_history_alone`, `editing_replaces_only_what_is_given`; Task 7 `invoice_schedule_pause_resume_and_end_keep_the_schedule_and_its_history` |
| #9 an unsendable client is reported, not skipped or half-sent | Task 6 `an_unsendable_client_still_gets_its_draft_and_the_run_says_why`, `an_autosend_schedule_with_nothing_to_send_through_still_drafts_and_says_why`, `a_schedule_whose_client_was_archived_is_reported_and_does_not_stop_the_others` |
| #10 unattended via `NIGEL_DB_PASSWORD`, never prompts | Task 7 `invoice_schedule_run_never_prompts_on_an_encrypted_database` and `password::unlock_without_prompting` |
