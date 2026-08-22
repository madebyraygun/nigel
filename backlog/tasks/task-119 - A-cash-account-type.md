---
id: TASK-119
title: A cash account type
status: To Do
assignee: []
created_date: '2026-08-19 17:04'
updated_date: '2026-08-21 00:21'
labels:
  - enhancement
milestone: m-0
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Every account in Nigel is a bank product: `ACCOUNT_TYPES` is `checking`, `credit_card`, `line_of_credit`, `payroll` (`crates/nigel-core/src/accounts.rs`). A business that takes or spends actual cash has no account to put it in, so the money either goes unrecorded or gets forced into a fake checking account. For a product whose primary audience is small cash-basis businesses, cash is a first-class kind of account, not an edge case.

## Proposal

Add `cash` to the account-type vocabulary. Asset class — TASK-9.1's account-type derivation already lands unfamiliar bank-product types on asset by default, and this task adds the explicit cash arm to that derivation, so classification costs one match arm and nothing gets duplicated. Otherwise it is an **ordinary account**: it appears in the register, in every report, in per-account balances, and in reconciliation exactly like the others. The deliberate work of this task is confirming that ordinariness rather than building anything around it:

- **Reconciliation needs nothing.** `nigel reconcile` takes a statement balance and compares it to the calculated balance from transactions (`reconciler.rs` is pure account/date sums, no import involvement). For a cash account the "statement" is the drawer count. Confirm it works unchanged with a test; do not add a till workflow, a float, or a count screen.
- **No importer variant.** `ImporterKind` variants exist per bank statement format, and cash has no statements. The generic CSV path should still accept a cash account for anyone keeping a spreadsheet — confirm, don't build.
- **Type appears wherever accounts are created or edited**: the CLI's `accounts add` validation and help text, the TUI account manager, the web account form's type picker.

The point of a cash account is being able to type into it — that is its own task (manual register entries) and neither task depends on the other: a cash account is useful the day the entry surface lands, and manual entries work on any account from day one.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 cash is an accepted account type wherever accounts are created or edited — CLI validation and help, TUI account manager, web account form — and rejects nothing else that works today
- [ ] #2 A cash account appears in the register, all reports and per-account balances like any other account; a fixture test pins the figures
- [ ] #3 nigel reconcile works unchanged on a cash account against a drawer-count balance — a test confirms it, and no till workflow, float or count screen is added
- [ ] #4 Generic CSV import into a cash account works; no dedicated importer variant is added
- [ ] #5 Classification comes from the TASK-9.1 vocabulary (cash → asset) once both land; nothing here duplicates the classification mechanism
- [ ] #6 Update test coverage
- [ ] #7 Create or update documentation, making sure to remove any out of date information
- [ ] #8 All linting checks pass
- [ ] #9 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
