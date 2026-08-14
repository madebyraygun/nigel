---
id: TASK-94
title: 'Web UI: toast renders off-screen'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-12 17:51'
updated_date: '2026-08-14 04:55'
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

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reproduce the placement from the stylesheet: .region set left: 50% then inset-inline: auto later in the same rule, unanchoring it
2. Re-anchor the region to a corner with both inline insets, no translate
3. Stack up to three toasts with per-toast timers; wrap long messages against the region width
4. Preview states for single, stacked and long-message; axe over every state
5. Tests assert resolved geometry (preview/css-geometry.ts) rather than declarations
6. npm ci / build / test / lint / typecheck from web/
<!-- SECTION:PLAN:END -->
