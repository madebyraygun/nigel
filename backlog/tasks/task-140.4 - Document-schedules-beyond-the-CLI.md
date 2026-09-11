---
id: TASK-140.4
title: Document schedules beyond the CLI
status: To Do
assignee: []
created_date: '2026-09-11 16:29'
labels:
  - invoicing
  - docs
dependencies: []
parent_task_id: TASK-140
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The docs describe schedules as a cron feature because that was all they were. Bring them up to what ships: the launch hook and what it will and will not do, the web tab, the endpoints, and the cron setup as one option among several rather than the only path.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 docs/invoicing.md covers the launch hook, including that it never sends and that autosend still needs a run
- [ ] #2 docs/commands.md and docs/api.md agree with what shipped
- [ ] #3 README.md's invoicing section does not imply cron is the only way schedules fire
<!-- AC:END -->
