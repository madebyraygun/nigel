# Invoicing Correctness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Stripe payment is dated the day the client paid, an invoice reads as overdue the day it becomes overdue on every surface, and `--amount -5` gets the app's own validation sentence instead of clap's misleading tip.

**Architecture:** Three scoped fixes on one branch. (1) `PaidSession` carries Stripe's `created` timestamp; `clock::local_day` turns an epoch instant into the local calendar day the books are kept in; `sync_invoice` records that day as `paid_date` and passes the run's day separately, so `record_payment` grows an explicit reference-day parameter rather than deriving status from a backdated payment. (2) The stored `status` column stays the event-driven record and nothing writes on read; one pure function, `effective_status`, overlays `overdue` onto a stored `sent`/`partial` at the data layer's read paths, and the CLI, the TUI and the HTTP routes all read through it with a reference day they pass in. (3) `allow_negative_numbers` on Pay's `--amount`.

**Tech Stack:** Rust 2021, rusqlite 0.31 (SQLite), chrono 0.4, clap 4 (derive), axum (behind the `serve` feature), ratatui (TUI), `assert_cmd` for the spawn-the-binary tests.

**Spec:** `docs/superpowers/specs/2026-08-20-invoicing-correctness-design.md` — binding. Tasks: `backlog task 36 --plain`, `backlog task 35 --plain`, `backlog task 66 --plain` (read-only; never edit a task file on this branch).

## Spec-vs-code conflicts, resolved here

Seven places where the spec, the acceptance criteria and the code as it stands do not line up. Each is resolved below and the resolution is what the tasks implement.

