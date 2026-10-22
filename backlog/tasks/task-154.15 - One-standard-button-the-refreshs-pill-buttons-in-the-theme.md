---
id: TASK-154.15
title: 'One standard button: the refresh''s pill buttons in the theme'
status: To Do
assignee: []
created_date: '2026-10-22 17:07'
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
The design system draws every button as a pill (radius list: '999px: pills, buttons, switches'), with a flat brand primary, an outline secondary, and danger variants. The theme's standard button (wa-button through controlsCss) still has 8px corners, its own heights, and a gradient brand treatment, so the refreshed screens worked around it: PR #67 drew native buttons in wc-action-pills, PR #63 added opt-in [pill]/[solid] attributes. The invoice editor's Create draft/Cancel then no longer matched New invoice.

Make the standard button the design's button, so every screen, dialog and editor matches with no per-component styling and no opt-ins.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every wa-button is a pill (999px): default height 32px, size=large 38px; text 13px (controls) with weight 500 on primary
- [ ] #2 variant=brand is a flat --br fill with --onbr text (no gradient); neutral/default is an outline in --bd; danger is an outline in --exp with --exp text, and a filled danger appearance exists for destructive confirms
- [ ] #3 Disabled buttons sit at 40% opacity with no hover change; :focus-visible draws a 2px --wa-color-focus outline offset 2px and survives forced colors
- [ ] #4 Icon-only buttons are 32px round with the hover fill
- [ ] #5 No screen or component styles buttons itself: wc-action-pills uses wa-button, the [pill]/[solid] opt-ins are gone, and a test fails the build if a component sets a button's radius, height or brand fill
- [ ] #6 Every preview that shows buttons passes describePreviewA11y in light and dark, contrast tests cover the brand fill, on-brand text and danger text, and docs/native-feel.md describes the button
<!-- AC:END -->
