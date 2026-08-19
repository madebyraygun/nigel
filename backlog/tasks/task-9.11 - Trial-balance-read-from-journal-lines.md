---
id: TASK-9.11
title: Trial balance read from journal lines
status: To Do
assignee: []
created_date: '2026-08-19 16:10'
updated_date: '2026-08-19 16:41'
labels:
  - reports
milestone: m-0
dependencies:
  - TASK-9.8
parent_task_id: TASK-9
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The trial balance becomes a read: sum journal lines per account, print debit and credit columns, and the total ties to zero **by construction** — TASK-9.4's balance invariant means there is nothing to derive and nothing to prove per report.

## Relationship to TASK-27

TASK-27 derived a trial balance from single-entry data, and its own text said that if journal lines land, "this report should be reimplemented as a straight query over journal lines and its derivation logic dropped." This task **is** that report, and TASK-27 is closed as superseded by it. What carries over from TASK-27 rather than being dropped:

- The output contract: one row per account with separate Debit and Credit columns, `--format csv` producing an Account/Debit/Credit file that tax software (TaxAct Business) accepts without hand-editing.
- The reporting-year framing: balance-sheet rows as of the last day of the reporting year, income and expense rows for the year.
- The warnings: uncategorized transactions on or before year end are named (they sit in the uncategorized account, so the report should say so rather than tie quietly around them).

No pre-ledger stopgap ships: the operator closed TASK-27 rather than keeping it for the filing window, so no derivation logic is written at all. The as-of-date balance primitive is shared with TASK-46 and TASK-102.1 — whichever of this task and TASK-102.1 starts first builds it.

On vocabulary: the standing constraint is that nobody sees the word "debit" *unless they go looking for it* — and asking for a trial balance is going looking. Debit/Credit columns are correct here; every default surface keeps decision-5's invariant 2.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 nigel report trialbalance computes from journal lines only, and total debits equal total credits by construction, asserted on a fixture that includes uncategorized transactions, splits and transfers
- [ ] #2 One row per account with separate Debit and Credit columns; balance-sheet rows are as of the reporting year end and income/expense rows cover the year
- [ ] #3 --format csv produces an Account/Debit/Credit file that TaxAct Business accepts without manual editing, honouring TASK-27's column contract
- [ ] #4 Uncategorized transactions on or before year end are named in the report rather than tied around quietly
- [ ] #5 The report is available in the interactive viewer and exports like other reports
- [ ] #6 Update test coverage
- [ ] #7 Create or update documentation, making sure to remove any out of date information
- [ ] #8 All linting checks pass
- [ ] #9 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
