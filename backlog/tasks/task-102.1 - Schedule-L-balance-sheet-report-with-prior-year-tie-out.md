---
id: TASK-102.1
title: Schedule L balance sheet report with prior-year tie-out
status: To Do
assignee: []
created_date: '2026-08-13 15:20'
updated_date: '2026-08-13 17:30'
labels:
  - tax
  - reports
dependencies:
  - TASK-46
  - TASK-9.1
parent_task_id: TASK-102
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
1120-S Schedule L wants beginning-of-year and end-of-year figures side by side, and the beginning column must match the prior return's ending column exactly. Nigel can produce neither today: `nigel report balance` reports the position as of now, and accounts carry no opening balance, so any account whose history predates the first import is understated by its pre-import balance. For the 2025 filing both columns were reconstructed from BofA statement PDFs.

## Proposal

A `nigel report schedl --year <Y>` report with two columns — balance at `<Y-1>-12-31` and at `<Y>-12-31` — grouped into the Schedule L sections Nigel can actually populate:

- **Assets**: cash by account, other current assets
- **Liabilities**: credit cards, lines of credit, other current liabilities
- **Equity**: retained earnings as the balancing figure

Two things make it useful rather than decorative:

1. **Prior-year tie-out.** Store last year's filed ending figures (a small `prior_year_balances` table, or a `--prior <file>` CSV) and show a variance column. The credit-card discrepancy in the 2025 filing would have surfaced immediately instead of being spotted by eye.
2. **Account classification.** Schedule L groups by *kind* of account — assets, liabilities, equity — and drives sign conventions from it. That vocabulary comes from **TASK-9.1**, which is a prerequisite; this report classifies from it rather than inferring from `account_type` strings.

Fixed assets, accumulated depreciation, A/R, A/P and loan principal remain out of scope here — TASK-102.5 covers the asset side, and the rest stay manual with an explicit "not tracked" note in the report rather than a silent zero.

Relevant code: `src/reports.rs`, `src/cli/report/`, `src/db.rs` (accounts, new prior-year table + migration).

Coordinate with TASK-27 (trial balance): both need as-of-date balances from TASK-46 and both classify from TASK-9.1. Whichever starts first builds the shared primitive.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `nigel report schedl --year <Y>` shows beginning (Y-1 year end) and ending (Y year end) balances in adjacent columns, incorporating opening balances from TASK-46
- [ ] #2 Accounts are grouped into assets, liabilities and equity using the TASK-9.1 classification, with correct sign conventions for liability accounts
- [ ] #3 Prior-year filed figures can be recorded and a variance column flags any beginning balance that does not tie to them
- [ ] #4 Schedule L items Nigel does not track are named explicitly in the report rather than reported as zero
- [ ] #5 The report is available in the interactive viewer and exports to PDF, text and CSV like other reports
- [ ] #6 Update test coverage
- [ ] #7 Create or update documentation, making sure to remove any out of date information
- [ ] #8 All linting checks pass
- [ ] #9 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
