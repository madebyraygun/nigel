# Import Integrity (TASK-50, TASK-51, TASK-52) — Design

**Goal:** An import either happens completely and honestly or leaves no trace: no
spent checksum for a zero-row parse, no half-committed sequence on failure, and
no silently dropped rows. Three filed bugs, one seam, one branch.

## Facts the design rests on

- `importer::import_file` writes the `imports` row — filename, checksum,
  `record_count` — before parsing confirms anything, and without regard to
  `parsed_rows` being empty. A statement imported under the wrong format
  records its checksum with `record_count` 0, and every retry of the corrected
  file short-circuits on the duplicate-checksum branch (TASK-50).
- `POST /api/imports/confirm` and the CLI import both run snapshot →
  `import_file` → `categorize_transactions` on one connection with **no SQL
  transaction around any of it** — each insert and update autocommits. Compare
  `apply_review`, `undo_review`, `delete_import` and the transactions PATCH,
  which all take `conn.unchecked_transaction()`. A disk-full, a `SQLITE_BUSY`
  past the timeout, or a panic partway leaves committed transactions, some
  categorized and some not, answering 500 (TASK-51).
- Every parser increments a malformed counter and continues; the row's content
  is captured nowhere. `record_count` excludes malformed rows, so the import
  history agrees with the truncated import. A bank changing its date format
  mid-statement silently loses those rows and no report ever says so
  (TASK-52).

## Design

Sequenced as three commits on one branch, because each layer is the next one's
foundation.

### 1. One transaction around the sequence (TASK-51)

The unit of work becomes: begin transaction → `import_file` → categorize →
commit. Both entry points — the confirm route's plan and the CLI import — run
that shape via a shared function in the importer module, not two copies. The
pre-import snapshot stays where it is, taken before the transaction begins: it
is a file-level copy and the escape hatch when the transaction itself cannot
help (schema corruption, a bad migration).

- Failure anywhere inside rolls back to exactly the pre-import state: no
  `imports` row, no transactions, no partial categorization, and therefore no
  spent checksum — a retry after the cause is fixed starts clean (AC 51#2 by
  construction).
- The injected-failure test (AC 51#3) uses a categorizer handed a rule that
  fails on the fixture data (a stored regex that panics or an update that
  violates a constraint), asserting the transaction table and imports table
  afterwards equal their before state.

### 2. A zero-row parse is a refusal, not an import (TASK-50)

Inside the transaction, after parsing: if no rows parsed, the import is
refused with an error that says what happened — the format it used, the
malformed count if any, and the first few reasons — instead of committing an
empty `imports` row. Nothing is written (the transaction from §1 makes that a
rollback, not bookkeeping), so the checksum is never spent and the corrected
file imports normally (AC 50#1, #2, #4).

The CLI and the API both surface that error as a refusal, not a success with
zeros (AC 50#3). The API's existing error envelope carries it; no new shape.

### 3. Malformed rows leave a record (TASK-52)

For an import that *does* commit, malformed rows become data:

- Schema (via `migrations.rs`): `imports.malformed_count INTEGER NOT NULL
  DEFAULT 0`, and a new `import_rejects` table —
  `(id, import_id REFERENCES imports(id) ON DELETE CASCADE, row_number,
  content TEXT, reason TEXT)`. Rejects ride inside §1's transaction, and undo
  (`delete_import`) removes them with the import via the cascade.
- Parsers stop counting anonymously: the malformed path captures the raw row
  and a reason string (what failed to parse, in the parser's own words).
- Surfacing: the import history (CLI and `GET /api/imports`) shows the
  malformed count beside the record count; an import's rejects are readable
  (CLI subcommand and API detail) well enough to diagnose the row; and
  `nigel status` plus the web dashboard's status data flag any account whose
  imports carry rejects, so incomplete books are visible where the user
  already looks (AC 52#1–3).

A refused zero-row import (§2) records nothing — rejects belong to an import
that exists; a refusal carries its reasons in the error message instead.

## Consequences elsewhere

- `web/` import screen: the confirm/preview error path already routes API
  errors; the history views gain the malformed column where record counts
  already show. Component-first applies to any visual change.
- TASK-117 (QFX/OFX importer) builds on this: a new parser writes rejects
  from day one instead of retrofitting.
- Undo/delete-import behavior is unchanged apart from the cascade.

## Testing

- Transaction seam: injected failure between import and categorize restores
  the exact before state (row counts and imports table compared).
- Zero-row: import under a wrong format → refusal with reasons, no imports
  row; corrected re-import succeeds on the same file (the TASK-50 repro,
  as a test).
- Rejects: a fixture statement with a mid-file format break imports the good
  rows, records the bad ones with reasons, shows both counts in history, and
  undo removes rejects with the import.
- Migration: an existing database migrates cleanly; old imports read as
  `malformed_count` 0 with no rejects.
- All fixtures from the fictional cast with invented amounts.

## Out of scope

- Any new importer format (TASK-117).
- Retrying or auto-repairing malformed rows.
- Snapshot mechanics (TASK-47 covers filename collisions).
