---
id: TASK-139.1
title: 'Reminder engine, schema and `nigel invoice remind run`'
status: To Do
assignee: []
created_date: '2026-09-11 16:21'
labels:
  - invoicing
dependencies: []
parent_task_id: TASK-139
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The selection rules and the table behind them, plus the cron entry point. Design: `docs/superpowers/specs/2026-09-11-invoice-payment-reminders-design.md`.

`invoice_reminders` with UNIQUE(invoice_id, offset_days) carries the idempotency, the way schedule runs dedupe on (schedule, period). A row means an offset is discharged — outcome 'sent' or 'superseded' — whether or not an email left the building. `invoices` gains `reminders_enabled` for the per-invoice escape hatch; `Settings` gains `reminder_offsets` and a global `reminders_enabled` that defaults to off.

The run takes every in-scope invoice, computes due_date + offset for each configured offset, takes those now passed with no row, sends the largest and supersedes the rest. Two consequences of that rule are the ones that bite: a partially paid invoice keeps reminding on the remainder, and an offset added behind one already sent must be superseded rather than firing a gentle nudge after an escalation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 invoice_reminders exists with UNIQUE(invoice_id, offset_days); a second run the same day writes nothing and sends nothing
- [ ] #2 Three passed offsets send one email — the largest — and write superseded rows for the rest
- [ ] #3 An offset earlier than the latest already sent for an invoice is superseded, never sent
- [ ] #4 A partially paid invoice keeps reminding and quotes the remainder, not the original total
- [ ] #5 Paid, void, no due date, the per-invoice switch and the global switch each stop it
- [ ] #6 reminders_enabled defaults to off; a database upgraded from before the feature sends nothing
- [ ] #7 A client with no billing address is reported, writes no row, and still sends once the address is fixed
- [ ] #8 `nigel invoice remind run` never prompts, takes --today and --dry-run, and works on an encrypted database via NIGEL_DB_PASSWORD
- [ ] #9 `nigel invoice remind on|off <number>` sets the per-invoice switch, and the global switch can be turned on from the CLI
- [ ] #10 docs/invoicing.md and docs/commands.md cover the cadence, the cron setup and both switches
<!-- AC:END -->
