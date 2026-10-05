---
id: TASK-109.18
title: >-
  Documents: native drag-and-drop filing in the desktop shell, and the in-app
  PDF preview
status: To Do
assignee: []
created_date: '2026-10-05 23:22'
labels:
  - documents
  - desktop
dependencies: []
parent_task_id: TASK-109
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The documents dropzone is not wired to the desktop shell's native drop source (the import screen is), so dropping a PDF in the desktop app does nothing; and the detail view's embedded PDF sits in a sandboxed srcdoc frame that may not render (opaque origin, SameSite cookie, Chromium blocks PDFs in sandboxed frames).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Dropping a PDF on the documents screen in the desktop shell files it
- [ ] #2 The detail view's PDF preview renders in a browser and in the desktop shell, or is replaced by a working alternative
<!-- AC:END -->
