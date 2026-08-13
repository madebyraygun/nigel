---
id: TASK-9
title: 'Epic: double-entry foundations — account classification, then journal lines'
status: To Do
assignee: []
created_date: '2026-04-25 18:06'
updated_date: '2026-08-13 15:45'
labels:
  - epic
  - architecture
dependencies: []
references:
  - 'archived issue #81'
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Formerly a single task ("Journal entry layer (lightweight double-entry)"). Split into two, because the tax-package work in TASK-102 needs one half of it urgently and the other half not at all.

## Why the split

The original task bundled two changes of very different size and risk:

- **Account classification** (TASK-9.1) — every account and category carries an accounting class: asset, liability, equity, revenue, expense. This is what makes equity a first-class thing rather than an expense category with a "not deductible" note, and it is what TASK-102.2 (owner distributions) and TASK-102.1 (Schedule L) actually need. Additive, migratable, no change to how anyone imports a CSV.
- **Journal lines** (TASK-9.2) — every transaction generates balanced debit/credit pairs over a merged chart of accounts. This is the real general ledger, and it is a rewrite of the data layer touching `models.rs`, `migrations.rs`, `cli/mod.rs`, `main.rs`, every report, the TUI and the web API — on live books.

Three of the four benefits the original task claimed (trial balance, a real balance sheet, structurally correct equity) come from classification, not from journal lines. Journal lines buy structural *guarantee* — the books cannot silently fail to tie — which is worth having, but is not what the 1120-S is waiting on.

TASK-9.2 also still carries an unresolved design question (how `invoice_payments` maps to bank transactions) that its own text calls a prerequisite rather than an implementation detail. Sequencing a filing deadline behind an open architectural question is how March arrives with neither.

## Sequencing

TASK-9.1 lands inside the TASK-102 push, ahead of TASK-102.2. TASK-9.2 stays deferred with no date, and should be picked up when the invoicing reconciliation question is settled on its own merits — not as tax-season work.

The classification introduced in TASK-9.1 is designed to survive TASK-9.2 intact: when the tables merge, the classes come with them.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 TASK-9.1 is Done and TASK-102.2 is built on it rather than on a parallel category-type mechanism
- [ ] #2 TASK-9.2 remains open with its design question stated, and nothing in TASK-102 depends on it
<!-- AC:END -->