1. **"Session completion timestamp" (TASK-36 AC #1) vs `created` (spec).** A Stripe Checkout Session carries `created`, not a completion time; the spec puts "any Stripe API surface beyond deserializing `created`" out of scope. `PaidSession::paid_at` therefore carries `created`, and the field is documented as the day Stripe recorded the session. A session is completed within minutes of being created, so the calendar day is the same in every practical case — which is the fact AC #3 turns on.

2. **`get_invoice` cannot take a reference day.** The spec says the derivation is applied at "`list_invoices`, `get_invoice`, and friends, each taking `today` as a parameter". `get_invoice` has 127 call sites, nearly all of them write paths (`refresh_status`, `ensure_editable`, `void_invoice`, `delete_invoice`) that must keep reading the *stored* column. Growing its signature would push a display concern through every writer. Resolution: `get_invoice` is unchanged; the read paths get named siblings — `get_invoice_as_of`, `get_invoice_by_number_as_of`, and `with_effective_status` for a caller that has already loaded the row. `list_invoices` does take `today`, as the spec says, because every one of its callers is a read.

3. **The `--status` filter.** The spec only reasons about the `IN ('sent','partial','overdue')` filters in sync and aging. It says nothing about `list_invoices(status = Some("overdue"))`, which filters the stored column: after the overlay, `--status sent` would return rows the same call renders as `overdue`, and `--status overdue` would miss them. Resolution: the SQL selects the stored candidate set that *could* present as the requested word (`overdue` widens to the open set), and the rows are then kept by their **effective** status. `open` keeps everything it selected, since all three effective values are in that set.

4. **Aging agreement for an invoice with no due date.** `ar_aging_detail` ages from `COALESCE(due_date, issue_date)`, so a due-date-less invoice issued 60 days ago sits in the `31-60` bucket, while TASK-35 AC #3 forbids ever calling it overdue. AC #3 wins: `is_overdue(None, _)` stays `false`. AC #2's agreement is defined over invoices that *have* a due date, and a test pins both halves so the boundary is recorded rather than assumed.

5. **The HTTP reference day and the committed parity fixtures.** The spec says no route contract changes. But `/api/invoices` and `/api/invoices/{number}` feed `web/apps/app/src/__fixtures__/invoicing/`, captured from a database anchored at `testutil::AS_OF = 2026-03-15`, and `invoicing-parity.test.ts` asserts `manifest.asOf === '2026-03-15'`. A route reading only the wall clock would force those fixtures to be recaptured against whatever day the capture ran, destroying the fixture's `sent`/`partial` variety and the anchor the web suite pins. Resolution: both GETs accept an optional `asOf`, defaulting to the server's today — the same knob `/api/invoices/aging` already has, in the same module, spelled the same way. The JSON *shape* is unchanged, the SPA sends nothing new, and the captured `.json`/`.txt` files come back byte-identical. `docs/api.md` gains the two parameter entries; this is the one documented contract change.

6. **`allow_hyphen_values` (TASK-66 text) vs `allow_negative_numbers` (spec).** The spec's is narrower — it admits `-5` but still refuses `--amount --oops` — and the spec is binding. `allow_negative_numbers = true`.

7. **`record_payment`'s date serves two roles today.** The spec's conditional is true: the current call passes `paid_date` to `refresh_status`. It therefore grows a trailing `today` parameter, and every one of its ~40 call sites is updated in Task 2.

One further boundary, decided here because the spec does not reach it: a **write refusal's** `details.status` (the 409 from `enrich_block`/`enrich_conflict`) keeps reporting the **stored** record. Those payloads describe the row a write refused, not what a reader sees.

## Global Constraints

Every task's requirements implicitly include this section.

- **⛔ Public repository — no real book data**, in any file, test, fixture or commit message. Fictional cast only: Acme, Cedar Systems, Juniper Labs, Harbor & Vale, Globex, Initech, Northwind Traders, Umbrella Corp, with invented amounts. The pre-commit hook runs `./scripts/check-no-real-data.sh --staged`; **judge it by its exit status, never by grepping its output**, and never bypass it (`--no-verify` is forbidden).
- **Tests run serially:** `cargo test -- --test-threads=1`. The DB password is a process global.
- **Both feature variants must pass:** `cargo test -- --test-threads=1` and `cargo test --no-default-features -- --test-threads=1`. The second compiles without `serve`, so the routes are absent there — everything under `invoicing/` and `clock.rs` must compile and pass without them.
- **`cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` must be clean.** CI runs fmt first and a failure there fails the build.
- **The web suite passes untouched.** No file under `web/` may be edited by hand, and the committed invoicing fixtures (`web/apps/app/src/__fixtures__/invoicing/*.json`, `*.txt`) must come back byte-identical from a recapture. Only `manifest.json`'s `params` may change, and only to gain `asOf`.
- **The invoicing no-clock rule.** Nothing under `crates/nigel-core/src/invoicing/` reads the clock: every derivation takes its reference day as a parameter (`crates/nigel-core/src/clock.rs`). `clock::local_day` converts a *given* instant and is not a clock read; the run's day still arrives as an argument.
- **Every invoicing date is normalized by the writer**, never by the caller: `validate_date` returns the padded form, and a function that derives a status validates the reference day it derives against (`validate_date(today, "reference")`).
- **No provenance comments.** No "added because", "renamed from", "don't change back", no migration history in prose. Describe the current state; `git log` is the audit trail.
- **Backlog task files are never edited on this branch.** Reads go through `backlog task <id> --plain`.
- **Only what the acceptance criteria say.** More work means editing the AC first or filing a follow-up.

## File Structure

| File | Responsibility after this plan |
|---|---|
| `crates/nigel-core/src/clock.rs` | The app's one clock read, plus `local_day` — the one place that turns an instant into the local calendar day |
| `crates/nigel-core/src/invoicing/gateway.rs` | `PaidSession` grows `paid_at: Option<i64>` |
| `crates/nigel-core/src/invoicing/stripe.rs` | `Session` deserializes `created`; `parse_paid_sessions` carries it through |
| `crates/nigel-core/src/invoicing/sync.rs` | Derives `paid_date` from `paid_at` with a today-fallback; passes the run's day as the reference day |
| `crates/nigel-core/src/invoicing/invoices.rs` | `record_payment`'s reference day; `effective_status`, `with_effective_status`, `get_invoice_as_of`, `get_invoice_by_number_as_of`; `list_invoices` takes `today` and filters on the effective status |
| `crates/nigel/src/cli/invoice.rs` | `list`/`show` take and pass a reference day |
| `crates/nigel/src/cli/invoice_manager.rs` | The TUI's list and detail reads take a reference day |
| `crates/nigel-core/src/server/routes/invoices.rs` | `list`/`detail` take an optional `asOf`; `detail_for` takes the reference day |
| `crates/nigel/src/cli/fixture_capture.rs` | Captures both sides of the parity fixtures at `AS_OF` |
| `crates/nigel/src/cli/mod.rs` | `allow_negative_numbers` on Pay's `--amount` |
| `crates/nigel/tests/cli_dispatch.rs` | The spawn-the-binary tests for read-time overdue and the negative amount |
| `docs/invoicing.md`, `docs/api.md`, `docs/design-constraints.md` | The behaviour, the two new parameters, the rules |

---

### Task 1: The local day and the Stripe timestamp

**Files:**
- Modify: `crates/nigel-core/src/clock.rs`
- Modify: `crates/nigel-core/src/invoicing/gateway.rs:11-15`
- Modify: `crates/nigel-core/src/invoicing/stripe.rs:47-67`
- Modify (fakes only): `crates/nigel-core/src/invoicing/sync.rs`, `crates/nigel-core/src/invoicing/void.rs`, `crates/nigel-core/src/server/routes/invoices.rs`
- Test: the `#[cfg(test)] mod tests` in `clock.rs` (new) and in `stripe.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub fn crate::clock::local_day(epoch_seconds: i64) -> Option<String>` — `YYYY-MM-DD` in the machine's local zone, `None` for an unrepresentable instant.
  - `PaidSession { pub session_id: String, pub amount: f64, pub paid_at: Option<i64> }` in `invoicing::gateway`.

- [ ] **Step 1: Write the failing conversion tests**

Append to `crates/nigel-core/src/clock.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    /// 2025-12-31T22:30:00Z is still December where the offset is zero or west
    /// of it, and already January five hours east. The conversion is pinned
    /// against explicit zones rather than the machine's, so the test says the
    /// same thing everywhere.
    #[test]
    fn an_instant_becomes_the_day_it_falls_on_in_the_zone_it_is_read_in() {
        let new_years_eve = 1_767_220_200;
        let utc = FixedOffset::east_opt(0).unwrap();
        let east = FixedOffset::east_opt(5 * 3600).unwrap();
        let west = FixedOffset::west_opt(5 * 3600).unwrap();

        assert_eq!(day_in(new_years_eve, &utc).unwrap(), "2025-12-31");
        assert_eq!(day_in(new_years_eve, &east).unwrap(), "2026-01-01");
        assert_eq!(day_in(new_years_eve, &west).unwrap(), "2025-12-31");
    }

    /// The local reading is a real calendar day, and an instant no calendar can
    /// hold is `None` rather than a panic or a guess.
    #[test]
    fn the_local_day_is_a_real_date_and_an_impossible_instant_is_none() {
        let day = local_day(1_767_220_200).expect("a representable instant");
        assert!(
            chrono::NaiveDate::parse_from_str(&day, "%Y-%m-%d").is_ok(),
            "not a calendar day: {day}"
        );
        assert!(local_day(i64::MAX).is_none());
    }
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p nigel-core --lib clock:: -- --test-threads=1`
Expected: FAIL to compile — ``cannot find function `day_in` in this scope`` and ``cannot find function `local_day` in this scope``.

- [ ] **Step 3: Add the conversion to `clock.rs`**

Insert above the test module, below the existing `today()`:

```rust
/// The calendar day an instant falls on in a given zone, `YYYY-MM-DD`.
///
/// Generic over the zone so the conversion can be tested against a fixed
/// offset: reading it through the machine's own zone would make the answer a
/// property of the test host.
fn day_in<Tz: chrono::TimeZone>(epoch_seconds: i64, zone: &Tz) -> Option<String> {
    chrono::DateTime::from_timestamp(epoch_seconds, 0).map(|instant| {
        instant
            .with_timezone(zone)
            .date_naive()
            .format("%Y-%m-%d")
            .to_string()
    })
}

/// The local calendar day an epoch-second instant falls on.
///
/// The books are kept in local days, so this is where an instant handed over by
/// a gateway becomes one. `None` for a timestamp outside the representable
/// range, which is what a gateway sending nonsense looks like from here.
///
/// Converting a given instant is not a clock read: the reference day every
/// derivation ages against still arrives as a parameter.
pub fn local_day(epoch_seconds: i64) -> Option<String> {
    day_in(epoch_seconds, &chrono::Local)
}
```

- [ ] **Step 4: Run them again**

Run: `cargo test -p nigel-core --lib clock:: -- --test-threads=1`
Expected: PASS, 2 passed.

- [ ] **Step 5: Write the failing parser tests**

Add to the `tests` module in `crates/nigel-core/src/invoicing/stripe.rs`:

```rust
    #[test]
    fn parse_paid_sessions_carries_the_session_timestamp() {
        let json = r#"{"object":"list","data":[
            {"id":"cs_1","status":"complete","payment_status":"paid","amount_total":25000,"created":1767220200}
        ]}"#;
        let sessions = parse_paid_sessions(json).unwrap();
        assert_eq!(sessions[0].paid_at, Some(1_767_220_200));
    }

    /// A session without one is still a payment worth recording — the absence
    /// is a fact `sync` handles, not a parse failure.
    #[test]
    fn a_session_with_no_timestamp_still_parses_as_a_payment() {
        let json = r#"{"object":"list","data":[
            {"id":"cs_1","status":"complete","payment_status":"paid","amount_total":25000}
        ]}"#;
        let sessions = parse_paid_sessions(json).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "cs_1");
        assert_eq!(sessions[0].amount, 250.0);
        assert_eq!(sessions[0].paid_at, None);
    }
```

- [ ] **Step 6: Run them and watch them fail**

Run: `cargo test -p nigel-core --lib invoicing::stripe -- --test-threads=1`
Expected: FAIL to compile — ``no field `paid_at` on type `PaidSession` ``.

- [ ] **Step 7: Add the field and read it**

In `crates/nigel-core/src/invoicing/gateway.rs`, replace the `PaidSession` struct:

```rust
#[derive(Debug, Clone)]
pub struct PaidSession {
    pub session_id: String,
    pub amount: f64,
    /// When the gateway recorded the checkout session, in Unix seconds — the
    /// day the client paid, which is not the day a sync happens to run.
    ///
    /// `Option` because a gateway that answers without one is a fact `sync`
    /// handles, and because every fake in the test modules builds this by hand.
    pub paid_at: Option<i64>,
}
```

In `crates/nigel-core/src/invoicing/stripe.rs`, the private `Session` and the map:

```rust
#[derive(Deserialize)]
struct Session {
    id: String,
    status: String,
    payment_status: String,
    amount_total: i64,
    /// Unix seconds. `default` rather than required: a session missing one
    /// still carries money that has to be recorded.
    #[serde(default)]
    created: Option<i64>,
}
```

```rust
        .map(|s| PaidSession {
            session_id: s.id,
            amount: s.amount_total as f64 / 100.0,
            paid_at: s.created,
        })
```

- [ ] **Step 8: Update every fake that builds a `PaidSession`**

Run `rg -n 'PaidSession \{' crates/` and add `paid_at: None,` to each literal. As of writing they are: `invoicing/sync.rs` (in `FlakyGw::paid_sessions`, `PerLinkGw::paid_sessions`, `SlowFirstCall::paid_sessions`, and the four `Gw(vec![…])` literals in the tests), `invoicing/void.rs:673`, and `server/routes/invoices.rs:2624` and `:3192`. The fakes in `invoicing/send.rs` and `cli/invoice_manager.rs` return `Ok(vec![])` and need no change.

- [ ] **Step 9: Run the tests and the lints**

Run: `cargo test -p nigel-core --lib invoicing:: -- --test-threads=1`
Expected: PASS, including the two new parser tests.

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: no output, exit 0.

- [ ] **Step 10: Commit**

```bash
git add crates/nigel-core/src/clock.rs crates/nigel-core/src/invoicing/gateway.rs \
        crates/nigel-core/src/invoicing/stripe.rs crates/nigel-core/src/invoicing/sync.rs \
        crates/nigel-core/src/invoicing/void.rs crates/nigel-core/src/server/routes/invoices.rs
git commit -m "Carry Stripe's session timestamp through the gateway"
```

The pre-commit hook runs the no-real-data check. If it refuses, read the report and fix the content; never pass `--no-verify`.

---

### Task 2: The payment's day and the run's day (TASK-36)

**Files:**
- Modify: `crates/nigel-core/src/invoicing/invoices.rs:232-262` (`record_payment`)
- Modify: `crates/nigel-core/src/invoicing/sync.rs:39-63` (`sync_invoice`)
- Modify (call sites): `crates/nigel-core/src/invoicing/render.rs`, `republish.rs`, `void.rs`, `clients.rs`, `crates/nigel-core/src/server/testutil.rs`, `crates/nigel-core/src/server/routes/invoices.rs` (`pay_with`), `crates/nigel/src/cli/invoice.rs`, `crates/nigel/src/cli/invoice_manager.rs`, `crates/nigel/src/cli/demo.rs`, `crates/nigel/src/main.rs`
- Test: `crates/nigel-core/src/invoicing/sync.rs` tests, `crates/nigel-core/src/invoicing/invoices.rs` tests

**Interfaces:**
- Consumes: `PaidSession::paid_at` and `clock::local_day` from Task 1.
- Produces:
  - `pub fn invoices::record_payment(conn: &Connection, invoice_id: i64, amount: f64, paid_date: &str, method: &str, stripe_session: Option<&str>, today: &str) -> Result<bool>` — `paid_date` is the day the money moved; `today` is the reference day the status is derived against. Both are validated.
  - `pub fn cli::invoice::pay(number: i64, amount: Option<f64>, date: &str, method: &str, today: &str) -> Result<()>`.
  - `fn routes::invoices::pay_with<P: AssetPublisher>(conn, number, request, publisher, cfg, data_dir, today: &str)` — the trailing reference day, matching `void_with` and `send_with`.

- [ ] **Step 1: Write the failing year-boundary test**

Add to the `tests` module in `crates/nigel-core/src/invoicing/sync.rs`. Add `payments` to the `use crate::invoicing::invoices::{…}` list at the top of that module.

```rust
    /// A December payment synced in January is December's — and the invoice is
    /// still aged against the day the sync ran, not the day the client paid.
    #[test]
    fn a_payment_made_before_year_end_is_recorded_in_the_earlier_year() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "W".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        let id = create_invoice(
            &conn,
            cid,
            "2025-12-01",
            Some("2025-12-31"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        set_payment_link(&conn, id, "pl_1", "https://pay/x").unwrap();
        conn.execute(
            "UPDATE invoices SET status='sent', published_at='2025-12-01' WHERE id=?1",
            [id],
        )
        .unwrap();

        // 2025-12-29T12:00:00Z — a moment that is still December in every zone
        // a set of books could be kept in.
        let gw = Gw(vec![PaidSession {
            session_id: "cs_1".into(),
            amount: 40.0,
            paid_at: Some(1_767_009_600),
        }]);
        assert_eq!(sync_invoice(&conn, id, "2026-01-08", &gw).unwrap(), 1);

        let recorded = payments(&conn, id).unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(&recorded[0].paid_date[..7], "2025-12", "{recorded:?}");
        // Partly paid, and months past due on the day the sync ran.
        assert_eq!(get_invoice(&conn, id).unwrap().status, "overdue");
    }

    /// No timestamp: the run's own day, which is the only other day this code
    /// can honestly name.
    #[test]
    fn a_session_with_no_timestamp_is_dated_the_day_the_sync_ran() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let id = open_invoice(&conn, cid, "pl_1");

        let gw = Gw(vec![PaidSession {
            session_id: "cs_1".into(),
            amount: 40.0,
            paid_at: None,
        }]);
        assert_eq!(sync_invoice(&conn, id, "2026-08-10", &gw).unwrap(), 1);
        assert_eq!(payments(&conn, id).unwrap()[0].paid_date, "2026-08-10");
    }
```

- [ ] **Step 2: Write the failing reference-day test**

Add to the `tests` module in `crates/nigel-core/src/invoicing/invoices.rs` (its helpers already build invoices; follow the surrounding tests for the `test_conn`/`add_client`/`create_invoice` shape used there):

```rust
    /// A backdated payment must not time-travel the status refresh: the money
    /// moved in December, the run is happening in January, and the invoice is
    /// overdue on the day somebody is looking at it.
    #[test]
    fn a_backdated_payment_derives_its_status_against_the_run_day() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "W".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        let id = create_invoice(
            &conn,
            cid,
            "2025-12-01",
            Some("2025-12-31"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        mark_published(&conn, id, "2025-12-01").unwrap();

        assert!(record_payment(&conn, id, 40.0, "2025-12-29", "stripe", None, "2026-01-08").unwrap());

        assert_eq!(get_invoice(&conn, id).unwrap().status, "overdue");
        assert_eq!(payments(&conn, id).unwrap()[0].paid_date, "2025-12-29");
    }

    /// The reference day is validated like every other date the writer stores.
    #[test]
    fn a_malformed_reference_day_is_refused_by_name() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "W".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        let id = create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap();

        let err = record_payment(&conn, id, 40.0, "2026-08-05", "ach", None, "26-8-9")
            .unwrap_err()
            .to_string();
        assert!(err.contains("reference"), "got: {err}");
    }
```

- [ ] **Step 3: Run them and watch them fail**

Run: `cargo test -p nigel-core --lib invoicing::sync invoicing::invoices -- --test-threads=1`
Expected: FAIL to compile — `this function takes 6 arguments but 7 arguments were supplied`.

- [ ] **Step 4: Grow `record_payment`**

In `crates/nigel-core/src/invoicing/invoices.rs`, replace the signature, the doc comment and the refresh:

```rust
/// Record one payment and re-derive the invoice's status.
///
/// `paid_date` is the day the money moved — a gateway session's own day, or a
/// date somebody typed. `today` is the reference day the status is derived
/// against, which is the day the run is happening. They are two different
/// facts: a December payment recorded in January is December's, and aging the
/// invoice from December would leave an invoice months past due reading `sent`.
pub fn record_payment(
    conn: &Connection,
    invoice_id: i64,
    amount: f64,
    paid_date: &str,
    method: &str,
    stripe_session: Option<&str>,
    today: &str,
) -> Result<bool> {
    validate_payment_method(method)?;
    let paid_date = validate_date(paid_date, "payment")?;
    let today = validate_date(today, "reference")?;
    ensure_not_void(&get_invoice(conn, invoice_id)?, "paid")?;
    if let Some(sid) = stripe_session {
        let seen: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM invoice_payments WHERE stripe_checkout_session_id = ?1)",
            [sid],
            |r| r.get(0),
        )?;
        if seen {
            return Ok(false);
        }
    }
    conn.execute(
        "INSERT INTO invoice_payments (invoice_id, amount, paid_date, method, stripe_checkout_session_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![invoice_id, amount, paid_date, method, stripe_session],
    )?;
    refresh_status(conn, invoice_id, &today)?;
    Ok(true)
}
```

- [ ] **Step 5: Derive the payment's day in `sync_invoice`**

In `crates/nigel-core/src/invoicing/sync.rs`, replace the loop body:

```rust
    for session in gateway.paid_sessions(link_id)? {
        // The day the client paid, read off the gateway's own timestamp. A
        // session with no timestamp falls back to the run's day — the only
        // other day this code can honestly name.
        let paid_date = session
            .paid_at
            .and_then(crate::clock::local_day)
            .unwrap_or_else(|| today.to_string());
        let is_new = record_payment(
            conn,
            invoice_id,
            session.amount,
            &paid_date,
            "stripe",
            Some(&session.session_id),
            today,
        )?;
        if is_new {
            recorded += 1;
        }
    }
```

- [ ] **Step 6: Sweep the call sites**

Run `rg -n 'record_payment\(' crates/ | rg -v 'fn record_payment'` and give each call a reference day:

- **Tests** in `invoicing/render.rs`, `invoicing/republish.rs`, `invoicing/void.rs`, `invoicing/clients.rs`, `cli/invoice.rs`, `cli/invoice_manager.rs`: repeat the call's own `paid_date` as the trailing argument — e.g. `record_payment(&conn, id, 40.0, "2026-08-05", "other", None, "2026-08-05")`. That is exactly what those calls did before, so no assertion changes.
- `crates/nigel-core/src/server/testutil.rs:671` and `:704`: pass `AS_OF`. The seed already re-derives every status at `AS_OF` afterwards.
- `crates/nigel/src/cli/demo.rs:473`: `record_payment(conn, id, amount, &offset_day(today, -days_ago), "ach", None, &offset_day(today, 0))?` — the demo's `today` is a `NaiveDate`, and `offset_day(today, 0)` is its `YYYY-MM-DD` spelling.
- `crates/nigel/src/cli/invoice.rs:682` (`pay`): the function grows a trailing `today: &str` and passes it. `crates/nigel/src/main.rs:279-284` becomes `cli::invoice::pay(number, amount, &date, &method, &cli::today())`.
- `crates/nigel/src/cli/invoice_manager.rs:1557`: `record_payment(conn, invoice_id, amount, &date, method, None, &crate::cli::today())`.
- `crates/nigel-core/src/server/routes/invoices.rs:607` (`pay_with`): `pay_with` grows a trailing `today: &str` and passes it to `record_payment`; the async `pay` handler computes `let today = crate::clock::today();` before the closure — as `void` and `send` already do — and passes `&today`. The direct `pay_with` callers in that module's tests pass `AS_OF`.

- [ ] **Step 7: Run the tests and fix the two route expectations**

Run: `cargo test -p nigel-core -- --test-threads=1`

`a_partial_payment_moves_the_status_to_partial_and_a_full_one_to_paid` (`server/routes/invoices.rs:2183`) now fails: the HTTP handler derives against the server's real day, and seeded 1251 is due `2026-04-06`. Rename it and pin what it is really about:

```rust
    #[tokio::test]
    async fn a_partial_payment_leaves_a_balance_and_a_full_one_settles_it() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        // 1251: 1,850 outstanding, due 2026-04-06 — a day the wall clock is
        // past, so a part-paid 1251 reads `overdue` rather than `partial`.
        // That is the read this branch exists to make true.
        let (status, partial) = post_json(
            &app,
            "/api/invoices/1251/pay",
            &token,
            &serde_json::json!({ "amount": 500.0, "date": "2026-03-14", "method": "ach" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{partial}");
        assert_eq!(partial["status"], "overdue");
        assert_eq!(partial["paid"], 500.0);
        assert_eq!(partial["balance"], 1350.0);
        assert_eq!(partial["payments"][0]["method"], "ach");

        let (_, settled) = post_json(
            &app,
            "/api/invoices/1251/pay",
            &token,
            &serde_json::json!({ "amount": 1350.0, "date": "2026-03-15" }),
        )
        .await;
        assert_eq!(settled["status"], "paid");
        assert_eq!(settled["balance"], 0.0);
    }
```

The `partial` status itself stays covered at the data layer by the tests in `invoicing/invoices.rs`, which pin their own reference day.

Run again: `cargo test -p nigel-core -- --test-threads=1`
Expected: PASS.

- [ ] **Step 8: Run the whole suite, both variants, plus the lints**

Run: `cargo test -- --test-threads=1`
Expected: PASS.

Run: `cargo test --no-default-features -- --test-threads=1`
Expected: PASS.

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: no output, exit 0.

- [ ] **Step 9: Commit**

```bash
git add -A crates/
git commit -m "Date a Stripe payment by the day it was made"
```

---

### Task 3: `effective_status` at the data layer (TASK-35)

**Files:**
- Modify: `crates/nigel-core/src/invoicing/invoices.rs` (`is_overdue` region, `list_invoices`, new read helpers)
- Modify (call sites, minimally, to keep the tree compiling): `crates/nigel-core/src/invoicing/clients.rs:1042`, `crates/nigel-core/src/server/routes/invoices.rs:197`, `crates/nigel/src/cli/invoice.rs:322`, `crates/nigel/src/cli/invoice_manager.rs:433,447`, `crates/nigel/src/cli/fixture_capture.rs:290`
- Test: the `tests` module in `crates/nigel-core/src/invoicing/invoices.rs`

**Interfaces:**
- Consumes: `record_payment`'s seven-argument form from Task 2.
- Produces:
  - `pub fn invoices::effective_status<'a>(status: &'a str, due_date: Option<&str>, amount_owing: f64, today: &str) -> &'a str`
  - `pub fn invoices::with_effective_status(invoice: Invoice, paid: f64, today: &str) -> Invoice`
  - `pub fn invoices::get_invoice_as_of(conn: &Connection, id: i64, today: &str) -> Result<Invoice>`
  - `pub fn invoices::get_invoice_by_number_as_of(conn: &Connection, number: i64, today: &str) -> Result<Invoice>`
  - `pub fn invoices::list_invoices(conn: &Connection, status: Option<&str>, client_id: Option<i64>, today: &str) -> Result<Vec<InvoiceListRow>>`

- [ ] **Step 1: Write the failing derivation tests**

Add to the `tests` module in `crates/nigel-core/src/invoicing/invoices.rs`:

```rust
    /// The overlay is a display rule over three facts, so it is tested as one.
    #[test]
    fn effective_status_overlays_overdue_only_where_money_is_owed_past_a_due_date() {
        let due = Some("2026-08-10");

        assert_eq!(effective_status("sent", due, 100.0, "2026-08-11"), "overdue");
        assert_eq!(
            effective_status("partial", due, 60.0, "2026-08-11"),
            "overdue"
        );
        // On the due date itself, nothing is late.
        assert_eq!(effective_status("sent", due, 100.0, "2026-08-10"), "sent");
        // Nothing owed, nothing overdue.
        assert_eq!(effective_status("sent", due, 0.0, "2026-08-11"), "sent");
        // No due date is never overdue, however old the invoice is.
        assert_eq!(effective_status("sent", None, 100.0, "2030-01-01"), "sent");
        // A settled or cancelled invoice is neither.
        assert_eq!(effective_status("paid", due, 0.0, "2026-08-11"), "paid");
        assert_eq!(effective_status("void", due, 100.0, "2026-08-11"), "void");
        assert_eq!(effective_status("draft", due, 100.0, "2026-08-11"), "draft");
    }

    /// The read agrees with the aging report on the same day — the disagreement
    /// TASK-35 is about — and the stored column is not touched by looking.
    #[test]
    fn a_past_due_invoice_reads_overdue_from_the_list_and_from_the_report() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Globex", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Site build".into(),
            quantity: 1.0,
            unit_amount: 960.0,
        }];
        let id = create_invoice(
            &conn,
            cid,
            "2026-06-01",
            Some("2026-07-01"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        mark_published(&conn, id, "2026-06-01").unwrap();
        // Published before the due date, and nothing has touched it since.
        assert_eq!(get_invoice(&conn, id).unwrap().status, "sent");

        let rows = list_invoices(&conn, None, None, "2026-07-15").unwrap();
        assert_eq!(rows[0].status, "overdue", "{rows:?}");

        let report = ar_aging_detail(&conn, "2026-07-15").unwrap();
        assert_eq!(report.invoices[0].number, rows[0].number);
        assert_eq!(report.invoices[0].bucket, "1-30");

        // A read never writes: the column is still the event-driven record.
        let stored: String = conn
            .query_row("SELECT status FROM invoices WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(stored, "sent");
    }

    /// An invoice with no due date ages on the report from its issue date, but
    /// it is never overdue — the report is a cash-flow view, the status is a
    /// promise about a date nobody made.
    #[test]
    fn an_invoice_with_no_due_date_ages_on_the_report_but_never_reads_overdue() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Initech", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Audit".into(),
            quantity: 1.0,
            unit_amount: 500.0,
        }];
        let id = create_invoice(&conn, cid, "2026-06-01", None, "USD", &items, None, None).unwrap();
        mark_published(&conn, id, "2026-06-01").unwrap();

        let rows = list_invoices(&conn, None, None, "2026-07-15").unwrap();
        assert_eq!(rows[0].status, "sent", "{rows:?}");
        assert_eq!(get_invoice_as_of(&conn, id, "2026-07-15").unwrap().status, "sent");

        let report = ar_aging_detail(&conn, "2026-07-15").unwrap();
        assert_eq!(report.invoices[0].days_past_due, 44);
    }

    /// The filter answers in the same vocabulary the rows are rendered in.
    #[test]
    fn the_status_filter_selects_on_what_a_reader_sees() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "W".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        let late = create_invoice(
            &conn,
            cid,
            "2026-06-01",
            Some("2026-07-01"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        mark_published(&conn, late, "2026-06-01").unwrap();
        let current = create_invoice(
            &conn,
            cid,
            "2026-07-01",
            Some("2026-08-01"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        mark_published(&conn, current, "2026-07-01").unwrap();

        let overdue: Vec<i64> = list_invoices(&conn, Some("overdue"), None, "2026-07-15")
            .unwrap()
            .iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(overdue, vec![late]);

        let sent: Vec<i64> = list_invoices(&conn, Some("sent"), None, "2026-07-15")
            .unwrap()
            .iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(sent, vec![current]);

        // `open` selects both either way: the overlay moves a row between the
        // words inside that set, never out of it.
        assert_eq!(
            list_invoices(&conn, Some("open"), None, "2026-07-15")
                .unwrap()
                .len(),
            2
        );
    }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p nigel-core --lib invoicing::invoices -- --test-threads=1`
Expected: FAIL to compile — ``cannot find function `effective_status` `` and `this function takes 3 arguments but 4 arguments were supplied`.

- [ ] **Step 3: Add the derivation and the read helpers**

In `crates/nigel-core/src/invoicing/invoices.rs`, directly below `is_overdue`:

```rust
/// The status a reader sees on `today`, over the stored one.
///
/// The column is the event-driven record and stays that way: `refresh_status`
/// writes it when something happens to the invoice, and nothing happens the day
/// a due date passes. So the read overlays `overdue` onto a `sent` or `partial`
/// invoice that owes money past its due date, and leaves every other status
/// exactly as stored — a paid, void or draft invoice is none of the reader's
/// business here. `sent` and `partial` are the only two the overlay can widen,
/// which is why every `status IN ('sent','partial','overdue')` query already
/// selects the rows it applies to.
pub fn effective_status<'a>(
    status: &'a str,
    due_date: Option<&str>,
    amount_owing: f64,
    today: &str,
) -> &'a str {
    let owing_and_open = amount_owing > 0.0
        && (status == InvoiceStatus::Sent.as_str() || status == InvoiceStatus::Partial.as_str());
    if owing_and_open && is_overdue(due_date, today) {
        return InvoiceStatus::Overdue.as_str();
    }
    status
}

/// One loaded invoice as a reader sees it on `today`.
///
/// Takes `paid` rather than reading it, because every caller has already asked
/// for it to render a balance.
pub fn with_effective_status(mut invoice: Invoice, paid: f64, today: &str) -> Invoice {
    let effective = effective_status(
        &invoice.status,
        invoice.due_date.as_deref(),
        invoice.total - paid,
        today,
    )
    .to_string();
    invoice.status = effective;
    invoice
}

/// [`get_invoice`] for a screen: the same row, read on `today`.
pub fn get_invoice_as_of(conn: &Connection, id: i64, today: &str) -> Result<Invoice> {
    let invoice = get_invoice(conn, id)?;
    let paid = paid_amount(conn, invoice.id)?;
    Ok(with_effective_status(invoice, paid, today))
}

/// [`get_invoice_by_number`] for a screen: the same row, read on `today`.
pub fn get_invoice_by_number_as_of(conn: &Connection, number: i64, today: &str) -> Result<Invoice> {
    let invoice = get_invoice_by_number(conn, number)?;
    let paid = paid_amount(conn, invoice.id)?;
    Ok(with_effective_status(invoice, paid, today))
}
```

- [ ] **Step 4: Teach `list_invoices` the reference day**

Add above `statuses_for`:

```rust
/// The stored statuses that could present as `filter` on any day.
///
/// `overdue` is the only word that widens: an invoice a reader sees as overdue
/// is stored `sent` or `partial` until the next event touches it, so the query
/// has to select all three and let the derivation decide.
fn stored_candidates(filter: &str) -> Result<Vec<&'static str>> {
    if filter == InvoiceStatus::Overdue.as_str() {
        return Ok(OPEN_STATUSES.to_vec());
    }
    statuses_for(filter)
}
```

Change the signature and the two ends of `list_invoices`:

```rust
pub fn list_invoices(
    conn: &Connection,
    status: Option<&str>,
    client_id: Option<i64>,
    today: &str,
) -> Result<Vec<InvoiceListRow>> {
    let statuses = match status {
        Some(filter) => Some(stored_candidates(filter)?),
        None => None,
    };
```

Change the `let rows = stmt.query_map(…)` binding that follows to `let mut rows = …`, and replace the final `Ok(rows)`:

```rust
    for row in &mut rows {
        let effective =
            effective_status(&row.status, row.due_date.as_deref(), row.balance, today).to_string();
        row.status = effective;
    }
    // The filter answers in the vocabulary the rows are rendered in. `open` is
    // closed under the overlay — all three of its words stay inside the set —
    // so it needs no second pass.
    if let Some(filter) = status {
        if filter != "open" {
            rows.retain(|row| row.status == filter);
        }
    }
    Ok(rows)
}
```

- [ ] **Step 5: Keep the tree compiling**

Run `cargo build --all-targets 2>&1 | rg 'list_invoices'` and give every call site a reference day:

- The existing tests in `invoicing/invoices.rs` and `invoicing/clients.rs`: pass a literal that keeps their meaning — `"2026-08-10"` where the invoices are dated in August 2026, or the date the surrounding test already uses.
- `crates/nigel-core/src/server/routes/invoices.rs:197`: `&crate::clock::today()` for now; Task 5 replaces it with the request's reference day.
- `crates/nigel/src/cli/invoice.rs:322` and `crates/nigel/src/cli/invoice_manager.rs:433,447`: `&crate::cli::today()` for now; Task 4 replaces those with parameters.
- `crates/nigel/src/cli/fixture_capture.rs:290`: `AS_OF`; Task 5 finishes that view.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p nigel-core --lib invoicing::invoices -- --test-threads=1`
Expected: PASS, including the four new tests.

Run: `cargo test -- --test-threads=1`
Expected: PASS.

- [ ] **Step 7: Lints**

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: no output, exit 0.

- [ ] **Step 8: Commit**

```bash
git add -A crates/
git commit -m "Derive overdue at the data layer's read paths"
```

---

### Task 4: The CLI and the TUI read through it (TASK-35)

**Files:**
- Modify: `crates/nigel/src/cli/invoice.rs` (`list`, `show`)
- Modify: `crates/nigel/src/main.rs:272-273`
- Modify: `crates/nigel/src/cli/invoice_manager.rs` (`Detail::load`, `reload_list`, `InvoiceManager::new`)
- Test: `crates/nigel/tests/cli_dispatch.rs`, and the `tests` module in `crates/nigel/src/cli/invoice_manager.rs`

**Interfaces:**
- Consumes: `list_invoices(conn, status, client_id, today)`, `with_effective_status`, `get_invoice_as_of` from Task 3.
- Produces:
  - `pub fn cli::invoice::list(today: &str) -> Result<()>`
  - `pub fn cli::invoice::show(number: i64, today: &str) -> Result<()>`
  - `fn Detail::load(conn: &Connection, invoice_id: i64, today: &str) -> Result<Self>` (private to `invoice_manager`)

- [ ] **Step 1: Write the failing CLI test**

Add to `crates/nigel/tests/cli_dispatch.rs`:

```rust
/// An invoice whose due date passed with no event since publish reads `overdue`
/// on both surfaces — and looking at it does not write to it.
#[test]
fn invoice_list_and_show_report_a_lapsed_due_date_as_overdue() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    // Published in the past with a due date behind it, which is the state no
    // event will ever revisit.
    env.db()
        .execute_batch(
            "UPDATE invoices
                SET status = 'sent', published_at = '2026-01-05', due_date = '2026-02-04'
              WHERE number = 1248;",
        )
        .expect("publish the seeded invoice");

    env.cmd()
        .args(["invoice", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("overdue"));

    env.cmd()
        .args(["invoice", "show", "1248"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[overdue]"));

    let stored: String = env
        .db()
        .query_row("SELECT status FROM invoices WHERE number = 1248", [], |r| {
            r.get(0)
        })
        .expect("status");
    assert_eq!(stored, "sent", "a read wrote to the books");
}
```

- [ ] **Step 2: Write the failing TUI test**

Add to the `tests` module in `crates/nigel/src/cli/invoice_manager.rs`, beside the other manager tests. That module's helpers are `test_conn()` and `manager(&conn)`:

```rust
    /// The screen the operator actually looks at agrees with the report: the
    /// list and the detail both read the lapsed due date.
    #[test]
    fn the_manager_reads_a_lapsed_due_date_as_overdue() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Globex", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Site build".into(),
            quantity: 1.0,
            unit_amount: 960.0,
        }];
        let id = create_invoice(
            &conn,
            cid,
            "2026-01-05",
            Some("2026-02-04"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        mark_published(&conn, id, "2026-01-05").unwrap();

        // Loaded on a day past the due date, which every wall clock this runs
        // on already is.
        let detail = Detail::load(&conn, id, "2026-03-15").unwrap();
        assert_eq!(detail.invoice.status, "overdue");

        // The manager reads the wall clock, which is past 2026-02-04 too.
        let mgr = manager(&conn);
        assert_eq!(mgr.rows[0].status, "overdue");
    }
```

- [ ] **Step 3: Run them and watch them fail**

Run: `cargo test -p nigel --test cli_dispatch invoice_list_and_show_report_a_lapsed -- --test-threads=1`
Expected: FAIL — `stdout does not contain "overdue"` (the list prints `sent`).

Run: `cargo test -p nigel --lib the_manager_reads_a_lapsed -- --test-threads=1`
Expected: FAIL to compile — `this function takes 2 arguments but 3 arguments were supplied`.

- [ ] **Step 4: Thread the day through the CLI**

In `crates/nigel/src/cli/invoice.rs`:

```rust
pub fn list(today: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    println!(
        "{}",
        format_invoice_list(&list_invoices(&conn, None, None, today)?)
    );
    Ok(())
}

pub fn show(number: i64, today: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let invoice = find_invoice(&conn, number)?;
    let client = get_client(&conn, invoice.client_id)?;
    let items = line_items(&conn, invoice.id)?;
    let paid = paid_amount(&conn, invoice.id)?;
    let invoice = with_effective_status(invoice, paid, today);

    print!("{}", format_invoice_show(&invoice, &client, &items, paid));
    Ok(())
}
```

Add `with_effective_status` to the `use nigel_core::invoicing::invoices::{…}` list at the top of the file.

In `crates/nigel/src/main.rs`:

```rust
            InvoiceCommands::List => cli::invoice::list(&cli::today()),
            InvoiceCommands::Show { number } => cli::invoice::show(number, &cli::today()),
```

- [ ] **Step 5: Thread the day through the TUI**

In `crates/nigel/src/cli/invoice_manager.rs`, `Detail::load` takes the day and overlays:

```rust
    fn load(conn: &Connection, invoice_id: i64, today: &str) -> Result<Self> {
        let invoice = get_invoice(conn, invoice_id)?;
        let client = match get_client(conn, invoice.client_id) {
            Ok(client) => Some(client),
            // Only a missing row is survivable; a database failure still is not.
            Err(NigelError::NotFound(_)) => None,
            Err(e) => return Err(e),
        };
        let paid = paid_amount(conn, invoice.id)?;
        Ok(Self {
            items: line_items(conn, invoice.id)?,
            payments: payments(conn, invoice.id)?,
            // Asked of the stored row: what may be deleted is a fact about the
            // record, not about the day it is being looked at.
            deletable: delete_blocker(conn, &invoice)?.is_none(),
            invoice: with_effective_status(invoice, paid, today),
            paid,
            client,
        })
    }
```

`load_detail`, `reload_list` and `new` read the day at the screen's edge, which is where this crate reads the clock already:

```rust
    fn load_detail(&mut self, conn: &Connection, invoice_id: i64) -> Result<()> {
        self.detail = Some(Box::new(Detail::load(
            conn,
            invoice_id,
            &crate::cli::today(),
        )?));
        Ok(())
    }
```

```rust
    fn reload_list(&mut self, conn: &Connection) {
        self.rows = list_invoices(conn, None, None, &crate::cli::today()).unwrap_or_default();
```

and the same fourth argument in `InvoiceManager::new`. Add `with_effective_status` to that file's `use nigel_core::invoicing::invoices::{…}` list.

- [ ] **Step 6: Run both tests**

Run: `cargo test -p nigel --test cli_dispatch invoice_list_and_show_report_a_lapsed -- --test-threads=1`
Expected: PASS.

Run: `cargo test -p nigel --lib the_manager_reads_a_lapsed -- --test-threads=1`
Expected: PASS.

- [ ] **Step 7: Run the whole suite and the lints**

Run: `cargo test -- --test-threads=1`
Expected: PASS. If `invoice_and_client_listings_print_money_the_way_every_other_report_does` fails, read the diff: the seeded #1248 there is a `draft` with no due date and must still print `draft`.

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: no output, exit 0.

- [ ] **Step 8: Commit**

```bash
git add -A crates/
git commit -m "Read overdue on the terminal's two invoice screens"
```

---

### Task 5: The HTTP routes read through it, at a reference day the fixtures can pin

**Files:**
- Modify: `crates/nigel-core/src/server/routes/invoices.rs` (`ListQuery`, `list`, `detail`, `detail_for` and its five callers)
- Modify: `crates/nigel/src/cli/fixture_capture.rs:270-350`
- Test: the `tests` module in `crates/nigel-core/src/server/routes/invoices.rs`

**Interfaces:**
- Consumes: `effective_status`, `with_effective_status`, `list_invoices(…, today)` from Task 3.
- Produces: `GET /api/invoices?status&clientId&asOf` and `GET /api/invoices/{number}?asOf`, both defaulting to the server's today; `fn detail_for(conn: &Connection, invoice: Invoice, today: &str) -> ApiResult<InvoiceDetail>`.

- [ ] **Step 1: Write the failing route tests**

Add to the `tests` module in `crates/nigel-core/src/server/routes/invoices.rs`:

```rust
    /// The same rows, read on two days. `asOf` is what lets a caller — the
    /// fixture capture above all — ask the question on a fixed day.
    #[tokio::test]
    async fn the_list_and_the_detail_read_overdue_on_the_day_they_are_asked_about() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        // 1250 is due 2026-03-20: partly paid and not yet late at AS_OF.
        let at_as_of = ok_json(&app, "/api/invoices?asOf=2026-03-15", &token).await;
        assert_eq!(at_as_of[2]["number"], 1250);
        assert_eq!(at_as_of[2]["status"], "partial");

        let later = ok_json(&app, "/api/invoices?asOf=2026-04-15", &token).await;
        assert_eq!(later[2]["number"], 1250);
        assert_eq!(later[2]["status"], "overdue");

        let detail = ok_json(&app, "/api/invoices/1250?asOf=2026-04-15", &token).await;
        assert_eq!(detail["status"], "overdue");
        // Still the invoice it was: reading it changed nothing.
        assert_eq!(detail["paid"], 2000.0);
        assert_eq!(detail["balance"], 1200.0);
        let unchanged = ok_json(&app, "/api/invoices/1250?asOf=2026-03-15", &token).await;
        assert_eq!(unchanged["status"], "partial");
    }

    #[tokio::test]
    async fn a_malformed_as_of_on_the_list_or_the_detail_is_a_400() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        for uri in [
            "/api/invoices?asOf=2026-4-1",
            "/api/invoices/1250?asOf=yesterday",
        ] {
            let (status, body) = get_json(&app, uri, &token).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body}");
            assert_eq!(body["error"]["code"], "bad_request");
        }
    }
