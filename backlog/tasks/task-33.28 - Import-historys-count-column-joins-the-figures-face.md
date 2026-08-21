---
id: TASK-33.28
title: Import history's count column joins the figures face
status: To Do
assignee: []
created_date: '2026-08-21 00:12'
updated_date: '2026-08-21 00:21'
labels:
  - ui
milestone: m-0
dependencies: []
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
TASK-33.15 split --wa-font-family-sans (system) from --wa-font-family-mono (IBM Plex Mono) and added --nc-font-figures for columns whose digits have to line up. The rule it applied was: every selector already declaring font-variant-numeric: tabular-nums, plus explicit date cells.

wc-import-history's td.count matches that rule and was deliberately left out, because PR #37 (import integrity) rewrites wc-import-history.ts and a one-line font-family change there would have been a merge conflict for no benefit. Its td.file already reads the mono token and is unaffected.

Once #37 has landed, add font-family: var(--nc-font-figures) to the td.count / th.count rule so the import counts align with every other figure column.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 wc-import-history's count column reads --nc-font-figures and names no font stack of its own
- [ ] #2 The font-stack guard test and the import-history tests still pass
<!-- AC:END -->
