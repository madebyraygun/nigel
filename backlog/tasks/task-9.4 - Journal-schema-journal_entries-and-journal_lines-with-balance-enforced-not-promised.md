---
id: TASK-9.4
title: >-
  Journal schema: journal_entries and journal_lines with balance enforced, not
  promised
status: To Do
assignee: []
created_date: '2026-08-19 16:09'
labels:
  - architecture
milestone: m-0
dependencies:
  - TASK-59
  - TASK-9.3
parent_task_id: TASK-9
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The ledger proper: `journal_entries` and `journal_lines` over the merged chart of accounts (TASK-9.3). Every transaction generates one entry whose lines balance; the account leg comes from the import, the category leg from the user's choice, exactly as today (decision-5, invariant 2).

## Balance is enforced, not promised

An entry that does not sum to zero must be **unrepresentable**, by the database or by a repository invariant that cannot be bypassed — never by convention. Concretely: lines are only writable through a repository API that takes a whole balanced entry, and the database carries a backstop (trigger or equivalent) so that even raw SQL cannot commit an unbalanced entry. A test proves both — the API refuses, and the backstop refuses a write that goes around the API.

## Depends on TASK-59: integer minor units, never `f64`

Double-entry books kept in floating point still fail to balance — the sum-to-zero invariant is only meaningful in integers. Line amounts are integer minor units, full stop. This is why TASK-59 is first on the v1 milestone.

## One forward-compatibility hook, and only one

A `currency` column on journal lines: `NOT NULL`, defaulted to the book's currency, with v1 validation rejecting any other value. Single-currency behaviour is unchanged; what it buys is that multi-currency later is feature work instead of a second ledger migration (decision-5). No exchange rates, no conversion logic, no UI — the column and the constraint, nothing more. No other speculative columns: A/P needs nothing and inventory deliberately gets no hook.

Debit/credit vocabulary stays internal to the schema and repository; no default surface prints it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 journal_entries and journal_lines exist over the merged chart of accounts, with line amounts in integer minor units — never f64
- [ ] #2 An unbalanced entry is unrepresentable: the repository API refuses it, a database-level backstop refuses a write that bypasses the API, and a test proves both
- [ ] #3 journal_lines carries a currency column, NOT NULL, defaulted to the book's currency, and v1 validation rejects any other value; no exchange rates, conversion logic or UI exist
- [ ] #4 No user-facing surface introduces debit/credit vocabulary
- [ ] #5 Update test coverage
- [ ] #6 Create or update documentation, making sure to remove any out of date information
- [ ] #7 All linting checks pass
- [ ] #8 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
