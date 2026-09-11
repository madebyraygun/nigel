---
id: TASK-137
title: Invoice line items cannot be selected for copy and paste
status: To Do
assignee: []
created_date: '2026-09-11 16:11'
labels:
  - invoicing
  - web
  - bug
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The app document sets `user-select: none` (`web/apps/app/index.html:44`) so dragging the desktop shell's chrome does not paint a selection. Every component that shows text a person would want to copy opts back in with `user-select: text` — wc-invoice-table, wc-report-table, wc-payment-list, wc-money and a dozen others do.

`wc-line-items` does not. It renders the read-only line items on the invoice detail (`web/apps/app/src/screens/invoices.ts:954`), so a description someone wants to paste into an email or a new invoice cannot be selected at all.

The fix is the one-line opt-in the other components already carry. Check the editing mode too — the inputs are selectable because they are form controls, but the surrounding description text and the row totals are not.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Read-only line item text — description, quantity, unit amount and row total — can be selected and copied in the invoice detail
- [ ] #2 The component carries the `user-select: text` opt-in itself rather than the app screen working around the document rule
- [ ] #3 A test pins the opt-in, the way wc-money.test.ts does
<!-- AC:END -->
