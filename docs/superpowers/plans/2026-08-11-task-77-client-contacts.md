# Client contacts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A client holds several email addresses, exactly one of them the
billing recipient, and a sent invoice reaches the rest as `Cc` — per
`docs/superpowers/specs/2026-08-11-task-77-client-contacts-design.md`.

**Architecture:** A `client_contacts` child table (name, email, title,
`is_billing`, `position`) with a partial unique index making "at most one
billing contact" a database fact, and a normalize step at every write making
"at least one, whenever there is any" a data-layer fact. `clients.email` is
backfilled into it and dropped; `models::Client` is **unchanged**, its `email`
field becoming a correlated-subquery projection of the billing contact, so every
existing read path — `format_client_list`, `{{CLIENT_EMAIL}}`, `require_email`,
the `Client` wire shape, `wc-client-form` — keeps working. Writes are a
whole-list replacement (`set_contacts`), the shape `update_invoice` already uses
for `items`. `send.rs` resolves `Recipients { to, cc }` at its existing
`Precheck` step and `Mailer::send_invoice` gains `cc`.

**Tech Stack:** Rust, rusqlite (SQLCipher-bundled SQLite), clap (derive),
ratatui/crossterm, reqwest multipart, axum (`serve` feature), Lit + Web Awesome,
vitest + axe.

## Global Constraints

- After every task: `cargo test -- --test-threads=1`,
  `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` clean.
- **Every task must also pass without the `pdf` feature** —
  `cargo test --no-default-features --features gusto -- --test-threads=1` — and
  with no default features: `cargo test --no-default-features -- --test-threads=1`.
  `send.rs`'s test module is `cfg(all(test, feature = "pdf"))`, so the cc
  assertions there only run in one build; the pure `message_fields` and
  `validate_contacts` tests must run in all three.
- **Depends on PR-3a (TASK-80).** `mailgun::format_address` and
  `validate_header_value` must already exist, and `message_fields` must already
  take an `EmailEnvelope`. Do not start this until 3a is merged.
- **`models::Client` does not change**, and neither does
  `src/invoicing/render_html.rs` or `src/invoicing/templates/invoice.html` — the
  page names the billing contact through the placeholder it already has, and
  Stream 2's TASK-78 owns those files.
- `src/invoicing/` reads no settings and reaches into no `src/cli/`.
- **Migration numbering:** the next free number on your branch's base, renumbered
  to last on rebase if another stream's migration landed first. A gap is unsafe —
  `apply_migrations` runs `version > current`, so a later-merged lower number is
  skipped forever on any database that upgraded in between.
- No test may reach the network; the cc path is asserted through the existing
  fake `Mailer`s.

---

### Task 1: The table, the backfill, and the dropped column

**Files:** modify `src/migrations.rs`.

- [ ] **Step 0: Probe the SQLite version before writing anything.** `DROP
  COLUMN` needs 3.35+:

```bash
cargo test --lib migrations::tests -- --nocapture   # or a scratch test printing it
```

```rust
#[test]
fn the_bundled_sqlite_supports_drop_column() {
    let (_dir, conn) = test_db();
    let v: String = conn.query_row("SELECT sqlite_version()", [], |r| r.get(0)).unwrap();
    let parts: Vec<u32> = v.split('.').filter_map(|p| p.parse().ok()).collect();
    assert!(parts[0] > 3 || (parts[0] == 3 && parts[1] >= 35), "sqlite {v} cannot DROP COLUMN");
}
```

  Keep this test — it is what documents the requirement. **If it fails**, the
  fallback is `ALTER TABLE clients RENAME COLUMN email TO legacy_email` (3.25+),
  the code stops reading the column either way, and the spec's "no second source
  of truth" property still holds; note the deviation in the PR description.

- [ ] **Step 1: Write failing tests** in `migrations.rs`'s `mod tests`:

