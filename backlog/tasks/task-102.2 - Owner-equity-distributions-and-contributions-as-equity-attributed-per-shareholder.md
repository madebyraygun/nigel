---
id: TASK-102.2
title: >-
  Owner equity: distributions and contributions as equity, attributed per
  shareholder
status: To Do
assignee: []
created_date: '2026-08-13 15:20'
updated_date: '2026-08-13 17:30'
labels:
  - tax
  - reports
dependencies:
  - TASK-9.1
parent_task_id: TASK-102
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`Owner Draw / Distribution` is seeded in `DEFAULT_CATEGORIES` as `category_type = "expense"` with `form_line = "K-16d"` and `tax_line = "Not deductible"`. It is not an expense — it is a reduction of equity. Consequences today:

- Any report that sums expenses by `category_type` overstates deductions by the distribution total, which is why TASK-27 has to special-case it.
- There is no owner *contribution* category at all, so money put into the business has nowhere correct to go.
- Distributions are not tracked per shareholder, so a K-1 split between two 50/50 owners is guesswork. The 2025 return needed the year's distribution total split between two shareholders.

## Scope, after the TASK-9 split

The mechanism for expressing equity now comes from **TASK-9.1** (account classification), which is a prerequisite. This task no longer introduces a category-type variant of its own — an earlier draft proposed exactly that, and it would have been migration debt the moment the classification landed.

What remains here is the tax-facing half:

- Seed `Owner Distribution` (`K-16d`) and `Owner Contribution` as equity-classed categories, and migrate the existing `Owner Draw / Distribution` category and its transactions onto them without data loss.
- Attribute distributions per shareholder, so the K-1 split is a figure rather than an assumption.

## Per-shareholder attribution

Two options, cheapest first:

- **Sub-categories per owner** — `Owner Distribution — <owner>`, one per shareholder. Zero schema change, works with the existing rules engine, ugly for a business with many owners.
- **A `shareholders` table with ownership percentages** and a nullable `shareholder_id` on transactions. Correct, enables a real K-1 split, and would let the K-1 worksheet allocate ordinary income by percentage too.

Recommend the second: TASK-102.3 (M-2 / AAA) is in the same push, and AAA and per-shareholder basis are close cousins. The first option is the fallback if this needs to ship alone.

Relevant code: `src/db.rs` (seed + migration), `src/reports.rs` (K-1 worksheet), `src/cli/category_manager.rs`.

Note that the report audit for equity handling — making sure no report counts an equity item as a deduction — belongs to TASK-9.1 and should not be duplicated here.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Owner distributions and contributions are seeded as equity-classed categories using the TASK-9.1 classification, not a mechanism specific to this task
- [ ] #2 Existing databases migrate the current `Owner Draw / Distribution` category and its transactions without data loss
- [ ] #3 Distributions can be attributed per shareholder, and the K-1 prep worksheet reports the per-shareholder total
- [ ] #4 The distribution total feeds Schedule K line 16d and the TASK-102.3 rollforward from one source, not two
- [ ] #5 Update test coverage
- [ ] #6 Create or update documentation, making sure to remove any out of date information
- [ ] #7 All linting checks pass
- [ ] #8 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
