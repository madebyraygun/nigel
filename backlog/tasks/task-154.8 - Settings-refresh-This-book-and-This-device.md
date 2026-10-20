---
id: TASK-154.8
title: 'Settings refresh: This book and This device'
status: To Do
assignee: []
created_date: '2026-10-20 22:57'
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
Spec section 7, replacing screens/settings.ts. A sticky index with scroll-spy over two groups: This book (Letterhead, Security, Location) and This device (Appearance, Updates). Letterhead saves through a sticky save bar instead of per-section buttons, and a saved name updates the breadcrumb, status line and window title. Security keeps the existing validation and error handling in the new status card and expanding panels. Location shows the books folder (Reveal in Finder in Tauri; Switch books arrives with book switching). Appearance picks System, Light or Dark per device. Updates is hidden in a store build (TASK-150.4). In the Swift shell this screen is what the native settings window (TASK-144.8) hosts.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every setting the current screen edits is still editable, with its existing validation and errors
- [ ] #2 Unsaved letterhead changes show the save bar; Discard restores the saved values; Save updates breadcrumb, status line and window title
- [ ] #3 Appearance applies immediately and persists per device, not in the book
- [ ] #4 Every new or changed @nigel/ui component has a preview covering its states in light and dark, and describePreviewA11y passes with zero violations
<!-- AC:END -->