```rust
#[test]
fn a_single_email_becomes_one_billing_contact() {
    // build a DB at the pre-contacts version, insert two clients — one with
    // "  ap@acme.test  ", one with NULL — then run_migrations and assert:
    //   the first has exactly one contact, email trimmed, is_billing = 1, position 0
    //   the second has none
    //   AC #5: nobody re-entered anything
}

#[test]
fn a_blank_email_does_not_become_a_contact() {
    // email = "   " → zero contacts, not one carrying whitespace
}

#[test]
fn the_clients_table_no_longer_carries_an_email_column() {
    let has: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('clients') WHERE name = 'email'",
        [], |r| r.get(0)).unwrap();
    assert!(!has, "a second source of truth for the billing address");
}

#[test]
fn two_billing_contacts_for_one_client_are_refused_by_the_index() {
    // raw INSERT of a second is_billing = 1 row → Err
}

#[test]
fn the_same_address_twice_for_one_client_is_refused_case_insensitively() {
    // "AP@acme.test" after "ap@acme.test" → Err
}

#[test]
fn deleting_a_client_takes_its_contacts_with_it() {
    // FK cascade — db.rs opens every connection with PRAGMA foreign_keys=ON
}

#[test]
fn the_contacts_migration_is_replayable() { /* run_migrations twice */ }
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib migrations 2>&1 | tail -20`

- [ ] **Step 3: Implement** the migration exactly as the spec's SQL, in one
  `up`: `CREATE TABLE IF NOT EXISTS client_contacts`, the two
  `CREATE UNIQUE INDEX IF NOT EXISTS`, the backfill `INSERT … SELECT` guarded by
  the same `pragma_table_info` probe v5 uses (so a replay after the column is
  gone does not fail on an unknown column), then the `DROP COLUMN` inside that
  same probe.

- [ ] **Step 4: Verify.** All three feature builds. The rest of the tree will
  not compile yet — `clients.rs` still selects `clients.email` — which is Task 2.

---

### Task 2: The data layer

**Files:** modify `src/invoicing/clients.rs`.

**Interface produced** (consumed by every later task):

```rust
pub struct ClientContact { pub id: i64, pub client_id: i64, pub name: Option<String>,
                           pub email: String, pub title: Option<String>,
                           pub is_billing: bool, pub position: i64 }
pub struct NewContact { pub email: String, pub name: Option<String>,
                        pub title: Option<String>, pub is_billing: bool }

pub fn list_contacts(conn: &Connection, client_id: i64) -> Result<Vec<ClientContact>>;
pub fn set_contacts(conn: &Connection, client_id: i64, contacts: &[NewContact]) -> Result<()>;
pub fn validate_contacts(contacts: &[NewContact]) -> Result<()>;
// ClientSummary gains `pub contacts: Vec<ClientContact>`
```

- [ ] **Step 1: Write failing tests** in `clients.rs`'s `mod tests`. The
  existing tests are the specification of what must not change — run them
  untouched wherever possible:

