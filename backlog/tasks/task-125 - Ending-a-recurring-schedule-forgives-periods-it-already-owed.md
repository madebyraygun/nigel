---
id: TASK-125
title: Ending a recurring schedule forgives periods it already owed
status: To Do
assignee: []
created_date: '2026-08-20 19:21'
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
