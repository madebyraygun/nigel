---
id: TASK-9.8
title: Reports read from journal lines; cash basis stays the default
status: To Do
assignee: []
created_date: '2026-08-19 16:09'
updated_date: '2026-08-19 16:41'
labels:
  - architecture
  - reports
milestone: m-0
dependencies:
  - TASK-9.6
  - TASK-9.7
parent_task_id: TASK-9
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The cutover on the read side: every report (P&L, expenses, cash flow, balance, tax summary, K-1 worksheet, register) computes from journal lines instead of deriving from single-entry rows. This is what turns "the books tie" from a property each report proves separately into a property the reports inherit.

## Basis lives here, and only here

Cash basis remains the default and the only reporting mode v1 ships. The basis is decided **in the report layer** — which entries a report recognises — never below it (decision-5, invariant 1). Nothing in the recording layer branches on basis; that discipline is what makes accrual later a report toggle instead of a migration. Cash-basis recognition reads the bank — and invoicing stays off the ledger entirely in v1 (decision-6), so there are no claim legs for a report to exclude.

## Parity is the acceptance bar

Every report produces **identical figures before and after the migration** on a committed fixture — the same bar TASK-59 sets for the money-type change, applied to the ledger cutover. The fixture must include the hard cases: uncategorized transactions, transfers (the `EXCLUDE_TRANSFERS` semantics are preserved — money movement stays out of P&L, cash flow and `ytd_net_income`, and stays in the register and per-account balances), and the unpaid/part-paid invoices from the reconciliation fixture (which must move nothing).

The single-entry derivation paths are retired as each report moves — two implementations of one figure is how they drift.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every report computes from journal lines, and the single-entry derivation path each one replaced is removed
- [ ] #2 Every report produces identical figures before and after the migration on a committed fixture that includes uncategorized transactions, transfers, and unpaid/part-paid invoices
- [ ] #3 Cash basis is the default and only shipped reporting mode, decided in the report layer; nothing below the report layer branches on basis
- [ ] #4 Transfer semantics are preserved: money movement stays out of P&L, cash flow and ytd_net_income, and stays in the register and per-account balances
- [ ] #5 Update test coverage
- [ ] #6 Create or update documentation, making sure to remove any out of date information
- [ ] #7 All linting checks pass
- [ ] #8 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