```rust
#[test]
fn add_client_still_stores_and_answers_one_email() {
    // the existing `add_and_get_client` assertions, unchanged: this is the
    // whole point of keeping `Client.email` as a projection
}

#[test]
fn the_projected_email_is_the_billing_contacts_address() {
    let id = add_client(&conn, "Acme Co", Some("ap@acme.test"), None, None).unwrap();
    set_contacts(&conn, id, &[
        NewContact { email: "dana@acme.test".into(), name: Some("Dana".into()), is_billing: true, ..Default::default() },
        NewContact { email: "ap@acme.test".into(), ..Default::default() },
    ]).unwrap();
    assert_eq!(get_client(&conn, id).unwrap().email.as_deref(), Some("dana@acme.test"));
}

#[test]
fn setting_the_email_upserts_the_billing_contact_and_leaves_the_cc_rows_alone() {
    // two contacts, then ClientUpdate { email: Some(Some("new@acme.test")) }
    // → the billing row's address changed, the cc row is untouched
}

#[test]
fn clearing_the_email_promotes_the_next_contact() {
    // ClientUpdate { email: Some(None) } with a cc row present
    // → the cc row is now billing and Client.email is its address; a client
    //   with contacts but no billing recipient is not representable
}

#[test]
fn clearing_the_email_of_a_single_contact_client_leaves_no_contacts() {
    assert_eq!(get_client(&conn, id).unwrap().email, None);
    assert!(list_contacts(&conn, id).unwrap().is_empty());
}

#[test]
fn a_list_with_no_billing_flag_makes_the_first_row_billing() { /* normalize */ }

#[test]
fn a_list_with_two_billing_flags_is_refused_before_anything_is_deleted() {
    let err = set_contacts(&conn, id, &[billing("a@x.test"), billing("b@x.test")]).unwrap_err();
    assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
    assert_eq!(list_contacts(&conn, id).unwrap().len(), 1, "validation runs before the delete");
}

#[test]
fn a_blank_or_duplicate_address_is_refused() { /* "", "   ", and A@x/a@x */ }

#[test]
fn a_contact_carrying_a_line_break_is_refused() {
    // these strings become mail headers — mailgun::validate_header_value
}

#[test]
fn set_contacts_replaces_the_whole_list_in_one_transaction() {
    // three rows → two rows; positions are 0,1 in the order supplied
}

#[test]
fn client_summary_carries_the_contacts() { /* … */ }

#[test]
fn list_clients_answers_every_billing_email_in_one_query() {
    // 3 clients with contacts; assert the emails and that the function issues
    // one statement (the correlated subquery, not an N+1)
}
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib invoicing::clients 2>&1 | tail -20`

- [ ] **Step 3: Implement the reads.** Replace `email` in `get_client`'s and
  `list_clients`' `SELECT` with

```sql
(SELECT cc.email FROM client_contacts cc
  WHERE cc.client_id = clients.id AND cc.is_billing = 1) AS email
```

  keeping the row mapping's column indices in step. Add `list_contacts`
  (`ORDER BY is_billing DESC, position, id`) and `ClientSummary.contacts`.

- [ ] **Step 4: Implement the writes.**

```rust
/// The one place a billing address is written, so `add_client`,
/// `update_client` and the InvoiceShelf importer cannot disagree about what
/// setting an email means.
fn set_billing_email(conn: &Connection, client_id: i64, email: Option<&str>) -> Result<()>;
```

  - `Some(addr)`: update the existing billing row's address, or insert one at
    the end of the list flagged billing if there is none.
  - `None`: delete the billing row, then flag the lowest-`position` survivor if
    any remain.

  `add_client` inserts the client row and then calls it. `update_client`'s
  `email` arm stops being a column in the `UPDATE` and becomes a call to it —
  note that this means `update_client` must now run its column update and the
  contact write inside one `unchecked_transaction`, as `update_invoice` does.

  `set_contacts` runs `validate_contacts` first, then, inside a transaction,
  `DELETE FROM client_contacts WHERE client_id = ?` and re-inserts with
  `position` from the slice index. Ids are not preserved — the same property
  `items` has on `update_invoice`, and nothing references a contact id.

  `validate_contacts` refuses: an empty or whitespace-only email; two addresses
  equal under `to_lowercase`; more than one `is_billing`; any field failing
  `mailgun::validate_header_value`. It does **not** shape-check an address —
  `nigel client add --email` never has, and `wc-client-form` records the reason.
  Normalization (no flag → first row) happens after validation, in
  `set_contacts`.

- [ ] **Step 5: Fix the call sites** the dropped column broke — `grep -rn
  "clients.email\|email TEXT" src/` — and check `delete_client` needs no new
  statement (the FK cascade does it; the test from Task 1 proves it).

- [ ] **Step 6: Verify.** All three feature builds, clippy, fmt. Every
  pre-existing test in `clients.rs`, `cli/client.rs` and
  `server/routes/clients.rs` must pass **untouched** — if one needed editing,
  the projection changed behaviour and is wrong.

