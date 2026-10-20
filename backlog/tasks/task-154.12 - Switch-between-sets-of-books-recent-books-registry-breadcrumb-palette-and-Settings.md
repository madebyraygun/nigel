---
id: TASK-154.12
title: >-
  Switch between sets of books: recent-books registry, breadcrumb, palette and
  Settings
status: To Do
assignee: []
created_date: '2026-10-20 22:57'
labels:
  - web
  - design
  - desktop
milestone: m-4
dependencies: []
references:
  - docs/design/app-refresh/README.md
parent_task_id: TASK-154
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The book-switching half of the web app refresh (spec sections 1, 2, 3 and 7). A recent-books registry on the host (id, name, kind, path, encrypted, last opened, monogram, colour from the pastel ramp by hashed id) drives the palette's Books section and scope chip, 'New books…' and 'Open books from a folder…', the breadcrumb and status-line book buttons, and Settings > Location > Switch books. Picking a book reloads onto its nigel.db. On the Mac the Swift shell owns the registry with security-scoped bookmarks per folder (TASK-150.2); against a shared server (epic 32) a server serves one set of books, so the Books section lists only it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The registry records each opened set of books and survives restarts on each host that has one
- [ ] #2 Switching from the palette, breadcrumb, status line or Settings reloads onto the chosen books and updates title, breadcrumb and status line
- [ ] #3 A missing or moved folder is reported and offered for removal, never silently recreated
<!-- AC:END -->
