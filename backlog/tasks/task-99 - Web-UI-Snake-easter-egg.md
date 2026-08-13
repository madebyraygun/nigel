---
id: TASK-99
title: 'Web UI: Snake easter egg'
status: To Do
assignee: []
created_date: '2026-08-12 22:51'
labels:
  - web
  - ui
  - enhancement
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TUI dashboard hides a Snake game behind the s key (cli/snake.rs, with the shared pastel gradient and particle effects from effects.rs). The web app should carry the same easter egg so the two front ends share their one secret. Needs a discreet trigger (a key on the dashboard when no input has focus, or a konami-style sequence — deliberately undocumented in the UI), a wc-snake component rendered as an overlay, arrow-key controls, score display, and an exit back to whatever screen was underneath. Visuals should read as the same game as the TUI: the brand gradient snake and the effects.rs-derived palette the theme already carries. It must not interfere with normal typing — the trigger only fires when focus is not in a form control.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A hidden trigger on the web dashboard opens Snake without being discoverable from the visible UI
- [ ] #2 Arrow keys steer, the game scores, and Esc exits back to the underlying screen with prior focus restored
- [ ] #3 The trigger never fires while focus is inside a form control or dialog
- [ ] #4 The snake renders in the brand gradient palette shared with the TUI game
- [ ] #5 Component-first: wc-snake ships with a preview and describePreviewA11y passes (reduced-motion respected)
<!-- AC:END -->
