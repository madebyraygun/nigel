---
id: TASK-171
title: Importers for BofA consumer credit cards and Citi cards
status: To Do
assignee: []
created_date: '2026-11-09 22:22'
labels:
  - importer
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Two common card exports need hand conversion today:

- **BofA consumer card**: `Posted Date,Reference Number,Payee,Address,Amount`, signed amounts. A different layout from the business card format `bofa_credit_card` reads.
- **Citi**: `Status,Date,Description,Debit,Credit,Member Name`. Amounts are unquoted with thousands separators, so `$ 2,500.00` splits into two CSV fields; debit and credit are separate columns and credits carry a leading `-$`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Both formats are detected from their headers and import with correct signs (charges negative, payments and credits positive)
- [ ] #2 The Citi parser rebuilds amounts split by an unquoted thousands separator; a fixture covers debit and credit values of $1,000 or more
- [ ] #3 Pending rows (Citi Status other than Cleared) are skipped and reported, not imported
- [ ] #4 BofA consumer Reference Number, when present, feeds external-ID dedup once TASK-168 lands
- [ ] #5 docs/importers.md lists both formats, with fictional fixtures
<!-- AC:END -->
