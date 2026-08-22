---
id: TASK-33.28
title: Import history's count column joins the figures face
status: To Do
assignee: []
created_date: '2026-08-21 00:12'
updated_date: '2026-08-21 01:48'
labels:
  - ui
milestone: m-0
dependencies: []
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
TASK-33.15 split --wa-font-family-sans (system) from --wa-font-family-mono (IBM Plex Mono) and added --nc-font-figures for columns whose digits have to line up.

wc-import-history has two cells that match those rules and were deliberately left out, because PR #37 (import integrity) adds a td.dropped rule to the same styles block and a font-family change there would have been a merge conflict for no benefit.

Once #37 has landed:

- td.count / th.count keeps its tabular-nums but has no face. Give the td font-family: var(--nc-font-figures), scoped to the td — the th above it is a word, which is the mistake the first pass made in three other tables and had to come back for.
- The Imported cell is a date rendered bare, with neither a class nor a face. Give it a class and the same figures token, matching wc-payment-list td.date and wc-reconciliation-history td.when.

td.file already reads --wa-font-family-mono and is correct as it stands.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 wc-import-history's count column reads --nc-font-figures and names no font stack of its own
- [ ] #2 The font-stack guard test and the import-history tests still pass
- [ ] #3 wc-import-historys count column reads --nc-font-figures on the td only, not the th
- [ ] #4 The Imported date cell reads --nc-font-figures
- [ ] #5 The font-stack guard and the import-history tests still pass
<!-- AC:END -->
