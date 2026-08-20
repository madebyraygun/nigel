# Money to Integer Minor Units (TASK-59) — Design

**Goal:** Every amount inside Nigel is an integer count of cents; dollars exist only
where a human reads them. The reconciler stops disagreeing with itself, report figures
survive byte-for-byte, and TASK-9.4's sum-to-zero ledger gets the integer arithmetic it
refuses to live without. Decision-7 records the ruling; this spec is its shape.

**Sequencing precondition:** this branch starts from a main that already contains the
import-integrity branch (parser layer with `ParseOutcome`), the account-classification
branch (rewritten reports, `natural_balance`), and the onboarding branch (demo fixtures
in `nigel-core/src/demo.rs`). All three rewrote files this conversion edits. Its
migration takes the next free number after theirs.

## Facts the design rests on

- `transactions.amount REAL NOT NULL` in the schema; `f64` amount fields in `models.rs`
  (transactions, parsed rows, at least one more); float arithmetic in `reconciler.rs`,
  the invoicing tables (`invoices`, `invoice_payments`), and every report.
- The filed symptom: `is_reconciled` computed from the unrounded discrepancy, the
  serialized discrepancy rounded to cents — `0.014` reports `0.01` beside
  `isReconciled: false`. The SPA deliberately refuses to recompute the tolerance
  client-side, which contains the damage to that one pairing.
- `parse_amount` (importer) returns `Option<f64>` post-integrity; a `None` becomes a
  recorded reject. Report totals flow through class-driven queries post-classification;
  `natural_balance(class, raw_sum)` owns sign convention.
- The demo seeder's fixture tables carry invented dollar amounts as float literals.
- The JSON API serializes amounts as decimal-dollar numbers; text/PDF reports format
  through `fmt.rs`. TASK-59 AC#3 pins all report output byte-for-byte.

## Design

### 1. The `Money` type

`nigel_core::money::Money(i64)` — cents. `Copy`, `Ord`, integer `Add`/`Sub`/`Neg`/
`Sum`. Constructors: `Money::from_cents(i64)`, `Money::parse_dollars(&str) ->
Result<Money>` (the importer's string-to-cents path: sign, thousands separators,
two-decimal handling, no float transit), and `Money::from_dollars_f64(f64) -> Money`
(rounds half-away-from-zero to cents — used exactly twice: the schema migration and
any legacy float ingestion, nowhere else). Rendering: `to_dollars_f64()` for JSON
serialization (exact for any realistic magnitude; serde_json's shortest-round-trip
printing keeps today's wire text), and `Display` deferring to the existing `fmt.rs`
conventions so text reports do not move.

Serde: `Serialize` emits the dollar number (wire-compatible); `Deserialize` accepts
the dollar number and rounds to cents (API inputs like manual amounts). The SPA and
`docs/api.md` are untouched — this is the "boundary renders dollars" rule made
mechanical.

### 2. Storage and migration

Amount columns move `REAL → INTEGER` in one migration (next free version): create-new,
`CAST(ROUND(amount * 100) AS INTEGER)` copy, rename — the SQLite column-retype dance,
inside one transaction per table. Columns: `transactions.amount`,
`invoices` money columns, `invoice_payments.amount`, `reconciliations` statement
balances, and any other REAL money column a schema audit turns up (the audit is a plan
step, not a guess). Fresh `SCHEMA` matches, and the fresh-vs-migrated schema-agreement
test pattern from the classification branch repeats here.

Idempotence: the migration probes column type via `pragma_table_info` before acting,
so a replay is a no-op — same discipline as v5/v10.

### 3. Call-site conversion, layer by layer

Mechanical but wide; the plan slices it so each layer compiles and passes before the
next: models → db read/write → importers (`parse_amount` returns `Option<Money>`;
reject reasons keep their exact strings) → categorizer/rules (amount-based rules
compare cents) → reconciler → invoicing → reports → CLI/TUI formatting → server route
serialization. The compiler drives: change the model field, chase the errors, never a
parallel `f64` path left alive. `rg 'f64'` bounded to money contexts gates the end —
remaining `f64`s must be non-money (percentages, layout math) and each one is named in
the final review.

### 4. The reconciler pairing (AC#2)

With cents, `discrepancy` is an integer and `is_reconciled` is `discrepancy == 0` —
the same value, structurally. The regression test recreates today's failing edge: a
statement balance 1.4 cents off books must serialize a discrepancy and an
`isReconciled` that agree (they round to the same cent value), pinned at the exact
tolerance edge that today disagrees.

### 5. The parity gate (AC#3)

Before the conversion branch changes anything, it captures every report on the
committed fixtures (text, PDF bytes where deterministic, JSON) to files; after, a test
compares byte-for-byte. Where PDF bytes are non-deterministic the existing smoke-test
convention stands in. The demo books and the server fixture set are the corpus. Any
intentional difference is a spec violation by definition — the conversion has no
license to change a figure.

## Testing

- `Money` unit table: parse/render round-trips including the traps (negatives,
  thousands separators, ".5" halves, half-cent rounding direction, i64 headroom).
- Migration: seeded pre-conversion database migrates with every amount equal to its
  rounded former self; replay no-op; fresh-vs-migrated schema agreement.
- The reconciler edge test (AC#2) and the byte-parity corpus (AC#3).
- Full serial suites, both feature variants, web suite untouched-but-green (the wire
  did not move — any SPA test failure is a wire regression by definition).

## Out of scope

- Currency awareness (TASK-9.4 owns the `currency` hook, decision-5).
- Journal tables (TASK-9.4), chart merge (TASK-9.3).
- Any change to displayed formats, API shapes, or stored dollar semantics.
