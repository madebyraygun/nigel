---
id: TASK-154.19
title: 'Category picker: keep the highlighted option in view inside dialogs'
status: To Do
assignee: []
created_date: '2026-10-24 18:32'
labels:
  - web
  - bug
  - a11y
milestone: m-3
dependencies: []
parent_task_id: TASK-154
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
In the register's Edit transaction dialog, the category list opens below the Category field but the dialog body clips it: only the first option or two show above the footer, and arrowing down highlights options that are out of sight, so you can't see what's being chosen. Neither the list nor the dialog body scrolls to follow the highlighted option. The same picker is used in Review and the Rules dialog.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Arrowing through wc-category-picker's list keeps the highlighted option visible: the list scrolls it into view (block: nearest) on every highlight change, including Home/End and typing that changes the match
- [ ] #2 Inside a dialog the open list is fully visible: it is not clipped by the dialog body (render it in the top layer/popover, flip above the field when there is no room below, or scroll the dialog body so the list fits)
- [ ] #3 Works in the register edit dialog, Review and the Rules dialog, at desktop and phone widths
- [ ] #4 Tests cover scroll-into-view on ArrowDown/ArrowUp/Home/End; a preview state shows the picker open inside a short dialog; describePreviewA11y passes
<!-- AC:END -->
