---
id: TASK-75
title: 'Web UI: dark and light mode switcher'
status: To Do
assignee: []
created_date: '2026-08-09 00:46'
labels:
  - enhancement
  - web
  - ui
  - theme
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The theme already defines a full light palette and a dark one — @nigel/theme tokens/color.ts carries both, and every solid colour in each is held to WCAG AA by a contrast test. What is missing is any way to choose between them: the mode follows the OS and nothing else.

Wants an explicit three-state control — light, dark, and follow the system — rather than a two-way toggle, so that following the OS stays reachable once a choice has been made. The choice needs to outlive a reload.

Two things worth settling in the design: where the preference lives (settings.json is shared with the CLI and the TUI, which have no use for it, so localStorage is likely the right home), and that print styling must keep winning — theme/src/print.ts is composed last for exactly that reason and a mode override must not defeat it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A control offers light, dark, and follow the system
- [ ] #2 The choice persists across a reload
- [ ] #3 Following the system tracks prefers-color-scheme changes live, without a reload
- [ ] #4 Both palettes keep passing the existing contrast test
- [ ] #5 Print output is unaffected by the selected mode
- [ ] #6 The control is reachable by keyboard and passes axe in both modes
<!-- AC:END -->
