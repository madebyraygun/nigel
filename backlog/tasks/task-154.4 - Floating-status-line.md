---
id: TASK-154.4
title: Floating status line
status: To Do
assignee: []
created_date: '2026-10-20 22:57'
labels:
  - web
  - design
milestone: m-3
dependencies:
  - TASK-154.2
references:
  - docs/design/app-refresh/README.md
parent_task_id: TASK-154
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Spec section 3. A second inset panel under the content: the pastel nigel block, the open book (monogram and name; opens the palette), the folder path (selectable), lock state, then '▲ n to review' (to Review), 'Cmd+K palette' and the version. /api/status supplies the path, lock state and version. Hidden in the Swift shell's hosted mode and replaced by the book card in the mobile drawer.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The status line shows book, path, lock state, review count, palette hint and version, each segment acting as the spec says
- [ ] #2 The version segment is omitted in a store build (TASK-150.4 distribution flag), and hosted mode hides the whole line
- [ ] #3 Every new or changed @nigel/ui component has a preview covering its states in light and dark, and describePreviewA11y passes with zero violations
<!-- AC:END -->
