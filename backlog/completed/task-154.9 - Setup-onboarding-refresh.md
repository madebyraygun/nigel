---
id: TASK-154.9
title: Setup (onboarding) refresh
status: Done
assignee: []
created_date: '2026-10-20 22:57'
updated_date: '2026-10-23 16:53'
labels:
  - web
  - design
milestone: m-3
dependencies:
  - TASK-154.1
references:
  - docs/design/app-refresh/README.md
parent_task_id: TASK-154
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Spec section 8: keep the particle field and ASCII wordmark with the new gradients and glow; centred 640px layout; pill buttons; the profile choice as side-by-side cards that pick and advance; the first-books step as two cards; a step indicator. Copy is unchanged except 'Tell me about yourself.' on the identity step. Coordinate with TASK-33.21 (demo users return to Setup) and TASK-118 (load path keeps the user's name).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 All four steps match the spec in light and dark and the existing onboarding flows still pass their tests
- [x] #2 Every new or changed @nigel/ui component has a preview covering its states in light and dark, and describePreviewA11y passes with zero violations
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in PR #63 (with a Back button on the profile step).
<!-- SECTION:FINAL_SUMMARY:END -->