---

### Task 3: Cc on the way out

**Files:** modify `src/invoicing/gateway.rs`, `src/invoicing/mailgun.rs`,
`src/invoicing/send.rs`.

**Interface produced:**

```rust
// gateway.rs
fn send_invoice(&self, to: &str, cc: &[String], subject: &str, html: &str, pdf: &[u8]) -> Result<()>;

// mailgun.rs
pub fn message_fields(envelope: &EmailEnvelope, to: &str, cc: &[String],
                      subject: &str, html: &str) -> Vec<(String, String)>;

// send.rs
pub struct Recipients { pub to: String, pub cc: Vec<String> }
pub fn require_recipients(conn: &Connection, client: &Client) -> Result<Recipients>;
```

- [ ] **Step 1: Write failing tests.** In `mailgun.rs`:

```rust
#[test]
fn cc_recipients_travel_as_one_comma_joined_field() {
    let f = message_fields(&envelope(Some("Bluepeak"), None), "ap@acme.test",
        &["Dana Chen <dana@acme.test>".into(), "sam@acme.test".into()],
        "Invoice #1248", "<p>hi</p>");
    assert!(f.contains(&("cc".into(), "Dana Chen <dana@acme.test>, sam@acme.test".into())));
}

#[test]
fn no_cc_recipients_emits_no_cc_field() {
    let f = message_fields(&envelope(None, None), "a@b.test", &[], "s", "<p>hi</p>");
    assert!(f.iter().all(|(k, _)| k != "cc"));
}
```

  In `send.rs` (`cfg(feature = "pdf")` module, whose fake `Mailer` records its
  arguments):

```rust
#[test]
fn a_send_reaches_every_contact_with_the_to_first() {
    // client with a billing contact and two others
    // → recorded to == "Ada Payne <ap@acme.test>", cc has both, in position order
    // AC #3
}

#[test]
fn a_client_with_one_contact_sends_with_no_cc() { /* cc is empty, not [""] */ }

#[test]
fn a_client_with_no_contacts_still_refuses_at_precheck_with_the_same_code() {
    // conflict code "client_missing_email", message "client 'Globex' has no email",
    // and no gateway call was made — the existing test, unchanged
}

#[test]
fn a_contact_name_with_a_comma_is_quoted_in_the_recipient_header() {
    // format_address applied to a recipient, not just a sender
}
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib invoicing 2>&1 | tail -20`

- [ ] **Step 3: Implement.** `require_recipients` calls `require_email(client)`
  first — so the refusal, its code and its sentence are the ones that already
  ship — then reads `list_contacts` and formats each entry with
  `mailgun::format_address(contact.name.as_deref(), &contact.email)`, the
  billing one as `to` and the rest, in `position` order, as `cc`. Call it in
  `run()` where `require_email` is called today, at the `Precheck` step, so the
  ordering property holds: no network call before the recipients are known.

- [ ] **Step 4: Update every `Mailer` implementation and fake** — `MailgunClient`,
  and the fakes in `send.rs`, `void.rs` (if it has one) and
  `server/routes/invoices.rs`'s test module.

- [ ] **Step 5: Verify.** All three feature builds, clippy, fmt.

---

### Task 4: The InvoiceShelf importer

**Files:** modify `src/invoicing/import_invoiceshelf.rs`.

- [ ] **Step 1: Write a failing test** in its `mod tests` (the fixture at line
  ~213 already creates a `customers` table with an email):

```rust
#[test]
fn an_imported_customer_email_becomes_its_billing_contact() {
    // after import: get_client(...).email is the source address, and
    // list_contacts has exactly one row, is_billing, position 0 — AC #7
}

#[test]
fn an_imported_customer_with_no_email_gets_no_contact() { /* … */ }
```

- [ ] **Step 2: Verify it fails** (it will not compile: the raw
  `INSERT INTO clients (name, email)` names a dropped column).

