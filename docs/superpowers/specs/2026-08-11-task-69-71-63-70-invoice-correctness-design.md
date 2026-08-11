# Invoice engine correctness — dates that compare, a clock that is the clock, an unlock that is tested, and a name that is only advisory

Tasks: TASK-69, TASK-71, TASK-63, TASK-70 (epic TASK-86, stream 1 — *Invoice engine correctness*).
All four land as one PR. Nothing here changes a screen, a route shape, or a printed table.

## Problem

Four separate reports, one theme: the invoicing data layer trusts strings it
should be normalizing, trusts a caller it should be asking, and promises a
uniqueness the schema does not keep.

1. **TASK-69 — `validate_date` accepts what it will not compare.**
   `invoices::validate_date` (`src/invoicing/invoices.rs:289`) parses with
   chrono's `%Y-%m-%d`, which accepts `2026-8-7`, and then throws the parsed
   date away — the caller stores the string the user typed. `refresh_status`
   compares `today > due_date` **as strings** (`is_overdue`,
   `src/invoicing/invoices.rs:280`), so `"2026-08-11" > "2026-8-7"` is *false*
   (`'0'` < `'8'` at index 5) and an invoice a month past due never derives
   `overdue`.
2. **TASK-71 — `update_invoice` calls `refresh_status` with the wrong day.**
   `src/invoicing/invoices.rs:454-458` passes the invoice's own issue date as
   "today". Harmless while only unpublished drafts are editable (they derive
   `draft` before the overdue branch is reached), and wrong the moment that
   changes.
3. **TASK-63 — no test drives an invoice command against an encrypted
   database.** `tests/cli_dispatch.rs` covers `backup` and `recategorize`
   through `NIGEL_DB_PASSWORD`; nothing covers `invoice`/`client`, which reach
   the database through the same `main.rs` pre-flight.
4. **TASK-70 — `clients.name` has no `UNIQUE` constraint.** `clients::name_taken`
   is advisory: two concurrent `POST /api/clients` can both pass the check and
   both insert.

## What is actually broken in TASK-69, precisely

The task text says the unpadded date "breaks … `ar_aging`'s `parse_from_str`,
whose failure path silently falls back to *today*". **That part is not what
happens.** chrono's `%Y-%m-%d` parses `2026-8-7` successfully — the repo already
says so, in the comment on `a_malformed_as_of_date_is_invalid_not_other`
(`src/invoicing/invoices.rs:2240`). `ar_aging_detail`'s
`unwrap_or(today)` fallback (`invoices.rs:856`) fires only for genuinely
unparseable stored text, which an unpadded date is not.

What an unpadded date really costs:

| Surface | Effect |
|---|---|
| `is_overdue` / `refresh_status` | String comparison. An unpadded `due_date` never reads as past due until the year digit changes. **This is the bug.** |
| `ar_aging_detail` buckets | Correct — the date parses. |
| `AgingInvoice.due_date` (and the API's `dueDate`, the aging table, the TUI list's Due column) | Prints `2026-8-7` where every other date prints `2026-08-07`. |
| `invoice show` / `invoice list` | Same — the stored string is echoed. |

So the fix is worth making for exactly the reason the task gives, but the
regression test that matters is a `refresh_status` one, not an `ar_aging` one.
Both are specified below anyway: AC #2 names both, and a test that pins aging's
indifference is what will notice if aging ever starts comparing strings too.

## Decision 1 — `validate_date` returns the normalized date

```rust
/// `YYYY-MM-DD`, zero-padded, or an `Invalid` error naming the field.
///
/// Returns the *re-formatted* date rather than the caller's string: chrono
/// accepts `2026-8-7`, and a value stored that way is never `>` its own month
/// in `is_overdue`'s string comparison.
pub fn validate_date(value: &str, what: &str) -> Result<String>;
```

Today it returns `Result<()>`. The new shape is not an invention: it is exactly
`validate_currency`, four lines below it in the same file, which has always
returned the *normalized* (uppercased) code rather than validating in place.
Anything else — a separate `normalize_date`, or normalizing at each call site —
is a rule a future writer can forget, and `validate_date` already has six
callers across three front ends.

Every caller changes from `validate_date(x, "issue")?;` to
`let x = validate_date(x, "issue")?;` and stores **that**.

### Normalization happens at the writer, not at the front end

`create_invoice`, `update_invoice`, `record_payment` and `void_invoice` are the
only functions that write a date column. Normalizing there means:

