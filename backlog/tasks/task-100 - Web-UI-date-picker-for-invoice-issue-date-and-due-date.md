---
id: TASK-100
title: 'Web UI: date picker for invoice issue date and due date'
status: In Progress
assignee: []
created_date: '2026-08-12 23:45'
updated_date: '2026-08-14 04:56'
labels:
  - web
  - ui
  - invoicing
  - enhancement
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The invoice form takes issue date and due date as free-text fields with a YYYY-MM-DD placeholder, so a date is typed by hand and only refused after submit. Pickers remove the whole class of typo — which matters more now that validate_date requires a four-digit year and refuses anything it cannot read.

Issue date wants a plain date picker. Due date wants terms, not a calendar: the common case is a net period counted from the issue date, so the control offers Net 7, Net 14, Net 30, a custom date, and none. Picking a preset computes the date from the issue date rather than making a person count days, and changing the issue date afterwards moves a preset-derived due date with it. Custom falls back to the same picker the issue date uses.

wc-reconcile-form:303 sets the precedent for the picker itself: a native wa-input type=month, confirmed to survive jsdom, keeping its own string check because Safari degrades the control to a text input. Follow that shape rather than pulling in a calendar library — native control, string check retained, and the value the form emits stays the zero-padded ISO string the data layer demands.

Two behaviours must survive: the due date can be empty (the form's own hint says an empty due date means the invoice never goes overdue), and a value typed by hand into a degraded text control is still accepted if it parses.

Worth deciding as part of this: whether choosing a net preset should also fill the invoice's existing terms field (the reference invoices read '09/05/2026 (Net 30)'), and whether the payment form's paid date (wc-payment-form) follows with a picker of its own.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A browser that degrades the control to text still accepts a typed date, and the existing shape check remains
- [x] #2 The value submitted is always zero-padded YYYY-MM-DD
- [x] #3 A decision is recorded on whether the payment form's paid date follows
- [x] #4 Preview states cover empty, filled and invalid, and describePreviewA11y passes with zero violations
- [x] #5 Issue date offers a native date picker
- [x] #6 Due date is chosen as Net 7, Net 14, Net 30, a custom date, or none
- [x] #7 A net preset computes the due date from the issue date, and changing the issue date moves a preset-derived due date with it
- [x] #8 The due date can still be cleared, and an empty due date keeps its current meaning
- [x] #9 A decision is recorded on whether a net preset also fills the terms field
<!-- AC:END -->



## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. wc-invoice-form.ts: add `dueTerm` to InvoiceFormValue plus pure helpers — netDueDate/withIssueDate/withDueTerm/dueTermFor — so a preset-derived due date follows the issue date in data, not in handlers.
2. Render the issue date as a native wa-input type=date, and the due date as a term select (None / Net 7 / Net 14 / Net 30 / Custom) with the custom date picker appearing only for Custom. Keep the DATE_PATTERN shape check for degraded text controls.
3. Prefill the terms field from a net preset, non-destructively (empty or a previously auto-filled label only).
4. wc-payment-form.ts: same native picker on the paid date, no presets.
5. invoice-data.ts: infer dueTerm in invoiceFormFrom; requests keep sending only dueDate.
6. Preview states for empty/preset/custom/none plus an invalid custom date; `satisfies` on the value literals; describePreviewA11y stays at zero violations.
7. Tests for the pure helpers and the component wiring; npm ci/build/test/lint/typecheck.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Two decisions recorded.

**A net preset also fills the terms field — yes, non-destructively.** The reference invoices print `2026-09-05 (Net 30)` beside the due date, and `document::terms_block_text` folds a single-line terms onto that row, so a Net 30 chosen in the form and an empty terms field would raise an invoice whose page contradicts the control that made it. `prefilledTerms` writes only over an **empty** field or a label it wrote itself (`Net 7`/`Net 14`/`Net 30`), so a sentence an operator typed is never overwritten; switching to a custom date or to none clears its own label rather than leaving a period the dates no longer describe. Nothing is written on load or on save — only on an explicit change of the due-date choice — so an existing invoice opened for editing keeps its terms verbatim.

**The payment form’s paid date follows — the picker, not the presets.** `wc-payment-form`’s date is now the same `wa-input type="date"` with the same shape check. It gets no term presets: a payment landed on the day it landed, and there is no period to count it from. `paymentFormFor` already seeds today, which is the only default that means anything there.

The preset-follows-issue-date behaviour lives in pure functions on `InvoiceFormValue`, not in handlers: `addDays`/`netDueDate` (UTC arithmetic so a daylight-saving boundary cannot cost a day; a computed date is always zero-padded, and a date the rule cannot read or a sum past year 9999 answers empty rather than guessing), `withIssueDate` (moves a net-derived due date, leaves a custom one and an absent one), `withDueTerm` (computes, keeps or clears), `dueTermFor` (reads an existing invoice’s two dates back as the choice that made them, which is what makes an edit behave like the raise) and `prefilledTerms`. The component’s three handlers each call one of them and emit the whole value.
<!-- SECTION:NOTES:END -->
