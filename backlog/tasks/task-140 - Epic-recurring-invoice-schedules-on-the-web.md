---
id: TASK-140
title: 'Epic: recurring invoice schedules on the web'
status: To Do
assignee: []
created_date: '2026-09-11 16:28'
labels:
  - epic
  - invoicing
  - web
dependencies:
  - TASK-125
references:
  - docs/superpowers/specs/2026-09-11-recurring-schedules-web-design.md
  - TASK-81
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Recurring schedules shipped a data layer and a CLI and nothing else — no route file, no endpoint, no SPA screen. `cli/invoice_schedule.rs` is the only surface in the workspace, so a retainer can only be set up by someone at a terminal.

The worse consequence is that even if the browser could create one, it would never fire. Generation lives in `nigel invoice schedule run`, which is built for cron, and nobody working in a desktop app edits a crontab.

Design: `docs/superpowers/specs/2026-09-11-recurring-schedules-web-design.md`. A launch hook that drafts what is due and never sends, an API mirroring the data layer, and a Schedules tab on the existing Invoices screen rather than a fourteenth sidebar entry.

Blocked on TASK-125: the screen needs an End button and a sentence saying what End does, and that is the question TASK-125 has not answered yet.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A schedule can be created, edited, paused, resumed and ended without touching a terminal
- [ ] #2 A schedule generates its due invoices for an operator who has never configured cron
- [ ] #3 Opening the app never sends email to a client
<!-- AC:END -->
