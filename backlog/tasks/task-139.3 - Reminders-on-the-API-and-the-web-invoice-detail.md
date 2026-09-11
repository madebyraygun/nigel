---
id: TASK-139.3
title: Reminders on the API and the web invoice detail
status: To Do
assignee: []
created_date: '2026-09-11 16:21'
labels:
  - invoicing
  - web
dependencies: []
parent_task_id: TASK-139
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The surface recurring schedules never got. The invoice detail shows what has been sent and what is still pending, and offers the per-invoice switch.

Per the component-first workflow: the history and the toggle live in `@nigel/ui` as `wc-*` components with co-located previews and `describePreviewA11y`, read theme tokens, and adopt controlsCss wherever they render a `wa-*` primitive. Nothing bespoke lands in `web/apps/app/src/components/`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The invoice detail payload carries reminder state: offsets discharged, when, and which are still pending
- [ ] #2 An endpoint sets the per-invoice reminders switch
- [ ] #3 The web invoice detail shows reminder history and the switch
- [ ] #4 The components live in @nigel/ui with previews covering every visible state, including an invoice with no reminders and one with the switch off
- [ ] #5 describePreviewA11y passes with zero violations
- [ ] #6 docs/api.md lists the endpoint and the added detail fields
<!-- AC:END -->
