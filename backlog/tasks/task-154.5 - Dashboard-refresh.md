---
id: TASK-154.5
title: Dashboard refresh
status: To Do
assignee: []
created_date: '2026-10-20 22:57'
labels:
  - web
  - design
milestone: m-3
dependencies:
  - TASK-154.1
references:
  - docs/design/app-refresh/README.md
parent_task_id: TASK-154
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Spec section 4: title row with the review pill, a three-cell stat strip (income and expenses year to date, net coloured by sign), the cash-flow card with paired monthly bars with rounded tops and a legend, and the balances card with a total. Coordinate with TASK-130 (A/R outstanding stat card), which adds to the same strip.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The Dashboard matches the spec in light and dark, and every figure equals what the existing dashboard and reports compute for the same books
- [ ] #2 Every new or changed @nigel/ui component has a preview covering its states in light and dark, and describePreviewA11y passes with zero violations
<!-- AC:END -->
