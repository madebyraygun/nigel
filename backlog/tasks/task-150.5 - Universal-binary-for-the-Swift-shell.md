---
id: TASK-150.5
title: Universal binary for the Swift shell
status: To Do
assignee: []
created_date: '2026-10-07 18:46'
labels:
  - swift
  - rust
  - ci
milestone: m-3
dependencies: []
parent_task_id: TASK-150
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Swift shell spec builds nigel-ffi for aarch64-apple-darwin only; release.yml already ships a universal CLI. Decide arm64-only or universal and record it on TASK-144.2. If universal, the xcframework carries both slices and CI compiles both.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The decision is recorded on TASK-144.2 and in the Swift shell spec
- [ ] #2 If universal, CI compiles both slices into the xcframework
<!-- AC:END -->
