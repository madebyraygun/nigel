---
id: TASK-9
title: 'Epic: cash-basis double entry on the v1 milestone'
status: To Do
assignee: []
created_date: '2026-04-25 18:06'
updated_date: '2026-08-21 00:21'
labels:
  - epic
  - architecture
milestone: m-0
dependencies: []
references:
  - 'archived issue #81'
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Nigel is adding double-entry bookkeeping underneath its single-entry surface, targeted at the **v1 milestone**. The point is credibility and structural correctness — a trial balance and a balance sheet that are read off a ledger rather than derived and separately proven — **without changing what a cash-basis user does or sees**. Two decisions govern every subtask: decision-5 (cash basis is a reporting concern and the second leg is always derived — both permanent invariants, recorded in `docs/design-constraints.md`) and decision-6 (invoicing stays off the ledger in v1 — no A/R account; recognition is the bank deposit, and the payment–deposit link is reconciliation, not posting).

## Scope

v1 is cash-basis double entry. Accounts payable, vendor bills, inventory and multi-currency are **deferred beyond v1, not rejected**, and the design keeps each cheap to add later: A/P is just another liability account on the TASK-9.1 classification when it comes; accrual is posting rules plus a report-basis toggle; multi-currency has its one hook (the `currency` column on journal lines, TASK-9.4); inventory deliberately gets no hook, because lots and cost basis are their own model and speculative schema is worse than none.

## Why now — the calculus changed

An earlier version of this epic kept TASK-9.2 deferred with no date and off the tax-season path. Two things changed that: invoicing shipped a second source of truth for client payments (`invoices`/`invoice_payments`, with no link to the register — the gap decision-6 closes), and every table added since — the asset register in TASK-102.5, the shareholders table in TASK-102.2 — raises the eventual migration cost. The work gets more expensive the longer it waits; v1 is the window.

## Order of work (the v1 milestone, in dependency order)

1. **TASK-59** — money to integer minor units (not started — first on this milestone). The money type is fixed before the ledger is built on it: double-entry books kept in floating point still fail to balance.
2. **TASK-50 / TASK-51 / TASK-52** — import integrity (in flight). The pipeline that feeds the ledger must not spend checksums, half-commit, or drop rows silently.
3. **TASK-9.1** — account classification (in flight). The class vocabulary the chart merge carries forward.
4. **TASK-9.5** — the Beancount exporter, **before** the migration it verifies: export, migrate, export again, load both in Beancount, compare reports. Identical output is machine-verified proof the backfill preserved the books.
5. The ledger proper, in dependency order: **TASK-9.3** (chart merge) → **TASK-9.4** (journal schema) → **TASK-9.6** (backfill) → **TASK-9.7** (invoicing reconciliation link) → **TASK-9.8** (reports read lines) → **TASK-9.9** (splits), **TASK-9.10** (transfers as one entry), **TASK-9.11** (trial balance). TASK-9.7 touches only the invoicing tables and is technically independent of the ledger; it is sequenced after the schema work deliberately, so the reconciliation surface and the invoice-bearing parity fixture land together.

TASK-9.2 is superseded: its work is decomposed into TASK-9.3 through TASK-9.11, and the design question it flagged as a prerequisite is settled in decision-6.

The classification introduced in TASK-9.1 survives the merger intact: when the tables merge in TASK-9.3, the classes come with them.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 TASK-9.1 is Done and TASK-102.2 is built on it rather than on a parallel category-type mechanism
- [ ] #2 TASK-9.3 through TASK-9.11 are Done in dependency order, with TASK-59, TASK-50, TASK-51 and TASK-52 landed first
- [ ] #3 The export-verify step ran: books exported before and after the backfill load in Beancount with identical reports — checked locally against the live books with no figure recorded in the repository, and pinned in-repo by the same check on a fixture
- [ ] #4 A cash-basis user's workflow is unchanged end to end: import, pick a category, reports default to cash basis, and no surface asks for two accounts or a direction
<!-- AC:END -->