```

Pin the existing seeded-status assertions to `AS_OF` in the same module, so they keep asking about the day the fixture is anchored to:

- `invoices_list_is_newest_first_and_carries_the_balance`: `ok_json(&app, "/api/invoices?asOf=2026-03-15", &token)`.
- `the_list_can_be_filtered_by_status_and_client`: append `&asOf=2026-03-15` to all four URLs (`?status=open&asOf=…`, `?status=draft&asOf=…`, `?clientId=1&asOf=…`, `?clientId=1&status=sent&asOf=…`).
- `an_invoice_detail_carries_items_payments_flags_and_no_token`: `ok_json(&app, "/api/invoices/1250?asOf=2026-03-15", &token)`.
- `a_sent_paid_or_void_invoice_refuses_deletion_with_one_reason`: the detail GET inside the loop becomes `/api/invoices/{number}?asOf=2026-03-15`, so it compares like with like — the 409's `details.status` reports the stored record.

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p nigel-core --lib server::routes::invoices -- --test-threads=1`
Expected: FAIL — `assertion failed: later[2]["status"] == "overdue"` (the route still prints the stored `partial`), and the malformed-`asOf` cases answer `200`.

- [ ] **Step 3: Take the reference day on both GETs**

In `crates/nigel-core/src/server/routes/invoices.rs`:

