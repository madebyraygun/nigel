---
id: TASK-125
title: Ending a recurring schedule forgives periods it already owed
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-20 19:21'
updated_date: '2026-09-11 17:33'
labels:
  - invoicing
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
TASK-81's schedule model marks a schedule ended with an ended_at date, and list_schedules(Active) filters on ended_at IS NULL with no date comparison. So ending a schedule drops every ungenerated period, including ones whose issue date falls before ended_at: end_schedule(id, '2026-06-01') on a schedule sitting at next_period 2026-01-01 silently forgives five cycles the client was owed an invoice for.

Whether that is right is a judgement call the original ACs did not make. Forgiving everything is defensible (you ended it because you stopped billing them); generating up to ended_at and then stopping is equally defensible and is what an operator ending a schedule in June for work billed since January would probably expect. Decide it deliberately, document the answer where someone ending a schedule will read it, and pin it with a test either way.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The behaviour when a schedule ends with unbilled periods before its end date is decided, documented and tested
- [ ] #2 Whichever way it goes, ending a schedule never generates a period dated after ended_at
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Decision: `end` refuses when unbilled periods sit on or before the end date, and takes --bill or --forgive to say which was meant. Neither outcome happens silently.

Data layer (invoicing/schedules.rs):
1. `unbilled_periods(conn, id, on)` — walk next_period while cursor <= on, skipping periods with a run row. No period after `on` is reachable, which is what pins AC #2. It is NOT equivalent to what a run would generate: a run walks ScheduleScope::Active and skips paused and ended schedules. A paused schedule still owes its cycles here, because pausing only holds next_period still and resuming bills the backlog. An ended schedule owes nothing.
2. `EndDisposition { RefuseIfOwed, Forgive, Bill }` — three named intents rather than an Option, so the signature states all three and the caller cannot express a fourth.
3. `EndOutcome { Ended { on, settled }, BillingStopped { billed, owed } }` — an enum, because the outcomes are mutually exclusive and one of them is a failure returned as Ok. A struct of optional fields would let a future HTTP handler answer 200 for a walk that stranded periods.
4. --bill drafts and never sends, even for an autosend schedule. Ending is a deliberate interactive act and drafting is the default TASK-81 already chose.
5. Each period commits on its own — generate_period owns the transaction — so a walk that stops partway leaves real invoices. The schedule is then left active rather than ended, and the run rows make a retry skip what landed. describe() failing after a commit is recorded as a failure, never raised with ?, which would discard the record of what was created.
6. Ending is terminal. --forgive leaves next_period behind the end date, so without a guard a second end would walk the same gap and bill periods dated after the schedule stopped.
7. No future-date guard: schedules.rs reads no clock by design, so it has no today to compare against. The precondition is documented on unbilled_periods instead, and every caller in the workspace passes today. If TASK-140.2 accepts a request-supplied date, the guard belongs at that boundary.

CLI: End gains --bill and --forgive, conflicting. The refusal is re-raised as a Conflict with the flag names appended — not narrowed to Other, which an API layer would answer 500 for.

Docs: docs/invoicing.md, docs/commands.md, docs/architecture.md.

Tests: core covers nothing owed, refusal writing nothing, forgive, an end date mid-cycle, quarterly anchor restoration, sequential numbering, autosend still drafting, a stopped walk, an already-billed period, an already-ended schedule, and a paused one. cli_dispatch covers the refusal text, both flags, the terminal guard and the clap conflict.
<!-- SECTION:PLAN:END -->
