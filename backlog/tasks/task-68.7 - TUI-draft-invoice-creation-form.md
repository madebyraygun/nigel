---
id: TASK-68.7
title: 'TUI: draft-invoice creation form'
status: Done
assignee: []
created_date: '2026-08-08 01:02'
updated_date: '2026-08-11 04:16'
labels:
  - invoicing
  - tui
dependencies: []
parent_task_id: TASK-68
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
68.4 ships the invoice screens without creation — the empty state points at `nigel invoice new`. This subtask adds a draft form to the invoice manager: client selector, issue/due dates (validate_date), currency, and repeatable line-item rows (the hardest TUI form in the app — study how import_manager and reconcile_manager handle multi-field forms, and design row add/remove/edit keys). Creates through invoicing::create_invoice; drafts only — send stays a separate deliberate action. Depends on 68.1 (validation) and 68.4 (the screens it extends).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A draft invoice can be created entirely from the TUI, including multiple line items
- [x] #2 Validation failures render beside the field with the CLI's wording
- [x] #3 The invoice-list empty state stops pointing at the CLI once the form exists
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From 68.4: Demo data loaded!
  Account:      BofA Checking
  Transactions: 270
  Rules:        10
  Categorized:  186
  Flagged:      84

Try these next:
  nigel accounts list
  nigel rules list
  nigel report pnl
  nigel report flagged
  nigel review seeds no clients or invoices, so the TUI invoice/client screens open empty on a demo database. Seeding a few demo clients/invoices belongs here alongside the creation form.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
PR #195 merged. TUI draft-invoice form (a/n from the invoice list): client selector, dates, currency, repeatable line-item rows (Ins/F2 add, Del/F3 remove), per-field validation attribution with create_invoice as the sole writer, error scrolled into view (regression-pinned). Item table widths fitted as one unit — description yields, figures never truncate. nigel demo seeds 3 clients + 4 invoices (derived statuses, no payment links) behind a per-half idempotency guard that recovers a partial seed. Review round fixed 5 findings.
<!-- SECTION:FINAL_SUMMARY:END -->