```rust
/// The list filters, taken as strings so a malformed one lands in the error
/// envelope instead of axum's plain-text `Query` rejection.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListQuery {
    status: Option<String>,
    client_id: Option<String>,
    as_of: Option<String>,
}

/// The day a read is asked about, `asOf` or the server's own — the same knob
/// the aging route has, so the two answer the same question about the same
/// invoice on the same day.
fn reference_day(as_of: Option<&str>) -> ApiResult<String> {
    match as_of {
        Some(value) => super::reports::parse_date("asOf", value),
        None => Ok(crate::clock::today()),
    }
}
```

In `list`, before the closure:

```rust
    let today = reference_day(query.as_of.as_deref())?;
```

and inside it:

```rust
        Ok(inv::list_invoices(
            conn,
            query.status.as_deref(),
            client_id,
            &today,
        )?)
```

(`today` is moved into the closure; `query.status` already is.)

The detail handler gains the same query:

```rust
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AsOfQuery {
    as_of: Option<String>,
}

async fn detail(
    State(state): State<AppState>,
    ApiPath(number): ApiPath<i64>,
    Query(query): Query<AsOfQuery>,
) -> ApiResult<Json<InvoiceDetail>> {
    let today = reference_day(query.as_of.as_deref())?;
    let detail = with_conn_api(&state, move |conn| {
        let invoice = find_invoice(conn, number)?;
        detail_for(conn, invoice, &today)
    })
    .await?;
    Ok(Json(detail))
}
```

