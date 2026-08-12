---
id: TASK-100
title: 'Web UI: date picker for invoice issue date and due date'
status: To Do
assignee: []
created_date: '2026-08-12 23:45'
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
The invoice form takes issue date and due date as free-text fields with a YYYY-MM-DD placeholder, so a date is typed by hand and only refused after submit. A picker removes the whole class of typo — which matters more now that validate_date requires a four-digit year and refuses anything it cannot read.

wc-reconcile-form:303 already sets the precedent: it uses a native wa-input type=month, confirmed to survive jsdom, and keeps its own string check because Safari degrades the control to a text input. A date picker should follow that shape rather than pulling in a calendar library — the control is native, the string check stays, and the value the form emits is the zero-padded ISO string the data layer already demands.

Two behaviours must survive: the due date can be empty (the form's own hint says an empty due date means the invoice never goes overdue), and a value typed by hand into a degraded text control is still accepted if it parses.

Worth deciding as part of this: whether the payment form's paid date (wc-payment-form) follows, since it is the same kind of field with the same validation behind it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Issue date and due date offer a native date picker on the invoice form
- [ ] #2 The due date can still be cleared, and an empty due date keeps its current meaning
- [ ] #3 A browser that degrades the control to text still accepts a typed date, and the existing shape check remains
- [ ] #4 The value submitted is always zero-padded YYYY-MM-DD
- [ ] #5 A decision is recorded on whether the payment form's paid date follows
- [ ] #6 Preview states cover empty, filled and invalid, and describePreviewA11y passes with zero violations
<!-- AC:END -->
