---
id: TASK-125
title: Ending a recurring schedule forgives periods it already owed
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-20 19:21'
updated_date: '2026-09-11 16:42'
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
Decision: `end` refuses when unbilled periods sit before the end date, and takes --bill or --forgive to say which was meant. Neither outcome happens silently.

Data layer (invoicing/schedules.rs):
1. `unbilled_periods(conn, id, on) -> Result<Vec<String>>` — walk next_period while cursor <= on, skipping periods with a run row. The same walk run_due_schedules does, so --bill produces exactly what a run on the end date would have produced. This is what pins AC #2: the walk cannot pass ended_at.
2. `EndDisposition { Forgive, Bill }`, and end_schedule takes Option<EndDisposition>. None refuses with a structured error naming the periods; Forgive ends leaving them ungenerated; Bill generates them then ends, in one transaction.
3. --bill drafts and never sends, even for an autosend schedule, and says so. Ending is a deliberate interactive act and drafting is the default TASK-81 already chose for generation.
4. A future `on` is refused — it would bill periods not yet due. The CLI only ever passes today.
5. --forgive leaves next_period where it is: it records where the schedule stopped, and an ended schedule never runs again.

CLI: End gains --bill and --forgive, conflicting. The refusal lists the periods and the two flags.

Docs: docs/invoicing.md and docs/commands.md.

Tests, written first: end with nothing owed is unchanged; end with periods owed refuses and writes nothing; --forgive ends and generates nothing; --bill generates exactly the owed periods dated by their own periods and nothing after ended_at; --bill keeps numbering sequential across several; --bill on an autosend schedule drafts and says it did not send; a future end date is refused.
<!-- SECTION:PLAN:END -->
