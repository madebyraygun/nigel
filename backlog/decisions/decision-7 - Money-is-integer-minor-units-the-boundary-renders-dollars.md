---
id: decision-7
title: Money is integer minor units; the boundary renders dollars
date: '2026-08-20 14:09'
status: accepted
---
## Context

Amounts are `f64` end to end — `transactions.amount REAL` in the schema, `f64` fields
through `models.rs`, float arithmetic in the reconciler and every report. TASK-59 filed
the standing symptom: `reconciler::is_reconciled` is computed from the unrounded
discrepancy while the serialized discrepancy is rounded to cents, so a 0.014 difference
reports `discrepancy: 0.01` beside `isReconciled: false` — the same number disagreeing
with itself in one JSON object. The Intl-vs-Rust rounding divergence fixed during the
web-UI epic was the identical root cause seen from the display end: two languages
rounding the same double differently.

The double-entry epic (TASK-9) turned this from a debt into a blocker: journal lines
must sum to exactly zero, and a sum-to-zero invariant is only meaningful in integers.
TASK-9.4 refuses to build on `f64`, which is why TASK-59 is first on the v1 milestone.

## Decision

**Amounts are integer minor units — cents, `i64` — everywhere inside the system.
Dollars exist only at the boundary, as rendering.**

- **Storage:** amount columns become `INTEGER` (a migration converts by rounding to
  cents, which is the rounding imports already perform conceptually; the register's
  figures do not change).
- **Domain:** a `Money(i64)` newtype in `nigel-core`. Arithmetic is integer arithmetic;
  comparison is integer comparison; there is no tolerance because there is no drift.
  `is_reconciled` and the serialized discrepancy become the same value structurally —
  the TASK-59 symptom class dies rather than the instance.
- **Parsing:** importers parse statement strings straight to cents. No float transits
  between the file and the database.
- **The boundary renders dollars:** the JSON API keeps serializing decimal-dollar
  numbers (`1250.01`), text/PDF reports keep their formatting, and the SPA changes
  nothing. Cents-to-dollars at serialization is one division at one boundary, and
  report output is pinned byte-for-byte across the conversion (TASK-59 AC#3).
- **No currency awareness here.** `Money` is an amount, not a denominated value;
  the `currency` column and its constraint arrive on journal lines in TASK-9.4
  (decision-5's single multi-currency hook). This decision deliberately does not
  front-run it.

## Consequences

- The conversion is one dedicated branch touching most of the data layer — models,
  importers, invoicing amounts, reconciler, reports, and a schema migration — and per
  the task's own filing it is never bolted onto a feature branch.
- **Sequencing:** implementation starts after the in-flight branches land
  (import-integrity and account-classification both rewrote the files this conversion
  edits; the onboarding branch owns the demo fixture tables). Its migration takes the
  next number after theirs.
- The parity gate is absolute: every report on the committed fixtures byte-identical
  before and after, and the reconciler pairing test asserts the two serialized values
  agree at the tolerance edge that today disagrees.
- TASK-9.4's balance enforcement builds on `Money` directly; nothing in the ledger
  ever sees a float.
