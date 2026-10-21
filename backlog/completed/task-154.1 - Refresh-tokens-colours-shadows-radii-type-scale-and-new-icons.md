---
id: TASK-154.1
title: 'Refresh tokens: colours, shadows, radii, type scale and new icons'
status: Done
assignee: []
created_date: '2026-10-20 22:57'
updated_date: '2026-10-21 00:31'
labels:
  - web
  - design
  - theme
milestone: m-3
dependencies: []
references:
  - docs/design/app-refresh/README.md
parent_task_id: TASK-154
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extend @nigel/theme (tokens/color.ts, gradient.ts) with the handoff's token table (Design tokens section), mapped onto the existing --color-* roles rather than adding parallel names: ground, surface, subtle, sidebar, highlight, hover, selection, border, divider, text, muted, brand and on-brand, ring, field, income, expense, flag, blue, income and expense fills, the ink and pastel gradients, floating and modal shadows, backdrop. Add the radius scale and the type scale, with tabular numerals on figures. Add the icons the spec lists to web/packages/ui/src/icons/icons.ts: lock, lock-open, search, chevron-down, chevron-up-down, flag, plus, folder, monitor, keyboard, x.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 @nigel/theme carries every token in the handoff's table for light and dark, mapped onto existing role names, with no inline brand values left in @nigel/ui
- [x] #2 Text and muted text on every ground and surface meet WCAG AA contrast in both modes, checked by a test
- [x] #3 The eleven new icons exist in the icon set and render in the icon preview
- [x] #4 Every new or changed @nigel/ui component has a preview covering its states in light and dark, and describePreviewA11y passes with zero violations
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in PR #57.
<!-- SECTION:FINAL_SUMMARY:END -->
