---
id: TASK-71
title: 'Invoicing: update_invoice derives status against the issue date, not today'
status: Done
assignee:
  - '@stream-1'
created_date: '2026-08-08 08:22'
updated_date: '2026-08-11 22:12'
labels:
  - invoicing
  - bug
dependencies: []
documentation:
  - >-
    docs/superpowers/specs/2026-08-11-task-69-71-63-70-invoice-correctness-design.md
  - docs/superpowers/plans/2026-08-11-task-69-71-63-70-invoice-correctness.md
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
update_invoice passes the invoice's issue date to refresh_status as "today", so a due-date edit derives overdue relative to the issue date rather than the wall clock. Harmless today because only drafts are editable and drafts never derive overdue — but it is a trap if edit is ever widened (TASK-35's stale-overdue work touches the same derivation). Fix by threading the real today through, matching void_invoice/record_payment. Surfaced during TASK-68.6 stage 3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 refresh_status is always called with the wall-clock today, and a test pins the due-date-edit path
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
update_invoice takes today as a trailing positional parameter, matching void_invoice's shape (orchestrator ruling: positional, not a field on InvoiceUpdate, which a caller could forget). The tail of the function drops the 'use the issue date as today' block and calls refresh_status(conn, invoice_id, today).

Reading the clock inside src/invoicing/ was rejected: no module under it does, every other date-sensitive function takes its reference day as an argument, and a Local::now() call there would take the module's deterministic tests with it.

Production callers: cli::invoice::edit grew a trailing today: &str and main.rs passes &cli::today() at the dispatch site the way Void/Send/Sync already do; the server's update handler computes let today = crate::cli::today() before with_conn_api, the shape sync uses. 19 in-module test call sites gained a literal date.

The pinning test needed published_at set by hand, because that is the only shape where an *editable* invoice can reach the overdue branch at all — mark_published would move the status off draft and ensure_editable would then refuse the edit. Verified red (derived 'sent') before the fix.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
update_invoice derives status against the wall clock, not the invoice's issue date.

today arrives as a trailing parameter like void_invoice's voided_on, keeping the invariant that nothing under src/invoicing/ reads the clock. Both production callers pass cli::today().

AC #1: a_due_date_edit_derives_status_against_today_not_the_issue_date pins the due-date-edit path — confirmed red ('sent' vs 'overdue') against the old derivation. Recorded in CLAUDE.md as a Key Design Constraint.
<!-- SECTION:FINAL_SUMMARY:END -->
