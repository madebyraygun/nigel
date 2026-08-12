---
id: TASK-94
title: 'Web UI: toast renders off-screen'
status: To Do
assignee: []
created_date: '2026-08-12 17:51'
labels:
  - web
  - ui
  - bug
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Toasts render partially outside the viewport — screenshot shows a success toast ('…recorded 0.') clipped at the top-left corner over the sidebar brand, with most of the message cut off. Toasts should render inside the viewport in a consistent corner with sane stacking, and never clip their message.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Toasts render fully inside the viewport in a consistent position on every screen
- [ ] #2 Long toast messages wrap rather than clip
<!-- AC:END -->
