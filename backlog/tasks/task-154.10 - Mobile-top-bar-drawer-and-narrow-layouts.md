---
id: TASK-154.10
title: 'Mobile: top bar, drawer and narrow layouts'
status: To Do
assignee: []
created_date: '2026-10-20 22:57'
labels:
  - web
  - design
  - mobile
milestone: m-3
dependencies:
  - TASK-154.2
  - TASK-154.4
references:
  - docs/design/app-refresh/README.md
parent_task_id: TASK-154
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Spec section 9: on a narrow viewport a 52px top bar (hamburger, compact breadcrumb, search), a floating drawer with the nav groups (Settings moved into Setup), 44px items and a book card at the foot in place of the status line; stats stack into one card, the chart is 150px tall, cards take a 14px radius. All touch targets at least 44px. Pairs with TASK-124 (installable iPad web app).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 At 390px wide every screen is usable with no horizontal scroll, and every touch target is at least 44px
- [ ] #2 The drawer opens and closes by button, backdrop and item selection, with focus managed
- [ ] #3 Every new or changed @nigel/ui component has a preview covering its states in light and dark, and describePreviewA11y passes with zero violations
<!-- AC:END -->
