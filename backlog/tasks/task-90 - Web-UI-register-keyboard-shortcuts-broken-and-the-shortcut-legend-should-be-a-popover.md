---
id: TASK-90
title: >-
  Web UI: register keyboard shortcuts broken, and the shortcut legend should be
  a popover
status: To Do
assignee: []
created_date: '2026-08-12 17:50'
labels:
  - web
  - ui
  - bug
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Two defects on the register screen. (1) The documented register shortcuts (arrows, PgUp/PgDn, Home/End, Enter, Esc, f, /) do not work — keystrokes do nothing on the table. (2) The 'Keyboard' disclosure expands inline as a plain block that overlaps/pushes the layout (screenshot); it should render as a proper popover anchored to the trigger, dismissable with Esc and outside-click, keyboard reachable.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every shortcut the legend lists works on the register table
- [ ] #2 The legend renders as an anchored popover that does not disturb the page layout
- [ ] #3 The popover is keyboard reachable and dismissable (Esc, outside click), and axe passes
<!-- AC:END -->
