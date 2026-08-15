---
id: TASK-100
title: 'Web UI: date picker for invoice issue date and due date'
status: In Progress
assignee: []
created_date: '2026-08-12 23:45'
updated_date: '2026-08-14 17:46'
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

**Review round on PR #11 — three data-integrity bugs fixed, and two of the decisions above narrowed.**

- `Date.UTC(26, …)` is 1926, and the round trip only checked month and day, so `addDays('0026-08-07', 30)` answered `1926-09-06`: a shape-valid mistyped year silently booked a due date nineteen centuries out. The year is now set back after construction and checked with the rest of the round trip. The year is not refused — the backend deliberately takes any four-digit year.
- The terms prefill recognised its own label **by value**, so switching away from a preset deleted a `Net 30` that came off the invoice or out of the CLI. Provenance is now tracked in component state: the form rewrites only the exact text it wrote this session, a value from anywhere else resets the claim, and hand-editing the field ends it.
- `dueTermFor` inferred a preset from the arithmetic, so a hand-picked due date that happened to land 30 days out was reclassified as Net 30 and then **moved** when the issue date was edited. Nothing is inferred on load any more: an existing invoice opens as Custom (date present) or None (absent), and a preset is only ever an explicit choice made in the session that makes it.
- A preset whose date cannot be written at all (30 days past 9999-12-31) now reverts to None and writes no terms, rather than promising a period the invoice does not have. A preset merely *waiting* on the issue date is kept, since that is mid-edit rather than impossible.
- `netDueDateFor` answers a `DueDateOutcome` (`ok`/`pending`/`unreadable`/`unreachable`) so the hint can tell "set an issue date" apart from "the issue date is not a date yet", and the "empty means it never goes overdue" hint no longer renders under a filled-in custom date.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The invoice form’s issue date gets a native date picker, and its due date becomes a choice of terms rather than a calendar.

**What changed**
- `wc-invoice-form` renders both dates as `wa-input type="date"` (the `wc-reconcile-form` `type="month"` precedent: jsdom implements it, the `YYYY-MM-DD` check stays because Safari degrades the control to a text box, and the value the form emits is still the zero-padded ISO string the API demands).
- The due date is a select — No due date / Net 7 / Net 14 / Net 30 / Custom date — with the picker appearing only for Custom. `InvoiceFormValue` gains `dueTerm`.
- The behaviour is pure functions rather than event-handler soup: `addDays`/`netDueDateFor`, `withIssueDate`, `withDueTerm`, `dueTermFor` and `prefilledTerms`. Each of the form’s handlers calls one and emits the whole value. Dates are computed in UTC so a daylight-saving boundary cannot cost a day, and the year is set back after construction because `Date.UTC` maps 0-99 to 1900-1999 — a mistyped `0026` must not book a due date nineteen centuries out.
- `netDueDateFor` answers a `DueDateOutcome` rather than a string: "no date" has three causes the hint must tell apart (no issue date yet, an issue date that is not a day, a period landing past year 9999), and only the last makes the form refuse the choice — it reverts to None and writes no terms rather than promising a period the invoice cannot have.
- Nothing is inferred on load. A stored due date opens as Custom and an absent one as None, because a date thirty days out may be a Net 30 or a coincidence, and reading it as a preset would move a stored due date the moment the issue date was edited.
- `wc-payment-form`’s paid date takes the same picker with no presets.

**Decisions recorded**
- *A net preset fills the terms field — yes, non-destructively.* The page prints `2026-09-05 (Net 30)` beside the due date, so a Net 30 with empty terms would contradict the control that set it. The form rewrites only the exact text it wrote itself this session — tracked as provenance in component state, not recognised by value — so a `Net 30` loaded off the invoice or typed by hand is never deleted, and hand-editing the field ends the claim.
- *The payment form’s paid date follows — the picker, not the presets.* A payment landed on the day it landed; there is no period to count from.

**Tests**
- Pure-logic tests for padding, month/year rollover, two-digit-looking years, daylight-saving boundaries, the three no-date outcomes, preset-follows-issue-date (and custom/none staying put), the unreachable fallback, no-inference-on-load, and the terms provenance rules.
- Component tests for the `type="date"` controls, a date typed into a degraded text control, the term options, the picker appearing only for Custom, the moved due date, the four hint cases, and the three provenance paths.
- Preview states cover empty, every due-date choice (preset, preset awaiting an issue date, preset with an unreadable issue date, custom, none), filled, invalid and saving; `describePreviewA11y` passes with zero violations, and the value literals carry `satisfies InvoiceFormValue`.
- `npm ci`, `npm run build`, `npm test` (188 + 1089 + 760 passing, no Unhandled Errors), `npm run lint`, `npm run typecheck` all clean.
<!-- SECTION:FINAL_SUMMARY:END -->
