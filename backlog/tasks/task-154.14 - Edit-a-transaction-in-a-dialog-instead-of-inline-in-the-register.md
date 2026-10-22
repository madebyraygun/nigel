---
id: TASK-154.14
title: Edit a transaction in a dialog instead of inline in the register
status: To Do
assignee: []
created_date: '2026-10-22 16:50'
labels:
  - web
  - design
  - register
milestone: m-3
dependencies: []
parent_task_id: TASK-154
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Editing a register row opens a dialog instead of editing the row in place. Inline editing is cramped on a phone (the stacked two-line row grows and its fields squeeze onto line 2), and a dialog gives every field room on any width. The operator wants to try it on desktop too before settling the desktop behaviour, so the dialog ships for every width at first, with inline editing kept behind a switch until that call is made.

The dialog edits what the inline editor edits today (category, vendor, and anything else the row's editor exposes), shows the row's date, description, amount and account read-only for context, and saves through the same API calls.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Enter, double-click or tap on a row (and the existing edit shortcut) opens an edit dialog for that transaction at every width
- [ ] #2 The dialog shows date, description, amount and account read-only, and edits every field the inline editor edits, with the same validation, saving and errors
- [ ] #3 Save updates the row and keeps it selected and in view; Cancel or Esc closes without changes; focus returns to the row
- [ ] #4 Inline editing stays available behind a setting or flag so the desktop behaviour can be compared, and the choice is documented
- [ ] #5 The dialog is an @nigel/ui component with previews (phone and desktop, light and dark, error state) and describePreviewA11y passes
<!-- AC:END -->