- `nigel invoice new --issue 2026-8-7` stores `2026-08-07`.
- The TUI draft form (`cli/invoice_manager.rs:334`) and pay form
  (`cli/invoice_manager.rs:1389`) need **no change at all** — they already call
  `validate_date` only to attribute a failure to a field and then hand the raw
  string to the data layer, which is now the thing that pads it. That is the
  same "the data layer stays the sole writer" reasoning the form's own doc
  comment gives.
- `POST /api/invoices` is unaffected: `checked_date` already refuses anything
  that is not ten characters, so the API never sends an unpadded date down.

### `record_payment` gains the check it never had

`record_payment` (`invoices.rs:214`) calls `validate_payment_method` but has
**never** validated `paid_date`, and neither does `cli::invoice::pay`
(`cli/invoice.rs:481`). `nigel invoice pay 1248 --date March` writes `March`
into `invoice_payments.paid_date` today, and then hands it to `refresh_status`
as the reference day. The TUI is the only front end that checks.

`record_payment` therefore validates and normalizes `paid_date` itself, in the
same position `validate_payment_method` occupies and for the same stated
reason: *a caller reaching the data layer directly cannot get past it.*

This is a **behavior change on the CLI**: `--date March` becomes
`Invalid payment date: March (expected YYYY-MM-DD)` instead of silent junk. It
is the documented shape of the flag (`/// Payment date: YYYY-MM-DD`), so no
documented behavior regresses.

`void_invoice` normalizes `voided_on` the same way, for symmetry and because
`voided_at` is a date column like any other. Every production caller already
passes `cli::today()`, so nothing observable moves.

### What is deliberately *not* changed

- **`refresh_status` does not validate.** It is called on nearly every write; the
  writers own normalization, and a validating `refresh_status` would be a second
  authority on date shape.
- **`is_overdue` keeps comparing strings.** With every writer normalizing and
  legacy rows migrated (Decision 2), ISO strings compare correctly, and the
  existing comment saying so becomes true rather than aspirational. Parsing two
  dates on every status refresh to prove the same thing is work with no payoff.
- **The HTTP API stays stricter than the CLI.** `routes::reports::parse_date`
  requires `len() == 10`; `2026-4-1` is a 400 there and a normalized
  `2026-04-01` here. A terminal user typing a date deserves the padding done for
  them; a JSON client sending one has a bug. Only the comment at
  `src/server/routes/invoices.rs:270` changes, from "which accepts `2026-4-1`" to
  "which accepts and normalizes `2026-4-1`".
- **Two-digit years are not expanded.** `validate_date("26-8-7", …)` normalizes to
  `0026-08-07`, because chrono's `%Y` is a full year and only `%y` guesses a
  century. That value is absurd before and after this change; it is at least now
  absurd *consistently*, and it sorts and buckets like a date. Guessing `2026`
  would be inventing data.

## Decision 2 — migration v6 normalizes stored dates

