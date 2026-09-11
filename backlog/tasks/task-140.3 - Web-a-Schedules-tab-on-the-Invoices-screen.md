---
id: TASK-140.3
title: 'Web: a Schedules tab on the Invoices screen'
status: To Do
assignee: []
created_date: '2026-09-11 16:29'
labels:
  - invoicing
  - web
dependencies: []
parent_task_id: TASK-140
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A tab on the existing Invoices screen rather than a fourteenth sidebar entry — a schedule is a thing that produces invoices and belongs where invoices are, and a small shop keeps three of them.

Per the component-first workflow, `wc-schedule-list` and `wc-schedule-form` live in `@nigel/ui` with co-located previews and `describePreviewA11y`, read theme tokens, and adopt controlsCss wherever they render a `wa-*` primitive. The form reuses `wc-line-items`.

Two things the screen must not hide. Running sends real email when a schedule has autosend, so Run takes a confirmation naming which invoices go to whom — the discipline the CLI applies by refusing a non-TTY send without --yes. And catch-up generates one invoice per missed cycle, each dated its own period, so a schedule six cycles stale produces six invoices; the count is stated before the run, not discovered after it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Schedules can be created, edited, paused, resumed and ended from the tab
- [ ] #2 The form offers an optional 'start from invoice' prefill — items, currency, notes, terms and the issue-to-due term — the web parallel of the CLI's --from
- [ ] #3 The form states that edits apply to future invoices, never past ones
- [ ] #4 Run warns before it sends, naming which invoices go to which clients, and how many a stale schedule will generate
- [ ] #5 The detail shows the schedule's line items and its run history
- [ ] #6 Components ship with previews covering no schedules, active, paused, ended, autosend, and a form with validation errors
- [ ] #7 describePreviewA11y passes with zero violations
- [ ] #8 Schedules join the invoicing-parity manifest, so the screen shows the figures nigel invoice schedule prints
<!-- AC:END -->