- [ ] **Step 3: Implement.** Insert the client row without the email, then call
  the same `set_billing_email` writer `add_client` uses. Keep the raw insert
  rather than switching to `add_client`: import data legitimately carries blank
  and duplicate names that `add_client` refuses, and this importer's job is to
  take what is there.

- [ ] **Step 4: Verify.** All three feature builds.

---

### Task 5: The CLI

**Files:** modify `src/cli/mod.rs`, `src/cli/client.rs`, `src/main.rs`.

- [ ] **Step 1: Write failing tests** in `cli/client.rs`'s `mod tests`:

```rust
#[test]
fn a_contact_spec_parses_email_name_and_title() {
    assert_eq!(parse_contact("ap@acme.test").unwrap(),
               NewContact { email: "ap@acme.test".into(), ..Default::default() });
    let c = parse_contact("ap@acme.test:Ada Payne:AP Manager").unwrap();
    assert_eq!((c.email.as_str(), c.name.as_deref(), c.title.as_deref()),
               ("ap@acme.test", Some("Ada Payne"), Some("AP Manager")));
}

#[test]
fn a_contact_spec_with_a_colon_in_the_title_keeps_the_remainder() {
    let c = parse_contact("a@x.test:Ada:Head: Billing").unwrap();
    assert_eq!(c.title.as_deref(), Some("Head: Billing"));
}

#[test]
fn an_empty_contact_spec_is_refused_with_the_flag_in_the_message() { /* names --contact */ }

#[test]
fn the_first_contact_is_the_billing_recipient() { /* build the list, assert is_billing */ }
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib cli::client 2>&1 | tail -20`

- [ ] **Step 3: Implement.** `--contact <SPEC>` on `client add` and
  `client edit`, `num_args(1..)`-style repeatable like `--item`, with
  `conflicts_with = "email"` on both. `parse_contact` splits on `:` with the
  last field taking the remainder (`splitn(3, ':')`), trimming each part and
  treating an empty one as `None`. The first spec gets `is_billing: true`; the
  command calls `set_contacts` with the whole list.

- [ ] **Step 4: `client show`.** A contacts table (Email, Name, Title, and a
  `billing` marker column) between the label block and the invoice history,
  built with `comfy_table` like the block below it. Print nothing when the list
  is empty, matching the `No invoices.` shape.

- [ ] **Step 5: Verify.** All three feature builds, clippy, fmt,
  `cargo run -- client edit --help`.

---

### Task 6: The TUI contacts sub-screen

**Files:** modify `src/cli/client_manager.rs`.

- [ ] **Step 1: Write failing tests** in its `mod tests`:

```rust
#[test] fn c_opens_the_contacts_screen_for_the_selected_client() { /* … */ }
#[test] fn a_adds_a_contact_and_the_first_one_is_billing() { /* … */ }
#[test] fn b_moves_the_billing_flag_to_the_selected_contact() { /* … */ }
#[test] fn b_on_the_only_contact_says_so_and_changes_nothing() { /* status line */ }
#[test] fn d_removes_a_contact_and_promotes_a_new_billing_one_when_needed() { /* … */ }
#[test] fn esc_returns_to_the_client_list() { /* … */ }
#[test] fn the_client_form_still_edits_the_billing_address_through_its_email_field() {
    // the four-field form is unchanged: this screen is additive
}
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib client_manager 2>&1 | tail -20`

- [ ] **Step 3: Implement.** A `Screen::Contacts { client_id, rows, selection }`
  plus a three-field add/edit form reusing the existing `ClientForm`/`FormField`
  machinery. Keys `a` add, `e` edit, `d` delete, `b` make billing, `Esc` back;
  the footer states them. Every write goes through `set_contacts` with the whole
  list, so the screen never invents an invariant the data layer does not enforce.
  The billing row is marked in the list. `c` on the client list opens it; the
  client-list footer gains `c=contacts`.

- [ ] **Step 4: Verify.** All three feature builds, clippy, fmt, and a manual
  pass through `cargo run -- ` → `k` → `c`.

