---
id: TASK-102.4
title: 'Nondeductible expenses: penalties, fines and the meals disallowance'
status: To Do
assignee: []
created_date: '2026-08-13 15:20'
labels:
  - tax
  - reports
dependencies: []
parent_task_id: TASK-102
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Schedule K line 16c wants the total of expenses that reduce AAA but are not deductible. Nigel has no concept of one. Two cases came up in the 2025 filing:

1. **Meals.** The K-1 worksheet already applies the 50% limit, but the disallowed half is never reported as a line 16c figure — it just vanishes between the gross and deductible columns. TASK-25 notes the presentation inconsistency; this task is the other half of it.
2. **Penalties.** A CARES-era payroll tax deferral was paid off with penalty and interest as a single entry in `Taxes & Licenses`. The tax and interest are deductible; the penalty is not. Splitting it required going back to an IRS notice, and the whole line had to be adjusted downward by hand with the penalty re-entered on line 16c.

## Proposal

A `deductibility` attribute on categories with three states — `full`, `partial(percent)`, `none` — replacing the hard-coded meals rule with something general. Then:

- Seed `Penalties & Fines` as `none`, mapped to `K-16c`.
- Set `Meals` to `partial(50)`, and have the K-1 worksheet derive both the deductible half and the line 16c disallowance from that attribute rather than from a literal category-name check.
- The tax summary and K-1 worksheet gain a **Nondeductible expenses (line 16c)** total combining `none` categories and the disallowed portion of `partial` ones.

Two design notes worth arguing about before implementation:

- **Per-transaction override.** The payroll-deferral payoff was one payment containing deductible tax, deductible interest, and a nondeductible penalty. A category-level attribute cannot split it; that entry has to be recorded as separate transactions, which cash-basis single-entry supports fine but which the importer will not do automatically. Document the split-the-payment workflow rather than building per-transaction deductibility.
- **The percentage is not permanent.** The meals limit has been 0%, 50% and 100% within recent memory. Storing the percentage per category (and ideally allowing a per-year value) avoids another hard-coded rule to chase.

Relevant code: `src/db.rs` (categories schema + seed + migration), `src/models.rs`, `src/reports.rs` (K-1 worksheet, tax summary), `src/cli/category_manager.rs`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Categories carry a deductibility attribute (full / partial with percentage / none), editable from the CLI, TUI and web UI
- [ ] #2 The 50% meals limit is derived from that attribute rather than from a category-name check, with identical results on existing books
- [ ] #3 A `Penalties & Fines` category is seeded as nondeductible and mapped to K-16c; existing databases pick it up via migration
- [ ] #4 The K-1 worksheet and tax summary report a nondeductible expenses total for Schedule K line 16c, itemised by source
- [ ] #5 Documentation covers splitting a mixed payment (tax / interest / penalty) into separate transactions
- [ ] #6 Update test coverage
- [ ] #7 Create or update documentation, making sure to remove any out of date information
- [ ] #8 All linting checks pass
- [ ] #9 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
