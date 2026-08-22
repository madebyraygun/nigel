---
id: TASK-9.2
title: Journal entry layer (lightweight double-entry)
status: To Do
assignee: []
created_date: '2026-08-13 15:45'
updated_date: '2026-08-19 16:11'
labels:
  - enhancement
  - architecture
dependencies:
  - TASK-9.1
references:
  - 'archived issue #81'
parent_task_id: TASK-9
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Superseded. This task is decomposed into TASK-9.3 through TASK-9.11 under the TASK-9 epic, on the v1 milestone. The open design question this task flagged as a prerequisite — how invoice payments map to bank transactions — is settled in decision-6 (invoice payments post to accounts receivable; cash-basis reports read the bank), and TASK-9.7 implements it. The governing invariants are recorded in decision-5 and docs/design-constraints.md. Nothing remains here; the subtasks carry all the work and their own acceptance criteria.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The invoice-payment to bank-transaction mapping is decided and documented before schema work begins
- [ ] #2 Every transaction produces balanced journal lines over a merged chart of accounts, generated automatically from the existing workflow
- [ ] #3 Existing reports produce identical figures before and after the cutover on a real books fixture
- [ ] #4 No user-facing surface introduces debit/credit vocabulary
- [ ] #5 Update test coverage
- [ ] #6 Create or update documentation, making sure to remove any out of date information
- [ ] #7 All linting checks pass
- [ ] #8 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
