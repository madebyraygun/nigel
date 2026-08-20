---
id: TASK-9.10
title: >-
  Transfers as one entry: two account legs, retiring the paired-transaction
  model
status: To Do
assignee: []
created_date: '2026-08-19 16:10'
labels:
  - enhancement
  - architecture
milestone: m-0
dependencies:
  - TASK-9.6
  - TASK-9.8
parent_task_id: TASK-9
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A transfer is one movement of money between two accounts, and under the ledger it becomes **one entry with two account legs** — retiring the paired-transaction model, where a transfer is two rows in the `Transfer` category tied together by nothing but convention.

What the pairing convention costs today:

- Nothing enforces that the pair exists, matches, or stays matched; `EXCLUDE_TRANSFERS` filters by category and hopes.
- The Beancount export is **lossy** on transfers: with no recorded pairing, the exporter cannot say which outflow matches which inflow, so a transfer exports as two independent transactions against a transfer placeholder instead of one `Assets:A` → `Assets:B` posting. A single-entry books cannot fix this; a one-entry transfer exports verbatim.

## Shape

- Recording a transfer (and the review flow's transfer categorization) produces one balanced entry with two account legs and no category leg. Being money movement with no income/expense leg, it is **structurally** outside the P&L, cash flow and `ytd_net_income` — the `EXCLUDE_TRANSFERS` special case becomes a fact of the data rather than a filter.
- Each account's register still shows its side of the movement, because per account the cash really moved — the existing rule, kept.
- Historical pairs are collapsed by the backfill migration under its review-queue discipline; this task owns the go-forward recording, the surfaces, and the export fix.
- No default surface asks for a direction; a transfer is "money moved from A to B", stated once.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Recording or categorizing a transfer produces one balanced entry with two account legs and no category leg
- [ ] #2 Transfers are structurally outside P&L, cash flow and ytd_net_income — no category filter involved — while each account's register still shows its side of the movement
- [ ] #3 The Beancount export emits one transaction per post-migration transfer, with no transfer placeholder artifacts
- [ ] #4 No default surface asks for a direction; a transfer is stated once as money moved from one account to another
- [ ] #5 Update test coverage
- [ ] #6 Create or update documentation, making sure to remove any out of date information
- [ ] #7 All linting checks pass
- [ ] #8 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
