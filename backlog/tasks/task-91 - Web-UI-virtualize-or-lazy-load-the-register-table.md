---
id: TASK-91
title: 'Web UI: virtualize or lazy-load the register table'
status: To Do
assignee: []
created_date: '2026-08-12 17:50'
labels:
  - web
  - ui
  - performance
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The register renders every row at once — an All Transactions view of 1,872 rows is fully materialized in the DOM and scrollable end to end. Investigate row virtualization (or windowed rendering) for wc-register-table so DOM size stays bounded; keyboard navigation, inline editing, search jump and scroll-to-today must keep working across virtual boundaries. Measure before/after on a multi-thousand-row register.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 DOM node count for the register stays bounded regardless of row count
- [ ] #2 Keyboard navigation, inline editing, search and scroll-to-today behave identically to the unvirtualized table
- [ ] #3 Before/after render and scroll performance is measured and recorded on a 1,800+ row register
<!-- AC:END -->