---

### Task 7: The API and the SPA

**Files:** modify `src/server/routes/clients.rs`,
`web/apps/app/src/api/{types,client}.ts`,
`web/apps/app/src/screens/{clients,invoice-data}.ts`,
`web/packages/ui/src/components/wc-client-form.ts` (+ preview, + test).

- [ ] **Step 1: Write failing route tests:**

```rust
#[tokio::test] async fn a_client_detail_carries_its_contacts() { /* billing first */ }
#[tokio::test] async fn creating_a_client_with_contacts_stores_them_all() { /* 201 */ }
#[tokio::test] async fn patching_contacts_replaces_the_whole_list() { /* like `items` */ }
#[tokio::test] async fn an_absent_contacts_field_leaves_the_list_alone() { /* … */ }
#[tokio::test] async fn email_and_contacts_in_one_body_is_a_400() { /* names both fields */ }
#[tokio::test] async fn two_billing_contacts_is_a_400_from_the_data_layers_own_check() { /* … */ }
#[tokio::test] async fn the_client_list_still_carries_one_email_per_row() {
    // GET /api/clients is unchanged: bare Client rows, no contacts, one query
}
```

- [ ] **Step 2: Verify they fail.**
  `cargo test --features serve routes::clients 2>&1 | tail -30`

- [ ] **Step 3: Implement the routes.** `contacts: Option<Vec<NewContact>>` on
  `NewClientRequest` and `ClientPatch` (a plain `Option`, not `double_option`:
  an empty array is how you clear the list, so `null` needs no meaning);
  `ClientDetail` gains `contacts`; the `email` + `contacts` conflict is a 400
  naming both fields, mirroring the CLI's `conflicts_with`. Validation stays the
  data layer's — the route adds nothing.

- [ ] **Step 4: The api seam.** `ClientContact` and `NewContact` in `types.ts`;
  `contacts` on `ClientDetail`, `NewClientRequest`, `ClientPatch`. No new
  endpoint — contacts ride on the client's own create/patch.

- [ ] **Step 5: `wc-client-form`.** A contacts repeater under the Email field:
  a row per contact (email, name, title), add/remove buttons, up/down reorder
  (the `wc-line-items` precedent — a drag handle has no keyboard equivalent that
  passes axe), and a radio column choosing the billing recipient. New preview
  states: empty, one contact, several, a long name. `describePreviewA11y`
  covers them automatically; do not restate the states in the test.

- [ ] **Step 6: The clients screen.** Opening Edit fetches
  `GET /api/clients/{id}` first — the list row is a bare `Client` and carries no
  contacts — with a loading state in the dialog and the load failure rendered
  beside the form, not as the whole screen. `clientFormFrom`/`clientPatch` in
  `invoice-data.ts` grow the contacts field; the patch sends `contacts` only
  when the list actually changed, since an all-absent PATCH is a 400.

- [ ] **Step 7: Verify.** `cargo test --features serve -- --test-threads=1`;
  from `web/`: `npm run typecheck`, `npm test`, `npm run lint`, `npm run build`.
  Re-run the invoicing fixture guard test — `Client` did not change, so
  `clients.json` should still match; if it does not, something leaked contacts
  onto the list row.

---

### Task 8: End-to-end and documentation

**Files:** modify `tests/cli_dispatch.rs`, `docs/invoicing.md`, `CLAUDE.md`,
`README.md`.

- [ ] **Step 1: Integration tests.**

```rust
#[test] fn client_add_then_contacts_then_show_lists_them_all() { /* … */ }
#[test] fn client_edit_email_and_contact_together_is_refused_by_clap() { /* … */ }
#[test] fn a_client_upgraded_from_a_single_email_keeps_it_as_the_billing_contact() {
    // init on an old-schema fixture DB if one exists, else: add a client, then
    // assert `client show` names the address as billing after migration
}
```

