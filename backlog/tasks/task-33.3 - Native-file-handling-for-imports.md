---
id: TASK-33.3
title: Native file handling for imports
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-06 16:29'
updated_date: '2026-08-19 14:17'
labels:
  - tauri
  - frontend
dependencies:
  - TASK-33.2
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Replace the browser upload dance with native affordances when running in the desktop shell: file-open dialog scoped to CSV/XLSX and drag-and-drop onto the window, passing paths straight to the path-based import pipeline. The web upload flow remains for remote mode.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Import works via native file dialog and window drag-and-drop in the desktop app
- [ ] #2 Preview and confirm behavior matches the web import flow
- [ ] #3 Remote mode falls back to the upload flow
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Spec: docs/superpowers/specs/2026-08-19-native-import-handling-design.md. Plan: docs/superpowers/plans/2026-08-19-native-import-handling.md (branch feat/native-imports). Staging commands in nigel-desktop reuse uploads::store so preview/confirm are shared with the web flow; ImportSource seam in the API client; wc-dropzone native mode; Tauri drag-drop events through the client seam.
<!-- SECTION:PLAN:END -->
