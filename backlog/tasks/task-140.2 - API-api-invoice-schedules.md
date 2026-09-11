---
id: TASK-140.2
title: 'API: /api/invoice-schedules'
status: To Do
assignee: []
created_date: '2026-09-11 16:28'
labels:
  - invoicing
  - api
dependencies: []
parent_task_id: TASK-140
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`server/routes/invoice_schedules.rs`, mirroring the data layer one-for-one because `schedules.rs` already exposes everything the screen needs.

    GET    /api/invoice-schedules?all=        list_schedules
    POST   /api/invoice-schedules             add_schedule
    GET    /api/invoice-schedules/{id}        get_schedule + schedule_items + schedule_runs
    PATCH  /api/invoice-schedules/{id}        update_schedule
    POST   /api/invoice-schedules/{id}/pause  pause_schedule
    POST   /api/invoice-schedules/{id}/resume resume_schedule
    POST   /api/invoice-schedules/{id}/end    end_schedule, ending today as the CLI does
    POST   /api/invoice-schedules/run         run_due_schedules, autosend honoured

The run endpoint sends real email when a schedule has autosend, so its response must carry enough for the caller to have warned first: which invoices are due, for whom, and which of those will be sent rather than drafted.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every endpoint round-trips against the real router with a real session
- [ ] #2 The detail carries the schedule, its line items and its run history — which invoice came from which period
- [ ] #3 The run endpoint reports per-schedule failures as data rather than failing the whole request, as sync does
- [ ] #4 A run's response distinguishes invoices that were sent from those left as drafts
- [ ] #5 docs/api.md lists every endpoint and its error cases
<!-- AC:END -->