Normalizing on write does nothing for a row already stored unpadded, and AC #2
("`refresh_status` and `ar_aging` behave identically for dates entered padded or
unpadded") is a claim about a database, not about a function. Migration v6
rewrites the date columns of existing rows:

| Table | Columns |
|---|---|
| `invoices` | `issue_date`, `due_date`, `published_at`, `voided_at` |
| `invoice_payments` | `paid_date` |

Rule, applied per value: parse with `%Y-%m-%d`; if it parses and the re-formatted
string differs, write the re-formatted string. **If it does not parse, leave it
alone** — a migration that rewrites what it cannot read is guessing at somebody's
books. `created_at`/`recorded_at` are excluded: they are `datetime('now')`
defaults and are neither dates nor user input.

This is safe in a way TASK-70's rejected rename is not: `2026-8-7` → `2026-08-07`
is the same day, the change is idempotent, and nothing user-visible is renamed.
It runs in a savepoint like every other migration, and `LATEST_VERSION` picks it
up automatically from the `MIGRATIONS` array.

## Decision 3 (TASK-71) — `update_invoice` takes the day as a parameter

```rust
pub fn update_invoice(
    conn: &Connection,
    invoice_id: i64,
    update: &InvoiceUpdate,
    today: &str,
) -> Result<()>;
```

Every other date-sensitive function in `src/invoicing/` takes its reference day
as an argument — `void_invoice(conn, id, voided_on)`,
`record_payment(…, paid_date, …)`, `refresh_status(conn, id, today)`,
`ar_aging_detail(conn, today)`, `sync_invoice(conn, id, today, gateway)` — and
**no module under `src/invoicing/` reads the clock**. Calling
`chrono::Local::now()` inside `update_invoice` would fix the symptom and break
that property, taking the module's deterministic tests with it.

`today` goes last, matching `void_invoice`'s shape. The two production callers
pass `cli::today()`:

- `cli::invoice::edit` grows a `today: &str` parameter and `main.rs` passes
  `&cli::today()` at the dispatch site, exactly as `Void`/`Send`/`Sync`/`Aging`
  already do.
- `src/server/routes/invoices.rs`'s `update` handler computes
  `let today = crate::cli::today();` before `with_conn_api`, exactly as its
  `void`, `pay`-adjacent and `sync` handlers already do.

The ~18 in-module test call sites gain a literal date. The pinning test is the
one the task asks for: an editable draft, published so the overdue branch is
reachable, with a due date in the past relative to the wall clock but in the
future relative to its issue date — under today's code it derives `sent`, under
the fix it derives `overdue`.

## Decision 4 (TASK-63) — two integration tests in `tests/cli_dispatch.rs`

`TestEnv` already has everything: `encrypt()` (the fixture asserts the database
really is unreadable afterwards), `TEST_TIMEOUT` (60s, so a run that reaches the
`rpassword` prompt fails instead of blocking), `write_stdin("")`, and per-command
clearing of all nine `NIGEL_*` invoicing variables so no test can reach Stripe.

1. **Happy path.** Init, one client, one draft (`init_with_client_and_invoice`),
   `encrypt("hunter2")`, then with `NIGEL_DB_PASSWORD=hunter2`:
   - a **read** — `invoice list` prints `1248`,
   - a **write** — `client add "Globex"` succeeds and `client list` shows both.
   The write matters: unlocking for a `SELECT` and unlocking for an `INSERT` are
   the same key, but a test that only reads would not notice a regression that
   left the connection read-only.
2. **Wrong password.** `invoice list` with `NIGEL_DB_PASSWORD=wrong` fails and
   stderr contains `NIGEL_DB_PASSWORD` — the sentence `db::env_password` writes
   (`"NIGEL_DB_PASSWORD did not unlock …"`). Asserting on the variable name is
   what distinguishes the documented refusal from reaching the prompt with no
   tty, which errors with `ENXIO` and would satisfy a bare `.failure()`. This is
   `backup_fails_fast_on_wrong_env_password`'s exact reasoning, reused.

Nothing about the invoicing commands needs to change for these to pass — the
point is that nothing *may* change without them noticing. `invoice`/`client` are
not in `main.rs`'s `needs_password` exclusion list, and with no
`stripe_secret_key` configured the launch sync returns before opening anything.

## Decision 5 (TASK-70) — no `UNIQUE` index; advisory-only, documented as deliberate

**Recommendation: do not add the constraint.** Document `clients.name` uniqueness
as an advisory data-layer rule, and pin the one place that deliberately does not
honour it.

### Why

1. **It is the house pattern, not an invoicing oversight.** `accounts.name` and
   `categories.name` (`src/db.rs` `SCHEMA`) are `TEXT NOT NULL` with no unique
   index, each guarded by the same kind of data-layer check —
   `accounts::add_account` is the precedent `clients::name_taken`'s own doc
   comment cites. The `UNIQUE`s in this schema are all on machine-generated
   identity: `invoices.number`, `invoices.token`,
   `invoice_payments.stripe_checkout_session_id`, `csv_profiles.name`.
   Constraining `clients.name` alone makes invoicing the odd corner; constraining
   all three is a different, larger task — and would be actively wrong for
   `categories`, which soft-deletes (`is_active`), so a retired `Travel` and a new
   `Travel` must be able to coexist.
2. **Nothing resolves a client by name.** The only `WHERE name = ?` on `clients`
   in production code is `name_taken` itself (`src/invoicing/clients.rs:14`);
   the other hit is a test helper. Invoices carry `client_id`. A duplicate row is
   a confusing picker entry, not a wrong figure, a broken join, or an orphan.
3. **The InvoiceShelf import must be free to mirror its source.**
   `import_invoiceshelf.rs:63` inserts customers with raw SQL, bypassing
   `add_client` deliberately — it is a faithful copy of another system's
   customer table, and that system does not guarantee unique names. Under a
   `UNIQUE` index a single duplicated customer name aborts the entire one-time
   migration (it is all one transaction), turning a cosmetic annoyance into a
   blocked import. Renaming on import instead would make Nigel silently disagree
   with the invoices the user already sent from InvoiceShelf.
4. **The migration would have to rewrite user-visible data.** Pre-existing
   duplicates would need a deterministic rename (`Acme Co (2)`), and a client's
   name is printed on invoices that have already been published and emailed.
   Compare Decision 2's date migration, which changes no meaning at all: that is
   the bar a schema-driven data rewrite has to clear, and a rename does not clear
   it.
5. **The race is real but narrow and cheap to lose.** Two `POST /api/clients` in
   flight at once, from one operator's browser against a loopback-bound
   single-user server (multi-user is TASK-32 and unshipped), interleaving
   check/check/insert/insert. The outcome is two rows, and the cure is renaming
   one on the clients screen.

### What lands instead

- `docs/invoicing.md`'s "Clients" section, which currently states flatly that a
  name "must be unique", says where that rule lives and what it does not cover.
- A `CLAUDE.md` Key Design Constraints bullet recording the decision and the
  reason, so the next person does not re-litigate it from the schema.
- A test in `import_invoiceshelf.rs` pinning that two source customers with the
  same name import as two clients. It passes today; it is there because it is
  precisely what a future `UNIQUE` index would break, which makes it the decision
  written down where the compiler can see it.

### Considered and rejected

- **`UNIQUE` index + rename migration** — points 3 and 4 above.
- **Case-insensitive (`COLLATE NOCASE`) index** — stricter than `name_taken`'s
  binary `=`, so `acme` after `Acme` would raise a raw
  `UNIQUE constraint failed: clients.name` (a 500 over HTTP) where the data layer
  says it is fine. A constraint that disagrees with the check above it is worse
  than no constraint.
- **Closing the race without a schema change**, by making `add_client` a single
  conditional `INSERT … WHERE NOT EXISTS`, or by wrapping check-and-insert in
  `Transaction::new_unchecked(conn, TransactionBehavior::Immediate)`. Both work
  and are ~10 lines. Both are left out here because the race has no deterministic
  regression test — proving it needs two threads and a barrier, and such a test
  passes by luck against the unfixed code. It is listed as an open question
  rather than smuggled in: a fix nothing pins is a fix that rots.

## Edge cases

| Input | Behavior after this PR |
|---|---|
| `--issue 2026-8-7` | Stored `2026-08-07`; `invoice show` prints the padded form |
| `--issue " 2026-8-7"` (leading space) | chrono trims before a numeric item, so it parses and stores padded |
| `--issue 2026-08-07extra` | `Invalid issue date: 2026-08-07extra (expected YYYY-MM-DD)` — unchanged |
| `--issue 2026-13-01` | Refused — unchanged |
| `--issue 26-8-7` | Stored `0026-08-07` (see Decision 1) |
| `--date March` on `invoice pay` | **Now refused**: `Invalid payment date: March (expected YYYY-MM-DD)` |
| A pre-existing row holding `2026-8-7` | Rewritten to `2026-08-07` by migration v6 |
| A pre-existing row holding `March` | Left alone by v6; `ar_aging_detail` keeps its `unwrap_or(today)` fallback |
| `PATCH /api/invoices/1248 {"dueDate":"2026-8-7"}` | 400 — the API's stricter parse, unchanged |
| Editing a draft's due date | `refresh_status` now runs against the wall clock, not the issue date |

## Out of scope

- **TASK-35** (overdue goes stale between events — nothing recomputes status on
  read). This PR makes the *derivation* correct when it runs; it does not add a
  new time to run it.
- **TASK-36** (Stripe payments dated at sync time). `sync_invoice` keeps passing
  `today` as the payment date.
- **TASK-59** (`f64` money).
- Repairing unparseable stored dates, or reporting them anywhere.
- Making the CLI as strict as the HTTP API about date shape (the ten-character
  rule). Normalizing is the opposite choice, taken deliberately.
- Any `UNIQUE` index on `accounts.name` or `categories.name`.
- Any visual change: no new command, no new flag, no new column, no route shape.

## Open questions for the orchestrator

1. **`invoice pay --date` becoming strict.** `nigel invoice pay 1248 --date March`
   currently succeeds and writes junk; after this PR it fails. Confirm that is
   wanted (it is the flag's documented contract), or say the word and
   `record_payment` normalizes what parses and passes anything else through
   unchanged — which would leave the hole open.
2. **Migration v6 rewriting stored dates.** It touches five columns across two
   tables on every existing database. Confirm the appetite; if you would rather
   ship the write-path fix alone, drop Task 3 and AC #2 holds for new data only.
3. **TASK-70's race.** The recommendation is to document advisory-only and stop
   there. If you want the race actually closed, say so and `add_client`/
   `update_client` become a conditional `INSERT`/`UPDATE` in the same PR —
   accepting that the concurrency itself stays untested.
4. **`update_invoice`'s new parameter.** Positional `today: &str` last, matching
   `void_invoice`. The alternative is a field on `InvoiceUpdate`, which would let
   a caller forget it; flag it if you prefer the struct.