- [ ] **Step 4: Overlay in `detail_for`, after the flags are decided**

```rust
/// The whole invoice a screen renders, read on `today`.
///
/// The capability flags are asked of the **stored** row: what may be edited,
/// voided, paid or deleted is a fact about the record, and the derivation only
/// changes what a reader is told the invoice's state is.
fn detail_for(conn: &Connection, invoice: Invoice, today: &str) -> ApiResult<InvoiceDetail> {
    let client = get_client(conn, invoice.client_id)
        .map_err(|e| not_found_because(e, "client_not_found"))?;
    let items = inv::line_items(conn, invoice.id)?;
    let payments = inv::payments(conn, invoice.id)?;
    let paid = inv::paid_amount(conn, invoice.id)?;

    let can_edit = inv::ensure_editable(conn, &invoice).is_ok();
    let can_void = inv::ensure_voidable(conn, &invoice).is_ok();
    let not_void = inv::ensure_not_void(&invoice, "sent").is_ok();
    let can_send = not_void && client.email.is_some() && invoice.total > 0.0;
    let can_pay = not_void && inv::payment_amount(&invoice, paid, None).is_ok();
    let can_delete = inv::delete_blocker(conn, &invoice)?.is_none();

    let invoice = inv::with_effective_status(invoice, paid, today);

    Ok(InvoiceDetail {
        public_url: public_url(&invoice),
        balance: invoice.total - paid,
        invoice,
        client,
        items,
        payments,
        paid,
        can_edit,
        can_send,
        can_void,
        can_pay,
        can_delete,
    })
}
```

