---
id: TASK-154.7
title: 'Invoices refresh: list and detail'
status: Done
assignee: []
created_date: '2026-10-20 22:57'
updated_date: '2026-10-23 00:54'
labels:
  - web
  - design
  - invoicing
milestone: m-3
dependencies:
  - TASK-154.1
references:
  - docs/design/app-refresh/README.md
parent_task_id: TASK-154
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Spec section 6. List: title row with Sync now and a filled New invoice, the Outstanding card with the stacked aging bar and five-bucket legend, status chips with counts, and the table with status pills (void rows struck through). Detail: title with status pill, action pills with the spec's enablement rules, a four-column fact strip, line items, and Payments and Published cards side by side. Routes #/invoices/{n}. Keep every existing action and its guardrails.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 List and detail match the spec in light and dark; aging bucket totals equal nigel invoice aging for the same books
- [x] #2 Action enablement follows the spec and the existing server guards, with disabled actions explaining why (TASK-148)
- [x] #3 Every new or changed @nigel/ui component has a preview covering its states in light and dark, and describePreviewA11y passes with zero violations
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in PR #67 (Invoices refresh, with TASK-148 and TASK-141 folded in).
<!-- SECTION:FINAL_SUMMARY:END -->
