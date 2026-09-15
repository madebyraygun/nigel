---
id: TASK-144.12
title: 'Notifications: update available, and a paid invoice found by a timer sync'
status: To Do
assignee: []
created_date: '2026-09-15 16:07'
labels:
  - swift
  - macos
dependencies:
  - TASK-144.4
documentation:
  - docs/superpowers/specs/2026-09-14-swift-shell-design.md
parent_task_id: TASK-144
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 3. UNUserNotificationCenter, only for things that happen while the user is not looking: an update available from /api/status, and a sync finding a paid invoice when the shell runs POST /api/invoices/sync on a timer with the window closed and sync configured. Clicking opens the relevant screen by hash. Permission is requested on first use, never at launch.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An update available and a newly paid invoice each produce one notification, and clicking it opens the right screen
- [ ] #2 The timer sync runs only when sync is configured and the window is closed, and never overlaps a sync the page started
- [ ] #3 Notifications can be turned off in the app's settings; permission is asked on first use
<!-- AC:END -->
