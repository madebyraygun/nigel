---
id: TASK-139
title: 'Epic: invoice payment reminders'
status: To Do
assignee: []
created_date: '2026-09-11 16:20'
labels:
  - epic
  - invoicing
dependencies: []
references:
  - docs/superpowers/specs/2026-09-11-invoice-payment-reminders-design.md
  - TASK-5 (closed without work)
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
An invoice that goes unpaid is chased by hand or not at all. TASK-5 'Invoicing: Automatic Reminders' carries a Done status but nothing was built — it was migrated from an archived GitHub issue with no acceptance criteria and marked done in a bulk sweep alongside five tasks that had shipped. There is no `remind` subcommand and no reminder code in the workspace.

The design is in `docs/superpowers/specs/2026-09-11-invoice-payment-reminders-design.md`. In short: the no-daemon pattern recurring schedules established — a cron-invoked command that does whatever is due, idempotent on a natural key, never prompting, working on an encrypted database via NIGEL_DB_PASSWORD. A global cadence of day offsets relative to the due date, a per-invoice off switch, and one table recording which offsets are discharged.

Reminders send unattended, which is where they part company with schedule runs. A generated invoice states new figures nobody has checked; a reminder states nothing new, quoting a balance the database already knows. So there is no queue, no release step and no review surface.

Recurring schedules shipped their engine and CLI with no web UI and the gap went unnoticed for months. The surfaces here are named children, not an implied follow-up.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Reminders are off until switched on, and an upgrade never turns them on by itself
- [ ] #2 Every child below is closed before this epic is
<!-- AC:END -->
