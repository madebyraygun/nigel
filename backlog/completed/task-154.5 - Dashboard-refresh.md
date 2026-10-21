---
id: TASK-154.5
title: Dashboard refresh
status: Done
assignee: []
created_date: '2026-10-20 22:57'
updated_date: '2026-10-21 20:08'
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
- [x] #1 The Dashboard matches the spec in light and dark, and every figure equals what the existing dashboard and reports compute for the same books
- [x] #2 Every new or changed @nigel/ui component has a preview covering its states in light and dark, and describePreviewA11y passes with zero violations
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in PR #64 (bar tops 7px per the user).
<!-- SECTION:FINAL_SUMMARY:END -->
