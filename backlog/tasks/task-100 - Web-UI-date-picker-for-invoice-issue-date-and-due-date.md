---
id: TASK-100
title: 'Web UI: date picker for invoice issue date and due date'
status: To Do
assignee: []
created_date: '2026-08-12 23:45'
updated_date: '2026-08-12 23:52'
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
- [ ] #1 A browser that degrades the control to text still accepts a typed date, and the existing shape check remains
- [ ] #2 The value submitted is always zero-padded YYYY-MM-DD
- [ ] #3 A decision is recorded on whether the payment form's paid date follows
- [ ] #4 Preview states cover empty, filled and invalid, and describePreviewA11y passes with zero violations
- [ ] #5 Issue date offers a native date picker
- [ ] #6 Due date is chosen as Net 7, Net 14, Net 30, a custom date, or none
- [ ] #7 A net preset computes the due date from the issue date, and changing the issue date moves a preset-derived due date with it
- [ ] #8 The due date can still be cleared, and an empty due date keeps its current meaning
- [ ] #9 A decision is recorded on whether a net preset also fills the terms field
<!-- AC:END -->
