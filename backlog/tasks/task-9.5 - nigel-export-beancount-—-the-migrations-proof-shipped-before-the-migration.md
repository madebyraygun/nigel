---
id: TASK-9.5
title: 'nigel export beancount — the migration''s proof, shipped before the migration'
status: To Do
assignee: []
created_date: '2026-08-19 16:09'
updated_date: '2026-08-20 14:00'
labels:
  - enhancement
  - architecture
milestone: m-0
dependencies:
  - TASK-59
parent_task_id: TASK-9
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`nigel export beancount` renders the books as a Beancount file. This is not a nice-to-have interchange feature — **it is the verification strategy for the backfill migration**, which is why it ships before the migration exists and sits ahead of the rest of this epic on the v1 milestone.

## The proof

Export the books, run the migration, export again. Load both files into Beancount and compare the reports. Identical output is machine-verified proof that the migration preserved the books — on the operator's real data, run locally, with no figure entering this repository (the repo carries the same check as a fixture test). For that to work the exporter must run off **single-entry data**, before any journal table exists: each transaction becomes one Beancount transaction with two postings.

## Mapping

- The TASK-9.1 **class** picks the Beancount root on both legs — asset → `Assets:`, liability → `Liabilities:`, equity → `Equity:`, revenue → `Income:`, expense → `Expenses:`. One rule, one vocabulary. Mapping from `account_type`/`category_type` instead would resurrect the drift TASK-9.1 just killed: `Owner Draw / Distribution` carries `category_type = expense` but `class = equity`, and an exporter reading the type would file distributions under `Expenses:` — corrupting the very baseline the migration is verified against.
- Nigel's existing sign convention (negative = expense, positive = income) means the account leg exports verbatim and the category leg negated.
- The `reconciliations` table becomes Beancount `balance` assertions, which makes the exported file **self-verifying**: if the books drifted from the statements, Beancount refuses to load the file.

Output must be deterministic — two exports of unchanged books are byte-identical — because the whole point is comparing two exports.

Uncategorized transactions export against an explicit placeholder account rather than being dropped; a dropped row would make the before/after comparison lie.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 nigel export beancount renders the whole books from single-entry data, before any journal table exists, and the file loads cleanly in Beancount on the demo books
- [ ] #2 Rows in reconciliations export as Beancount balance assertions, so a drifted book refuses to load
- [ ] #3 Two exports of unchanged books are byte-identical, and uncategorized transactions export against an explicit placeholder account rather than being dropped
- [ ] #4 A fixture test compares the export against a known-good file
- [ ] #5 Update test coverage
- [ ] #6 Create or update documentation, making sure to remove any out of date information
- [ ] #7 All linting checks pass
- [ ] #8 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
- [ ] #9 The TASK-9.1 class picks the Beancount root on both legs (asset/liability/equity/revenue/expense → Assets:/Liabilities:/Equity:/Income:/Expenses:), and the existing sign convention exports the account leg verbatim with the category leg negated
<!-- AC:END -->
