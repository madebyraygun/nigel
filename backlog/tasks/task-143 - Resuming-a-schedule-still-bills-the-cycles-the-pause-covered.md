---
id: TASK-143
title: Resuming a schedule still bills the cycles the pause covered
status: To Do
assignee: []
created_date: '2026-09-11 19:25'
labels:
  - invoicing
  - bug
dependencies: []
references:
  - TASK-125
  - TASK-81
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A pause forgives the cycles it covers — that is the rule, and TASK-125 made ending honour it: `unbilled_periods` stops the walk at `paused_at`, so ending a paused schedule offers only the arrears behind the pause.

Resuming does not honour it. `resume_schedule` clears `paused` and `paused_at` and leaves `next_period` where it stood, and `run_due_schedules` walks from `next_period` through today — so the first run after a resume invoices every cycle the pause was meant to skip. The same schedule forgives those cycles if you end it and bills them if you resume it, which cannot both be right.

The fix is not simply advancing `next_period` past the pause. Arrears sit *behind* the forgiven span — a schedule late since January and paused in March owes January and February and not March to May — and one cursor cannot express a gap in the middle.

The mechanism that can is already here: the walk skips any period carrying a row in `invoice_schedule_runs`, which is how a rerun stays idempotent. Resuming should write rows for the forgiven periods, marking them skipped rather than billed. That needs `invoice_schedule_runs.invoice_id` to become nullable with a reason beside it, and it gives the operator something worth having — `nigel invoice schedule show` would say which cycles were skipped and why, instead of them silently never appearing.

Once resume writes those rows, the `paused_at` check in `unbilled_periods` covers only the still-paused case, and the two paths agree by construction rather than by keeping two rules in step.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The first run after a resume generates nothing for any cycle the pause covered
- [ ] #2 Arrears that predate the pause are still generated
- [ ] #3 A skipped period is recorded rather than merely absent, and cannot be billed later by a second run
- [ ] #4 `nigel invoice schedule show` distinguishes a skipped period from a billed one
- [ ] #5 A schedule paused before migration v13 has no pause date and so forgives nothing on resume
- [ ] #6 Ending and resuming agree about what a paused schedule owes
- [ ] #7 docs/invoicing.md and docs/architecture.md describe the skip rows
<!-- AC:END -->
