---
id: TASK-154.16
title: 'One standard switch: high-contrast off state and a check when on'
status: Done
assignee: []
created_date: '2026-10-22 17:26'
updated_date: '2026-10-22 20:24'
labels:
  - web
  - design
  - theme
  - a11y
milestone: m-3
dependencies: []
parent_task_id: TASK-154
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The theme never styles wa-switch, so screens show Web Awesome's default: a pale off track that is hard to see (the Clients 'Show archived' switch). PR #65 restyled its own switch in wc-switch-row instead. Make the standard switch accessible and consistent everywhere, with no per-component switch styling.

Reference (operator's examples): the off state is a light track with a clearly visible mid-grey border and a white knob with its own border; the on state is a filled track with a check mark inside the knob.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Every wa-switch is styled by the theme: 38x22 track, pill radius, 150ms transition (zeroed under reduced motion)
- [x] #2 Off: light track with a border that meets 3:1 against the surface it sits on, and a white knob with a border meeting 3:1 against the track, in light and dark
- [x] #3 On: track filled with --br, knob shows a check mark (icon from the set) meeting 3:1 against the knob; state never conveyed by colour alone
- [x] #4 Focus-visible ring per docs/native-feel.md, surviving forced colors; disabled at 40%
- [x] #5 No component styles a switch itself: wc-switch-row (PR #65) uses the standard switch, and the guard test from TASK-154.15 also covers switches
- [x] #6 Contrast tests pin the off border, knob border and check-mark pairs; previews show off, on, focus and disabled in light and dark
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in PR #68 (with shared list geometry, true 700 table headings and a 3:1 outline border, per the operator).
<!-- SECTION:FINAL_SUMMARY:END -->
