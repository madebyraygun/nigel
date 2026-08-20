# Invoicing Correctness (TASK-36, TASK-35, TASK-66) — Design

**Goal:** Three scoped fixes on one branch: a Stripe payment is dated the day the
client paid, an invoice reads as overdue the day it becomes overdue, and a negative
`--amount` gets the validator's real answer instead of clap's misleading tip.

## Facts the design rests on

- `PaidSession` is `{session_id, amount}` (`invoicing/gateway.rs`); the private
  `Session` struct in `invoicing/stripe.rs` deserializes `{id, status,
  payment_status, amount_total}` and drops Stripe's `created` Unix timestamp on the
  floor. `sync_invoice` then records the payment with the caller's `today` — so a
  December payment synced in January lands in the wrong year (TASK-36).
- `status` is a stored column refreshed only by events (`record_payment`,
  `update_invoice`, `void_invoice`, `mark_published`). Nothing refreshes on read:
  CLI list/show, the HTTP list/detail routes, and the TUI all print the stored
  value, while `ar_aging_detail` computes buckets independently from due dates —
  which is exactly how two surfaces disagree about the same invoice (TASK-35).
- All invoicing dates are naive local `YYYY-MM-DD` strings, lexicographically
  comparable; nothing under `src/invoicing/` reads the clock — every derivation
  takes its reference day as a parameter (`clock.rs`'s documented rule).
- `InvoiceCommands::Pay`'s `amount: Option<f64>` has no hyphen handling, so
  `--amount -5` dies in clap with a `-- -5` tip that also fails; only `--amount=-5`
  reaches `payment_amount`'s real "must be greater than zero" message (TASK-66).
  No precedent for `allow_negative_numbers` exists in the repo; this introduces it.

## Design

### 1. Payments are dated by Stripe (TASK-36)

- `Session` deserializes `created` (Unix seconds); `PaidSession` gains
  `paid_at: Option<i64>` — optional defensively, since every fake gateway in the
  test modules constructs `PaidSession` by hand.
- `sync_invoice` derives the payment date from `paid_at` — epoch → local calendar
  day, matching the books' local-date convention — and falls back to the run's
  `today` only when the timestamp is absent. **The stored `paid_date` is the
  client's payment day; the run's `today` remains the reference day for status
  derivation.** If `record_payment`'s single date parameter currently serves both
  roles, it grows an explicit reference-day parameter rather than letting a
  backdated payment time-travel the status refresh.
- Tests pin the year-boundary case: a session paid in late December, synced in
  January, records a December `paid_date` and does not disturb the aging math.
  Epoch→local conversion is tested with fixed epochs; if local-timezone rendering
  makes a test environment-dependent, the test pins the conversion function with
  an explicit timezone rather than the wall clock.

### 2. Overdue is read-time truth (TASK-35)

The stored column stays the event-driven record; **reads gain a derivation**. One
function — `effective_status(status, due_date, amount_owing, today)` — overlays
`overdue` onto a stored `sent`/`partial` when `is_overdue(due_date, today)` and
money is owed, and is applied at the data-layer read paths (`list_invoices`,
`get_invoice`, and friends), each taking `today` as a parameter per the standing
clock rule. CLI, TUI, and the HTTP routes all read through it, so every surface
agrees with the aging report on the same day.

- **No writes on read.** A GET never mutates; the column is corrected the next
  time an event fires, as today.
- SQL that filters `WHERE status IN ('sent','partial','overdue')` (sync, aging)
  is unaffected: effective-overdue rows are stored as `sent`/`partial`, already
  in every such set.
- Tests: an invoice past due with money owing reads `overdue` from list, show,
  the detail route, and the TUI data source on the same `today` the aging report
  uses; a paid or void invoice never does; the stored column is untouched by
  reads.

### 3. The negative amount reaches the real validator (TASK-66)

`allow_negative_numbers = true` on Pay's `amount`, so `--amount -5` parses and
`payment_amount` answers with its own sentence ("must be a finite number greater
than zero"). A `cli_dispatch` test pins the space-separated spelling's stderr and
exit code. No other argument changes.

## Testing

Per-fix TDD as above; full serial suites both feature variants; no web changes
expected (the routes' JSON shape is unchanged — `status` simply reads truer), so
the web suite must pass untouched. Fixture cast only.

## Out of scope

- Recurring schedules and duplication (the sibling branch, TASK-7/81).
- Rewriting status storage to fully computed-at-read (the `IN`-filter rework it
  would force is not worth it for this bug).
- Any Stripe API surface beyond deserializing `created`.