The four write-path callers pass the day they already hold, so a write's answer and the next read agree:
- `create` (line ~350): `detail_for(conn, inv::get_invoice(conn, id)?, &crate::clock::today())`
- `update` (line ~417): `detail_for(conn, find_invoice(conn, number)?, &today)` — `today` is already bound above.
- `void_with` (line ~487): `detail_for(conn, find_invoice(conn, number)?, today)`.
- `pay_with` (line ~624): `detail_for(conn, refreshed, today)` — the parameter Task 2 added.
- `send_with` (line ~827): `detail_for(conn, find_invoice(conn, number)?, today)`.

- [ ] **Step 5: Run the route tests**

Run: `cargo test -p nigel-core --lib server::routes::invoices -- --test-threads=1`
Expected: PASS.

- [ ] **Step 6: Capture both sides of the parity fixtures on the same day**

In `crates/nigel/src/cli/fixture_capture.rs`, the routes and the text sides:

```rust
    let as_of_query = format!("asOf={AS_OF}");
    let routes = [
        ("invoices", format!("/api/invoices?{as_of_query}")),
        ("invoice-1250", format!("/api/invoices/1250?{as_of_query}")),
        ("aging", format!("/api/invoices/aging?{as_of_query}")),
        ("clients", "/api/clients".to_string()),
    ];

    let invoice = inv::get_invoice_by_number_as_of(&conn, 1250, AS_OF).expect("1250");
```

