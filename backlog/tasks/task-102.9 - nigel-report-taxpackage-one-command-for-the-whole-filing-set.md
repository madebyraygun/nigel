---
id: TASK-102.9
title: '`nigel report taxpackage` — one command for the whole filing set'
status: To Do
assignee: []
created_date: '2026-08-13 15:20'
labels:
  - tax
  - reports
dependencies:
  - TASK-102.1
  - TASK-102.2
  - TASK-102.3
  - TASK-102.4
  - TASK-102.5
  - TASK-102.6
  - TASK-102.7
parent_task_id: TASK-102
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The capstone. Once the sibling tasks land, the 1120-S filing set is a collection of separate reports that still have to be run individually and assembled by hand. `nigel report taxpackage --year <Y>` runs the lot into a single dated bundle:

1. P&L mapped to 1120-S lines (K-1 prep worksheet)
2. Schedule L balance sheet with prior-year tie-out
3. Schedule M-2 / AAA rollforward
4. Officer compensation vs. employee wages
5. Fixed asset register and §179 detail for Form 4562
6. Tax payments ledger
7. 1099 / contractor roster with documentation status
8. Per-shareholder K-1 summary

`nigel report all` already exports every report; this differs in three ways worth building rather than folding into it:

- **Filing-ordered, not alphabetical.** The bundle follows the order the return asks for the figures.
- **A pre-flight page.** Every warning in one place before any number is trusted: uncategorized transactions, unmapped categories, accounts with no opening balance, equipment purchases with no asset record, payees over threshold with no W-9, an unreconciled month. If the pre-flight is clean the numbers can be typed in; if it is not, the page says what to fix first.
- **A stable, dated output directory.** `~/Documents/nigel/exports/tax-2025/` with predictable filenames, so a filing set can be archived alongside the return PDFs and diffed against a later re-run if a figure is ever questioned.

Formats: PDF for the human, CSV for anything a tax package might ingest, text for diffing between runs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `nigel report taxpackage --year <Y>` produces all filing reports in one run, in filing order, into a dated output directory
- [ ] #2 A pre-flight summary leads the bundle, collecting every data-quality warning across the constituent reports
- [ ] #3 The command exits non-zero (or prints a clear top-level warning) when the pre-flight finds blocking issues
- [ ] #4 PDF, CSV and text output are all supported, with stable filenames suitable for archiving and diffing
- [ ] #5 Re-running for the same year is idempotent or clearly versioned, not silently overwritten
- [ ] #6 Update test coverage
- [ ] #7 Create or update documentation, making sure to remove any out of date information
- [ ] #8 All linting checks pass
- [ ] #9 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
