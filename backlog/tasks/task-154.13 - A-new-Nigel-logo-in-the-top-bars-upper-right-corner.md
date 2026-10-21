---
id: TASK-154.13
title: A new Nigel logo in the top bar's upper-right corner
status: To Do
assignee: []
created_date: '2026-10-21 17:10'
labels:
  - web
  - design
  - brand
milestone: m-3
dependencies: []
references:
  - docs/design/app-refresh/README.md
parent_task_id: TASK-154
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Nigel 2.0 gets a new logo, shown in the upper-right corner of the app's top bar, beside the search pill and refresh. The design handoff leaves the app icon out of scope ('still in design'); this task covers the logo artwork arriving and being placed in the shell.

The artwork comes from design; this task takes the delivered SVG and wires it in. Until then nothing here starts. The sidebar header keeps its gradient wordmark unless the delivered design says otherwise.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The delivered logo is in @nigel/ui as an SVG component with light and dark variants drawn from theme tokens, with a preview and describePreviewA11y passing
- [ ] #2 The top bar shows it in the upper-right corner at the size and spacing the design gives, in the browser and in Tauri, and it is hidden or reduced on a phone as the design says
- [ ] #3 It carries an accessible name ('Nigel') or is marked decorative where the wordmark already names the app
- [ ] #4 The design source is committed under docs/design/ beside the app-refresh handoff
<!-- AC:END -->