```rust
            format_invoice_list(&inv::list_invoices(&conn, None, None, AS_OF).expect("list")),
```

and the manifest's `params`, which now follows the route rather than naming one view:

```rust
            "params": if route.contains("asOf=") {
                serde_json::json!({ "asOf": AS_OF })
            } else {
                serde_json::json!({})
            },
```

- [ ] **Step 7: Recapture and prove the fixtures did not move**

Run: `cargo test -p nigel --features serve capture_web_invoicing_fixtures -- --ignored --test-threads=1`
Expected: PASS, printing four `wrote …` lines.

Run: `git diff --stat web/`
Expected: exactly one file changed — `web/apps/app/src/__fixtures__/invoicing/manifest.json` — gaining `asOf` under the `invoices` and `invoice-1250` entries. **If any `.json` or `.txt` fixture changed, stop:** the two sides are being captured on different days, and the fix is in this task, not in the fixture.

Run: `cd web && npm test`
Expected: PASS, with `invoicing-parity` green and `manifest.asOf` still `2026-03-15`.

- [ ] **Step 8: Whole suite, both variants, lints**

Run: `cargo test -- --test-threads=1`
Expected: PASS.

Run: `cargo test --no-default-features -- --test-threads=1`
Expected: PASS.

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: no output, exit 0.

- [ ] **Step 9: Commit**

```bash
git add -A crates/ web/apps/app/src/__fixtures__/invoicing/manifest.json
git commit -m "Answer the invoice reads on the day they ask about"
```

---

### Task 6: The negative amount reaches the real validator (TASK-66)

**Files:**
- Modify: `crates/nigel/src/cli/mod.rs:485-497`
- Test: `crates/nigel/tests/cli_dispatch.rs`

**Interfaces:**
- Consumes: `init_with_client_and_invoice(&env)` and `TestEnv` from `cli_dispatch.rs`; `payment_amount`'s existing refusal.
- Produces: no new API. `nigel invoice pay <n> --amount -5` reaches `invoices::payment_amount`.

- [ ] **Step 1: Write the failing test**

Add to `crates/nigel/tests/cli_dispatch.rs`, beside the other invoice tests:

