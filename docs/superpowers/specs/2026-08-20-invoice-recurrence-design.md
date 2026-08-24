# Invoice Duplication and Recurring Schedules (TASK-7, TASK-81) — Design

**Goal:** Duplicate any invoice into a fresh draft in one action, and let a
schedule do the same on a cycle: a stored schedule plus one idempotent command
that generates whatever is due, built for cron/launchd, drafting by default.
Duplication is the shared core; the generator is its unattended caller.

## Facts the design rests on

- `create_invoice(conn, client_id, issue_date, due_date, currency, items, notes,
  terms)` allocates the number from `next_invoice_number` metadata, generates the
  public-URL token, writes invoice + line items + counter in one transaction.
  Invoices carry separate line items; draft is the only editable state; numbers
  are never reused.
- No daemon, deliberately: launch-time Stripe sync is the only background-ish
  work. The cron/launchd + `NIGEL_DB_PASSWORD` arrangement is documented for
  backups (README "Automated backups") and exists because of TASK-39.
- Idempotency precedent cuts both ways in this codebase: invoice sync dedups by
  checkout-session id; `import_invoiceshelf` doubles everything on a second run.
  TASK-81 must be the former, by recorded provenance, not date inference.
- `invoicing_status` gates: `sync_configured` = Stripe key; `send_configured` =
  the full nine-key set. `require_email` refuses clients with no address.
- All invoicing derivations take their reference day as a parameter; nothing
  under `src/invoicing/` reads the clock.
- **Migration numbering:** the schedule tables are migration v12, which sits
  above account classification (v10) and import integrity (v11). The contiguity
  test in `migrations.rs` fails while v11 is still unmerged — that is the guard
  holding the merge order, not a defect in this branch.

## Design

### 1. Duplication is a core function (TASK-7)

`invoicing::invoices::duplicate_invoice(conn, source_id, issue_date) -> Result<i64>`:

- **Copied:** client, currency, notes, terms, every line item (description,
  quantity, unit amount).
- **Regenerated:** number (fresh `next_number`), token, `status = 'draft'`;
  `published_at`/`voided_at`/Stripe link fields start empty.
- **Dates:** `issue_date` is the caller's (surfaces default it to today). When
  the source has a due date, the new draft preserves the source's issue→due
  *offset* in days — a Net-14 invoice duplicates as Net-14 — otherwise no due
  date. The offset rule is documented on the function.
- Any source duplicates — draft, sent, paid, void — because duplication reads a
  shape, not a state. It runs through `create_invoice`'s own validation, so an
  archived client refuses exactly as a hand-created invoice would.

Surfaces: CLI `nigel invoice duplicate <number>`; TUI detail-screen action that
creates the draft and lands on it (mirroring the existing confirm-action
pattern); web `Duplicate` button in the invoice actions block calling
`POST /invoices/{number}/duplicate`, navigating to the new draft. TASK-7's
acceptance criteria are set from this section.

### 2. Schedules own their shape (TASK-81)

Two tables (one migration, renumber-aware):

- `invoice_schedules`: client, cadence (`monthly` | `quarterly` | `yearly`),
  anchor day-of-month (clamped in short months, the `clamp_day` precedent),
  `next_period`, net-days for the due date (nullable), notes/terms/currency,
  `autosend INTEGER NOT NULL DEFAULT 0`, `ended_at` nullable, `paused INTEGER
  NOT NULL DEFAULT 0`.
- `invoice_schedule_items`: the schedule's own line items. A schedule is seeded
  either from explicit items or from an existing invoice via `--from <number>`
  (the duplication core reading the same shape), and is editable thereafter —
  items are fixed at schedule level and re-read per run, so editing the schedule
  changes future invoices and never past ones. Ending a schedule sets
  `ended_at`; nothing is deleted, so history survives (AC #8).
- `invoice_schedule_runs`: `(schedule_id, period, invoice_id, generated_at)`
  with `UNIQUE(schedule_id, period)` — provenance and idempotency in one row,
  no change to the `invoices` table. A period is a date string (the cycle's
  scheduled issue date), so "which period produced this invoice" is a join,
  satisfying AC #3 without date inference.

### 3. One command generates what is due

`nigel invoice schedule run` (plus `add`/`list`/`show`/`edit`/`pause`/`resume`/
`end` under `invoice schedule`):

- For every active schedule, walk periods from `next_period` through `today`
  (parameterized for tests): each missed cycle generates its own invoice dated
  the *period's* issue date, not the run day — catch-up bills every missed cycle,
  in order, and a February invoice generated in March is honestly already due
  (AC #5, documented in `docs/invoicing.md`).
- Each generation is one transaction: duplicate-shaped `create_invoice` from the
  schedule's items, insert the run row, advance `next_period`. Sequential
  numbering falls out of `next_number` running inside each transaction (AC #7);
  a rerun finds the `UNIQUE(schedule_id, period)` row and generates nothing
  (AC #3, tested by running twice).
- **Draft by default; sending is per-schedule opt-in** (`autosend`, AC #4). An
  autosend schedule sends through the existing send path only when
  `send_configured`; an unsendable client (no email, config missing) still gets
  its draft generated and the run's report names it and why — reported, never
  silently skipped, never half-sent (AC #9). The run's exit status reflects
  send failures so cron surfaces them.
- Unattended: the command participates in the same `NIGEL_DB_PASSWORD`
  non-interactive unlock as backups and **never prompts** — with no password
  available on an encrypted database it fails with a clear sentence (AC #10).
  `docs/invoicing.md` and the README gain the cron/launchd wrapper-script
  section mirroring the backups one, secret store and all.
- Month-end: a monthly schedule anchored on the 31st bills the 28th/29th/30th in
  short months and returns to the 31st after (AC #6, table-tested).

### 4. Surfaces stay CLI-first

The schedule surface ships CLI-complete. TUI and web schedule management are
deliberately not in this branch — the web invoices screen gains nothing, and a
follow-up task covers a schedules surface if wanted. (The *duplicate* action does
ship on all three surfaces; it is small and pattern-following.)

## Testing

- Duplication: field-by-field copy/regenerate table test; offset rule; void/paid
  sources; archived-client refusal; the three surface wirings (route test, TUI
  test per the manager patterns, CLI dispatch test).
- Schedules: idempotent rerun; multi-cycle catch-up ordering and dating;
  month-end clamping table; autosend opt-in vs draft default; unsendable-client
  reporting; paused/ended schedules generate nothing; numbering sequential
  across a multi-schedule run; `NIGEL_DB_PASSWORD` path via the existing
  non-interactive-unlock test conventions.
- Fixture cast only; serial suites both variants; web suite for the one button.

## Out of scope

- TUI/web schedule management surfaces (follow-up if wanted).
- Any ledger/journal interaction (decision-6: invoices post nothing in v1).
- Reminder emails, dunning, or send-retry machinery.
