---
id: TASK-9.6
title: >-
  Backfill migration: every existing transaction becomes a balanced journal
  entry
status: To Do
assignee: []
created_date: '2026-08-19 16:09'
labels:
  - architecture
  - data-integrity
milestone: m-0
dependencies:
  - TASK-9.4
  - TASK-9.5
parent_task_id: TASK-9
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The backfill: every existing transaction becomes a balanced journal entry — the account leg from the transaction's account, the second leg from its category; uncategorized transactions post against an explicit uncategorized account so every entry balances from day one, and categorizing later moves the leg.

This is the highest-regret change on the board — a migration over live books — and it is deliberately scheduled **after** the Beancount exporter, because the exporter is its proof: export the books, run the migration, export again, load both files into Beancount and compare the reports. Identical output is machine-verified evidence the migration preserved the books, on real data rather than only on fixtures. The repository carries the same before/after check as a fixture test; the run against live books stays a local step and records no figures here.

## The risk to flag

Historical transfers are paired transactions tied only by the `Transfer` category convention. Collapsing a pair into one two-account entry requires heuristic matching on date and amount, and **a silent wrong match corrupts history** — two unrelated same-amount movements fused into a transfer that never happened. Therefore:

- Only exact, unambiguous pairs may be collapsed automatically, and the pairing is recorded so it can be inspected and undone.
- **Ambiguous candidates go to a review queue, never to automatic resolution.** Until reviewed, both rows stay as independent entries against the transfer account — correct, just uncollapsed — so the books never depend on a guess.

## Mechanics

The migration follows the standing rules: transactional (savepoint), aborts cleanly, appended to `MIGRATIONS` with a version bump. A pre-migration snapshot is taken the way imports take one, so the rollback story is the existing one.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 After migration every transaction has a balanced journal entry; uncategorized transactions post against an explicit uncategorized account
- [ ] #2 Only exact, unambiguous transfer pairs are collapsed automatically, the pairing is recorded and reversible, and ambiguous candidates land in a review queue — never resolved by heuristic alone
- [ ] #3 The Beancount export before and after the migration loads in Beancount and produces identical reports on a committed fixture; the same check against live books is a documented local step that records no figures in the repository
- [ ] #4 The migration is transactional, aborts cleanly, and a pre-migration snapshot is taken
- [ ] #5 Update test coverage
- [ ] #6 Create or update documentation, making sure to remove any out of date information
- [ ] #7 All linting checks pass
- [ ] #8 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