```rust
/// `--amount -5` used to die in clap with a tip (`-- -5`) that also fails, so
/// the app's own sentence was reachable only as `--amount=-5`. Both spellings
/// now get the same answer.
#[test]
fn a_negative_payment_amount_gets_the_apps_own_refusal_either_way() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    for amount in ["-5", "-5.00"] {
        env.cmd()
            .args([
                "invoice", "pay", "1248", "--date", "2026-08-20", "--amount", amount,
            ])
            .assert()
            .code(1)
            .stderr(predicate::str::contains(
                "--amount must be a finite number greater than zero",
            ))
            .stderr(predicate::str::contains("unexpected argument").not());
    }

    // The joined spelling has always reached it, and still says the same thing.
    env.cmd()
        .args(["invoice", "pay", "1248", "--date", "2026-08-20", "--amount=-5"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "--amount must be a finite number greater than zero",
        ));

    // Nothing was recorded by any of them.
    let payments: i64 = env
        .db()
        .query_row("SELECT COUNT(*) FROM invoice_payments", [], |r| r.get(0))
        .expect("payments");
    assert_eq!(payments, 0);
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p nigel --test cli_dispatch a_negative_payment_amount -- --test-threads=1`
Expected: FAIL — stderr is clap's `error: unexpected argument '-5' found` with the `tip: to pass '-5' as a value, use '-- -5'`, and the exit code is clap's `2`.

- [ ] **Step 3: Let the argument take a negative number**

In `crates/nigel/src/cli/mod.rs`, Pay's `amount`:

```rust
    /// Manually record a payment (direct deposit, etc.).
    Pay {
        /// Invoice number
        number: i64,
        /// Amount paid (default: the full outstanding balance)
        ///
        /// `allow_negative_numbers` so `--amount -5` reaches the app's own
        /// "greater than zero" refusal instead of dying as an unknown flag.
        #[arg(long, allow_negative_numbers = true)]
        amount: Option<f64>,
        /// Payment date: YYYY-MM-DD
        #[arg(long)]
        date: String,
        /// Payment method
        #[arg(long, default_value = "direct_deposit")]
        method: String,
    },
```

- [ ] **Step 4: Run it again**

Run: `cargo test -p nigel --test cli_dispatch a_negative_payment_amount -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Confirm nothing else moved**

Run: `cargo test -p nigel --test cli_dispatch -- --test-threads=1`
Expected: PASS.

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: no output, exit 0.

- [ ] **Step 6: Commit**

```bash
git add crates/nigel/src/cli/mod.rs crates/nigel/tests/cli_dispatch.rs
git commit -m "Let a negative --amount reach the payment validator"
```

---

### Task 7: The documentation

**Files:**
- Modify: `docs/invoicing.md:41-43`, `:381-382`, the "Recording payments" section around `:1105-1125`
- Modify: `docs/api.md:253-254`, `:362-385`
- Modify: `docs/design-constraints.md:39-41` (the invoicing date and status rules)

**Interfaces:**
- Consumes: everything above. No code changes.

- [ ] **Step 1: Correct the dashboard paragraph in `docs/invoicing.md`**

The current text says the list shows a stale status. Replace lines 41-43 with:

```markdown
The list shows an invoice's status as of today, exactly as `nigel invoice list`
does: an invoice that crossed its due date since it was last written reads
`overdue` without waiting for a payment or a publish to touch it. The stored
status is the record of what happened to the invoice; nothing is written by
looking at it.
```

- [ ] **Step 2: Keep the no-due-date sentence exact**

`docs/invoicing.md:381` already says an invoice with no due date never goes overdue and ages from its issue date. Extend it so the two halves are visibly deliberate:

```markdown
- `--due` is optional. An invoice with no due date never goes overdue — no date
  was promised — though the aging report still ages it from its issue date, so
  it appears in a bucket without ever being called late.
```

- [ ] **Step 3: Say how a Stripe payment is dated**

In the "Recording payments" section of `docs/invoicing.md`, after the paragraph describing what `sync` walks:

```markdown
A payment is dated the day the client paid it, read off the checkout session's
own timestamp and converted to the local calendar day the books are kept in — so
a payment made in late December and synced in January is recorded in December. A
session that arrives without a timestamp is dated the day the sync ran, which is
the only other day the run can honestly name. Either way the invoice's status is
derived against the day of the run, so a backdated payment never un-ages an
invoice.
```

- [ ] **Step 4: Document the two query parameters in `docs/api.md`**

In the read-endpoint table (line ~253):

```markdown
| `/api/invoices` | `status`, `clientId`, `asOf` | `InvoiceListRow[]` |
| `/api/invoices/{number}` | `asOf` | `InvoiceDetail` |
```

And in the prose for `GET /api/invoices` and `GET /api/invoices/{number}`, one paragraph each — the same sentence, since it is the same parameter:

```markdown
`asOf` is the day the answer is about, `YYYY-MM-DD`, defaulting to the server's
today. An invoice past its due date with money owing reads `overdue` on that day
even when the stored status still says `sent` or `partial`, which is what keeps
this endpoint and `/api/invoices/aging` agreeing about the same invoice. The
status filter selects on the same reading, so `?status=overdue` returns what the
rows are rendered as. Nothing is written by a read.
```

- [ ] **Step 5: Extend the invoicing rules in `docs/design-constraints.md`**

Add to the status rule (the entry beginning "Invoice status is derived, never set by hand"), in its own sentence at the end:

```markdown
Reads derive further: `effective_status(status, due_date, amount_owing, today)`
overlays `overdue` onto a stored `sent` or `partial` that owes money past its due
date, and `list_invoices`, `get_invoice_as_of`, `get_invoice_by_number_as_of` and
the HTTP list and detail routes all read through it with a reference day passed
in, so the CLI, the TUI, the browser and the aging report agree about the same
invoice on the same day. It is a read: no `GET` writes the column, which is still
corrected by the next event. Filters select on the derived word — `overdue`
widens to the stored open set and the rows are kept by what they render as — and
an invoice with no due date is never overdue, however far the report ages it from
its issue date. Write refusals (`enrich_conflict`, `enrich_block`) report the
stored status, because they describe the record a write refused.
```

Add to the date rule (the entry naming `validate_date` and the five writers):

```markdown
`record_payment` takes both days and validates both: `paid_date` is when the
money moved, `today` is the reference day the status derives against. A Stripe
payment's day comes from the session timestamp through `clock::local_day`, the
one place an instant becomes a local calendar day, and falls back to the run's
day when the gateway sends none.
```

- [ ] **Step 6: Check the docs against the rules**

Run: `./scripts/check-no-real-data.sh`
Expected: exit 0. Judge by the exit status, never by grepping the output.

Run: `rg -n 'added in|was formerly|changed in version|previously|used to' docs/invoicing.md docs/api.md docs/design-constraints.md`
Expected: no hits in the paragraphs this task wrote.

- [ ] **Step 7: Commit**

```bash
git add docs/invoicing.md docs/api.md docs/design-constraints.md
git commit -m "Document read-time overdue and the Stripe payment date"
```

---

## Closing the tasks

After Task 7, verify the whole branch before touching the acceptance criteria:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
cargo test -p nigel-core -- --test-threads=1
(cd web && npm run lint && npm run typecheck && npm test)
./scripts/check-no-real-data.sh
```

Every one must pass before any claim that this is done. Then tick the acceptance criteria through the CLI — never by editing a task file — and file the ticks as commits on `main`, not on this branch:

```bash
backlog task edit 36 --check-ac 1 --check-ac 2 --check-ac 3 --check-ac 4 --check-ac 5
backlog task edit 35 --check-ac 1 --check-ac 2 --check-ac 3 --check-ac 4
backlog task edit 66 --check-ac 1
```

Coverage of the criteria, so the ticks are honest:

| Criterion | Where it is discharged |
|---|---|
| 36 #1 `PaidSession` carries the timestamp | Task 1, Steps 5-7 |
| 36 #2 `sync` records from it | Task 2, Steps 1, 5 |
| 36 #3 December payment, January sync | Task 2, Step 1 (`a_payment_made_before_year_end_is_recorded_in_the_earlier_year`) |
| 36 #4 defined, documented fallback | Task 2, Steps 1, 5; Task 7, Step 3 |
| 36 #5 parser tests, including absence | Task 1, Step 5 |
| 35 #1 list and show report overdue | Task 4, Steps 1, 4 |
| 35 #2 agrees with the aging bucket | Task 3, Step 1 (`a_past_due_invoice_reads_overdue_from_the_list_and_from_the_report`) |
| 35 #3 no due date is never overdue | Task 3, Step 1 (both the unit and the report test) |
| 35 #4 due date passed, no events since publish | Task 3, Step 1; Task 4, Step 1 |
| 66 #1 space-separated `--amount -5` | Task 6, Steps 1, 3 |