- [ ] **Step 2: `docs/invoicing.md`** — rewrite the Clients section's email
  paragraph around contacts: what a contact is (email, optional name and title),
  that exactly one is the billing recipient and it is the `To`, that the rest are
  `Cc`, `--email` versus `--contact` and why they conflict, and — explicitly —
  **that every recipient receives the same page and PDF and can pay the invoice
  from the link** (AC #8). Note that the published page names the billing
  contact only, and why. Amend the `{{CLIENT_EMAIL}}` placeholder row to say
  "billing contact's address".

- [ ] **Step 3: `CLAUDE.md`** — the **Invoicing** and **Client Manager**
  architecture bullets (the latter still describes a four-field form); a Key
  Design Constraints bullet recording the decision: *a client's addresses live
  in `client_contacts`, one row per contact with a name and a title, exactly one
  flagged `is_billing` (a partial unique index for at most one, a normalize step
  at every write for at least one). `clients.email` was dropped and
  `Client.email` is now a projection of the billing contact, which is what keeps
  `require_email`, `{{CLIENT_EMAIL}}` and the wire shape unchanged. A
  comma-separated column was rejected: `require_email` and the Mailgun `To` must
  know which address is which, and the source data carries a name and a title
  per address that a string column would discard. Every recipient gets the same
  document and can pay it.* Add the new commands to the Commands block.

- [ ] **Step 4: `README.md`** — the invoicing command list gains a
  `--contact` example.

- [ ] **Step 5: Verify.** `git diff --stat` shows all three docs touched.

---

## Final verification

- [ ] `cargo test -- --test-threads=1`
- [ ] `cargo test --no-default-features --features gusto -- --test-threads=1`
- [ ] `cargo test --no-default-features -- --test-threads=1`
- [ ] `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`
- [ ] From `web/`: `npm run typecheck`, `npm test`, `npm run lint`, `npm run build`
- [ ] `git diff src/models.rs src/invoicing/render_html.rs
      src/invoicing/templates/invoice.html src/invoicing/render.rs` is empty.
- [ ] `grep -rn "clients.email\|c\.email" src/ | grep -v client_contacts` finds
      nothing that reads the dropped column.
- [ ] `git diff web/apps/app/src/__fixtures__/invoicing/clients.json` is empty —
      the list row did not change shape.
- [ ] Rebase onto `main` and re-check the migration version.
- [ ] Manual: on a copy of the real books, run the migration and confirm the 23
      imported clients each kept their address as a billing contact
      (`nigel client list` should read exactly as it did before).

## Acceptance criteria mapping

| AC | Verified by |
|---|---|
| #1 more than one address | Task 2 `set_contacts_replaces_the_whole_list_in_one_transaction`; Task 5, 6, 7 surfaces |
| #2 exactly one billing recipient, `require_email` in its terms | Task 1's partial-index test; Task 2 `a_list_with_no_billing_flag_makes_the_first_row_billing`, `a_list_with_two_billing_flags_is_refused_before_anything_is_deleted`, `clearing_the_email_promotes_the_next_contact`; Task 3 `a_client_with_no_contacts_still_refuses_at_precheck_with_the_same_code` |
| #3 the Mailgun call carries the extra recipients | Task 3 `a_send_reaches_every_contact_with_the_to_first`, `cc_recipients_travel_as_one_comma_joined_field` |
| #4 the decision is recorded, including the comma column | the design spec's "Why not a comma-separated column"; Task 8 Step 3 puts the short form in CLAUDE.md |
| #5 existing single emails migrate with no re-entry | Task 1 `a_single_email_becomes_one_billing_contact`; Task 8 `a_client_upgraded_from_a_single_email_keeps_it_as_the_billing_contact` |
| #6 CLI, TUI and web read and edit the full set | Tasks 5, 6, 7 |
| #7 the InvoiceShelf importer maps its address | Task 4 `an_imported_customer_email_becomes_its_billing_contact` |
| #8 settled and documented whether a cc can pay | Task 8 Step 2 — yes, one render, one message, everyone can pay |
