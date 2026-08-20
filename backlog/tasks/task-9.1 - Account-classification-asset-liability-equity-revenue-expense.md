---
id: TASK-9.1
title: 'Account classification: asset, liability, equity, revenue, expense'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-13 15:45'
updated_date: '2026-08-20 14:19'
labels:
  - architecture
  - tax
milestone: m-0
dependencies: []
parent_task_id: TASK-9
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Nigel has two parallel vocabularies for what a thing is. Bank accounts carry `account_type` (checking, credit_card, line_of_credit, payroll) and categories carry `category_type` (income, expense). Neither can express equity, neither agrees with the other, and every report that needs to know whether something is an asset or a liability infers it from the account type string.

The consequences show up wherever accounting structure is needed:

- Owner distributions are seeded as `category_type = "expense"` with a `"Not deductible"` note, so any report summing expenses overstates deductions until it special-cases them by name (TASK-27 has to; the K-1 worksheet has to).
- There is no owner contribution category at all — money put into the business has nowhere correct to go.
- A Schedule L balance sheet (TASK-102.1) has to group accounts into assets, liabilities and equity, and there is no field that says which is which.

## Proposal

Introduce a single accounting class vocabulary — `asset`, `liability`, `equity`, `revenue`, `expense` — applied to both accounts and categories, and make every report classify from it rather than from account-type strings or category names.

**Deliberately a classification change, not a table merger.** Categories and accounts stay in their own tables; they simply gain a shared class. The full promotion of categories into a unified chart of accounts belongs with TASK-9.2, where the journal lines that need it live. This keeps the change additive and migratable, and the classes survive the merger intact when it comes.

Migration backfills existing data: checking/savings → asset, credit_card/line_of_credit → liability, income categories → revenue, expense categories → expense, and the seeded `Owner Draw / Distribution` category → equity. Nothing needs re-categorizing by hand. A `cash` account type (TASK-119) never passes through this backfill — the migration only sees types that existed before it ran; cash lands on asset at creation time through the account-type derivation, where TASK-119 adds its explicit arm.

## The trap to watch for

Everything that currently branches on two variants now has five. An unhandled class falling into an `else` and being counted as an expense is the failure mode — it is how distributions came to be reported as deductions in the first place. Prefer exhaustive matching over a catch-all wherever the compiler can be made to help; see TASK-60 (closed-set string fields should be enums), which this task is a natural companion to and could partly subsume.

Surfaces needing an audit: `src/reports.rs` (P&L, expenses, tax summary, K-1, cash flow, balance), `src/cli/category_manager.rs`, `src/cli/accounts.rs`, `src/server/routes/categories.rs`, the TUI category and account screens, and the web UI equivalents.

## Alternative considered

Full table unification now — categories become accounts, one table, one type field. Structurally cleaner and removes the eventual migration, but it is most of TASK-9.2's blast radius without TASK-9.2's benefit, and it would put a data-layer rewrite in front of the filing work. Rejected for this window; revisit with TASK-9.2.

## Key constraint

Unchanged from the original task: a freelancer importing a bank CSV should never see the word "debit" unless they go looking for it. Classification is structure, not vocabulary — the user-facing labels stay as they are.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A single accounting class vocabulary (asset, liability, equity, revenue, expense) applies to both accounts and categories, stored as a closed set rather than free text
- [ ] #2 Migration backfills every existing account and category, including the seeded owner distribution category as equity, with no manual re-categorization
- [ ] #3 Every report classifies from the new class rather than from account-type strings or category-name checks; equity is excluded from deductions everywhere
- [ ] #4 Liability accounts carry a correct sign convention that the balance and Schedule L reports can rely on
- [ ] #5 Class is settable and visible wherever accounts and categories are created or edited (CLI, TUI, web UI)
- [ ] #6 No user-facing surface introduces debit/credit vocabulary
- [ ] #7 Update test coverage, including a test that a new class cannot be silently absorbed into expense totals
- [ ] #8 Create or update documentation, making sure to remove any out of date information
- [ ] #9 All linting checks pass
- [ ] #10 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
