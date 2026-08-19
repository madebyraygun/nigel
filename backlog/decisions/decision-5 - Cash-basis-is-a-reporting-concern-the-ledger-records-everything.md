---
id: decision-5
title: Cash basis is a reporting concern; the ledger records everything
date: '2026-08-19 16:06'
status: accepted
---
## Context

Nigel is adding double-entry bookkeeping underneath its single-entry surface, targeted at
v1. The point is credibility and structural correctness — a trial balance and a balance
sheet that are *read off a ledger* rather than derived and separately proven correct —
without changing what a cash-basis user does or sees. TASK-9 carries the work.

The question that decides the whole shape of it is what "cash basis" means once a ledger
exists. If cash basis is a property of what gets *recorded*, then invoices, receivables
and anything else that is not yet money in the bank must stay outside the ledger, and
every future accrual feature is a schema migration. If cash basis is a property of what a
*report recognises*, the ledger can record everything and the report decides.

The precedent is settled practice, not invention: double-entry engines are routinely used
to keep cash-basis books — Beancount, ledger, GnuCash all do it — because the recording
model and the recognition basis are independent axes. A balanced ledger says the books
tie; the basis says which entries a P&L counts. Conflating them is how tools end up
either refusing cash-basis users or rewriting their schema for accrual.

## Decision

Two permanent invariants, then the v1 scope boundary.

**1. Cash basis is a reporting concern, not a recording concern.** The ledger records
everything; the report decides what to recognise. Cash basis stays the default and the
primary supported mode. This invariant is permanent — it is not a v1 simplification.

**2. The second leg is always derived, never asked for.** The account comes from the
import; the user picks a category, exactly as today. Any screen that asks a user to
choose two accounts and a direction has broken the product. Debit and credit vocabulary
stays out of every default surface. Also permanent — it extends TASK-9.1's key
constraint from classification to the ledger itself.

**v1 scope is cash-basis double entry.** Accounts payable, vendor bills, inventory, and
multi-currency are **deferred beyond v1, not rejected**. The architecture is chosen so
that adding them later is feature work rather than a migration:

- **Accrual** arrives for free from invariant 1: posting rules plus a report basis
  toggle, not a schema rewrite. Decision-6 already posts invoices accrual-shaped; only
  the report's recognition rule changes.
- **A/P** needs nothing now. When it comes, it is another liability account under the
  TASK-9.1 classification, exactly as A/R is an asset account.
- **Multi-currency** gets the one deliberate cheap hook: a `currency` column on journal
  lines, `NOT NULL`, defaulted to the book's currency, with v1 validation rejecting any
  other value. It is the single deferred feature whose absence from the schema would
  force a second ledger migration. No exchange rates, no conversion logic, no UI — the
  column and the constraint, nothing more.
- **Inventory** gets no hook, on purpose. Lots and cost basis are their own model, and
  speculative schema is worse than none.

## Consequences

- Both invariants are recorded in `docs/design-constraints.md`, and every TASK-9 subtask
  is written against them. A subtask that needs to break one is a change to this
  decision, argued here first.
- Cash-basis reports must produce identical figures before and after the cutover, pinned
  by a parity test on a committed fixture. The Beancount export is the proof on real
  books: export, migrate, export again, compare the loaded reports — the step is the
  acceptance criterion, never the operator's figures.
- Nothing below the report layer may branch on basis. A recording-layer "cash mode"
  would be exactly the conflation this decision exists to prevent, and it is what would
  turn the accrual toggle back into a migration.
- Decision-6 is where invariant 1 is most at risk, because invoicing is the one place
  Nigel already records money that has not arrived. Its posting rule is chosen so that
  an unpaid invoice can never appear in cash-basis income.
