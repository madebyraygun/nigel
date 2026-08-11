---
id: TASK-69
title: 'Invoicing: validate_date accepts unpadded dates that break ISO comparisons'
status: In Progress
assignee:
  - '@stream-1'
created_date: '2026-08-08 04:33'
updated_date: '2026-08-11 21:24'
labels:
  - invoicing
  - bug
dependencies: []
documentation:
  - >-
    docs/superpowers/specs/2026-08-11-task-69-71-63-70-invoice-correctness-design.md
  - docs/superpowers/plans/2026-08-11-task-69-71-63-70-invoice-correctness.md
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
invoices::validate_date (68.1's shared rule) accepts chrono-parseable but unpadded dates like 2026-8-7. Stored in paid_date/due_date/issue_date, such a value breaks refresh_status's ISO string comparison against due_date and ar_aging's parse_from_str, whose failure path silently falls back to *today* — an invoice can misreport overdue status or aging bucket with no error. Fix: normalize to zero-padded %Y-%m-%d on validation (re-format the parsed date rather than storing the input verbatim), and add regression tests for an unpadded date round-tripping through pay/edit and landing padded in the row. Surfaced during TASK-68.4 implementation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 An unpadded but valid date entered anywhere (new/edit/pay, CLI or TUI) is stored zero-padded
- [x] #2 refresh_status and ar_aging behave identically for dates entered padded or unpadded
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented on branch feat/invoice-correctness (stream 1 of TASK-86).

validate_date now returns the normalized zero-padded YYYY-MM-DD instead of Result<()>, the same shape validate_currency has always had. The four functions that write a date column — create_invoice, update_invoice, record_payment, void_invoice — store what it returns, so normalization lives with the writer and no front end has to remember it. cli/invoice_manager.rs needed no edit: both forms already discarded the value, validating only to attribute a failure to a field.

record_payment gained the paid_date check it never had, beside its existing method check. That makes 'nigel invoice pay --date March' a refusal rather than junk in the column and a junk reference day for refresh_status — the flag's documented contract, ruled strict by the orchestrator.

Migration v6 pads the dates already stored (issue_date, due_date, published_at, voided_at, paid_date), parsing with chrono and leaving anything it cannot read alone. created_at/recorded_at are excluded as datetime('now') defaults.

The task text claimed the unpadded date breaks ar_aging's parse_from_str via a silent fallback to today. It does not: chrono parses 2026-8-7 fine, so aging always bucketed correctly and only *printed* the raw string. The real damage was is_overdue's string comparison, and that is what the headline regression test pins.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Dates that survive an ISO string comparison.

validate_date returns the re-formatted date; create_invoice, update_invoice, record_payment and void_invoice store it. record_payment validates its own paid_date for the reason it validates its own method. Migration v6 normalizes the rows written before the fix, leaving unparseable values untouched.

AC #1 (unpadded date entered anywhere is stored zero-padded): an_unpadded_issue_or_due_date_is_stored_padded, an_unpadded_date_edited_onto_a_draft_is_stored_padded, record_payment_stores_an_unpadded_date_padded, void_invoice_stores_an_unpadded_date_padded, plus unpadded_dates_round_trip_through_new_edit_and_pay_as_padded driving the real binary. The TUI inherits it — its forms hand raw strings to the data layer, which is now the padder.

AC #2 (refresh_status and ar_aging behave identically padded or unpadded): overdue_derives_the_same_whether_the_due_date_was_typed_padded_or_not (the half that was actually broken) and aging_buckets_and_prints_the_same_whether_the_due_date_was_typed_padded_or_not; v6 covers rows written before the fix.

Behavior change: 'invoice pay --date March' is now refused rather than recorded. Docs in CLAUDE.md (validator list, v6, a Key Design Constraint) and docs/invoicing.md (Recording payments).
<!-- SECTION:FINAL_SUMMARY:END -->
