---
id: TASK-102.5
title: Fixed asset register for Section 179 and Form 4562
status: To Do
assignee: []
created_date: '2026-08-13 15:20'
labels:
  - tax
  - reports
dependencies: []
parent_task_id: TASK-102
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Form 4562 wants, per asset: description, cost, and date placed in service. Nigel records the payment date and a description like `APPLE STORE #R123`, which is neither. For the 2025 return the placed-in-service date for a MacBook Pro came from an Apple order confirmation email found by searching Gmail — the purchase date and the in-service date happened to differ, and only the latter is what the form asks for.

This is deliberately **not** a depreciation engine. Nigel is cash-basis single-entry and has no journal entries or contra-asset accounts; formal fixed-asset accounting does not belong here. What is needed is a register: the handful of facts the form asks for, attached to transactions that already exist.

## Proposal

An `assets` table — description, cost, acquired date, placed-in-service date, category, method (`section_179` / `bonus` / `none`), disposal date, and a nullable link to the originating transaction — with a small CLI:

```
nigel assets add --transaction 412 --description "MacBook Pro 16" --in-service 2025-09-03 --method section_179
nigel assets list --year 2025
nigel assets dispose 3 --date 2026-04-01
```

Then `nigel report assets --year <Y>` renders the Form 4562 Part I detail and a §179 total that feeds the K-1 worksheet (Schedule K line 11) and TASK-102.3's rollforward.

Two things worth building in, both learned the hard way:

- **Method matters for state conformity.** California conforms to §179 but not to federal bonus depreciation, which is why §179 was elected for the 2025 purchase. Recording the method per asset — and noting the state-conformity consequence in the report — keeps that decision visible next year instead of buried in a chat log.
- **A year-end prompt.** A report flagging transactions over a configurable threshold in equipment-ish categories that have no asset record, so tangible property purchases get a decision rather than being silently expensed. Threshold in settings, defaulting to something sensible like $2,500 (the de minimis safe harbour).

Relevant code: `src/db.rs` (new table + migration), new `src/cli/assets.rs`, `src/reports.rs`, TUI/web surfaces can follow later.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Assets can be recorded with description, cost, acquired and placed-in-service dates, method and optional link to the originating transaction
- [ ] #2 `nigel report assets --year <Y>` produces the per-asset detail Form 4562 Part I asks for, plus a §179 total for the year
- [ ] #3 The §179 total flows into the K-1 prep worksheet (Schedule K line 11) rather than being entered separately
- [ ] #4 A year-end review flags transactions above a configurable threshold in equipment categories that have no asset record
- [ ] #5 The report notes where the elected method has state-conformity consequences (§179 vs bonus depreciation)
- [ ] #6 Update test coverage
- [ ] #7 Create or update documentation, making sure to remove any out of date information
- [ ] #8 All linting checks pass
- [ ] #9 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
