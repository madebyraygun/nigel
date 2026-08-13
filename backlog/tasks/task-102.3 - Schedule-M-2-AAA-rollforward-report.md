---
id: TASK-102.3
title: Schedule M-2 AAA rollforward report
status: To Do
assignee: []
created_date: '2026-08-13 15:20'
labels:
  - tax
  - reports
dependencies:
  - TASK-102.1
  - TASK-102.2
  - TASK-102.4
parent_task_id: TASK-102
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Schedule M-2 tracks the Accumulated Adjustments Account across the year:

    beginning AAA
      + ordinary business income
      + separately stated income items
      − nondeductible expenses
      − deductions (including §179)
      − distributions
      = ending AAA

For the 2025 return this was done on paper. Every input except beginning AAA already exists in the app or lands with sibling tasks in this epic — ordinary business income from the K-1 worksheet, nondeductibles from TASK-102.4, §179 from TASK-102.5, distributions from TASK-102.2. Only beginning AAA is genuinely external, taken from the prior return's ending AAA.

## Proposal

`nigel report m2 --year <Y>` renders the rollforward as an ordered list of adjustments with a running balance, the way the form reads. Beginning AAA is stored per year — either alongside the prior-year balance data from TASK-102.1 or as its own `nigel tax set-aaa --year 2026 --amount 38413.00` — and after the first year it can be defaulted from the previous year's computed ending AAA, with an explicit note that the figure is derived rather than filed.

Schedule M-1 (book-to-tax reconciliation) is the natural companion and, on cash-basis books with nondeductibles tracked, is short enough to include in the same report. Worth a look during design; not required by these acceptance criteria.

The value is not the arithmetic — it is that each line names the transactions behind it, so a figure that looks wrong can be traced in one step instead of being re-derived.

Relevant code: `src/reports.rs`, `src/cli/report/`, `src/db.rs` (per-year AAA storage + migration).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `nigel report m2 --year <Y>` renders the AAA rollforward in Schedule M-2 order with a running balance and an ending AAA
- [ ] #2 Beginning AAA can be recorded per year, and after the first year defaults to the prior year's computed ending AAA with the derivation noted
- [ ] #3 Each adjustment line reports the categories and totals it was built from, so any figure can be traced back to transactions
- [ ] #4 The report warns when an input is unavailable (no nondeductible tracking, no §179 register, uncategorized transactions) instead of reporting a confidently wrong ending AAA
- [ ] #5 Available in the viewer and exportable to PDF, text and CSV
- [ ] #6 Update test coverage
- [ ] #7 Create or update documentation, making sure to remove any out of date information
- [ ] #8 All linting checks pass
- [ ] #9 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
