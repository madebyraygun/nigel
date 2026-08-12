---
id: TASK-93
title: 'Theme: glow effect on button hover'
status: To Do
assignee: []
created_date: '2026-08-12 17:51'
labels:
  - web
  - ui
  - theme
  - enhancement
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Buttons currently change little on hover. Add a subtle glow (soft box-shadow in the button's accent color, e.g. the brand gradient hues) on hover/focus-visible for primary and secondary buttons. Must respect prefers-reduced-motion for any transition, pass the contrast suites, and hold up in both light and dark modes.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Primary and secondary buttons show a visible glow on hover and focus-visible in both modes
- [ ] #2 Transitions respect prefers-reduced-motion and all contrast/a11y suites pass
<!-- AC:END -->
