---
id: TASK-139.4
title: Reminders on the TUI invoice detail
status: To Do
assignee: []
created_date: '2026-09-11 16:21'
labels:
  - invoicing
  - tui
dependencies: []
parent_task_id: TASK-139
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The same two affordances as the web detail — reminder history and the per-invoice switch — on the TUI invoice subscreen, so the terminal path is not the one that cannot see why a client stopped being chased.

Scoped low and behind the web child: decision-8 puts web and desktop ahead of the TUI, and the CLI switch already covers the operator who lives in a terminal.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The TUI invoice detail shows which reminders have been sent and which are pending
- [ ] #2 The per-invoice reminders switch can be toggled from the TUI invoice detail
- [ ] #3 docs/commands.md covers the TUI affordance
<!-- AC:END -->
