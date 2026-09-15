---
id: TASK-144.6
title: >-
  Native sidebar and toolbar: NavigationSplitView over the posted registry,
  vibrancy, unified title bar
status: To Do
assignee: []
created_date: '2026-09-15 16:06'
labels:
  - swift
  - macos
  - ui
dependencies:
  - TASK-144.5
documentation:
  - docs/superpowers/specs/2026-09-14-swift-shell-design.md
parent_task_id: TASK-144
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 2. NavigationSplitView renders the registry the SPA posts; selecting an item sets location.hash in the page; navigated keeps the selection and the toolbar title in step when the page navigates itself. The sidebar gets system vibrancy and Reduce Transparency handling for free, with no private API, which un-forecloses the App Store that TASK-33.26's route would have closed. The toolbar carries the sidebar toggle and the screen title; the web header stays in the page. Absorbs TASK-33.20, 33.26 and 33.27.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The sidebar lists exactly the registry the SPA posted, in its order, and follows profile changes
- [ ] #2 Selecting an item navigates the page; a page-initiated navigation updates the selection and title
- [ ] #3 The sidebar shows vibrancy in light and dark, Reduce Transparency yields an opaque sidebar, and the window uses the unified title bar with the toggle and title in the toolbar
- [ ] #4 TASK-33.20, 33.26 and 33.27 are closed against this task
<!-- AC:END -->
