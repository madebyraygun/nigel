---
id: TASK-140.1
title: 'Launch hook: draft due schedules without ever sending'
status: To Do
assignee: []
created_date: '2026-09-11 16:28'
labels:
  - invoicing
dependencies: []
parent_task_id: TASK-140
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The piece that makes schedules work for someone who has never configured cron. Design: `docs/superpowers/specs/2026-09-11-recurring-schedules-web-design.md`.

`draft_due_schedules` (`invoicing/schedules.rs:492`) looks like the entry point and is not. It drafts every due schedule including autosend ones — correct for an installation with no sending configured, which is what it was written for. But generating a period consumes it: the run row and the advanced next_period commit in the same transaction as the invoice. A launch hook calling it would draft an autosend retainer, discharge the period, and leave a later cron run with nothing to do. The invoice would never be sent and nothing would report an error.

So this needs a variant that skips autosend schedules without generating or advancing them, reporting them as due and awaiting a run.

Placement matters as much as behaviour. `sync_invoice_payments()` is guarded by `if !matches!(command, … | Commands::Serve { .. } | …)` — serve is excluded from it. Beside it is therefore the one dispatch path a web or desktop operator never takes. Two call sites: serve startup in `server/mod.rs` before the listener binds, and the CLI dispatch guard. Do not widen the sync hook's guard while here; that is TASK-37.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A run variant skips autosend schedules entirely — no invoice generated, next_period unmoved — and reports them as due
- [ ] #2 A cron run after a launch hook still sends the autosend schedule, because the period was never consumed
- [ ] #3 The hook runs on serve startup, not only in the CLI dispatch guard
- [ ] #4 The hook is best-effort: a failure prints a notice, does not fail the command, and does not stop serve from binding
- [ ] #5 With no schedules due the hook is silent
- [ ] #6 TASK-37's guard is left alone
<!-- AC:END -->
