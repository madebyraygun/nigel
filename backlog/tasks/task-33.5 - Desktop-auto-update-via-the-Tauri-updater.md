---
id: TASK-33.5
title: Desktop auto-update via the Tauri updater
status: To Do
assignee: []
created_date: '2026-08-06 16:29'
updated_date: '2026-08-18 02:04'
labels:
  - tauri
  - backend
milestone: m-0
dependencies:
  - TASK-33.2
references:
  - src/cli/update.rs
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Wire the Tauri updater plugin so the desktop app checks, downloads and installs updates with signature verification, while the CLI keeps its existing self_replace path in cli/update.rs unchanged.

The feed is the licensed one, not GitHub Releases. Updates are part of what a purchase buys — a perpetual license with twelve months of them — so the update manifest is served from the paid feed described in TASK-115.2 and presented with the licence token. An expired key means no new updates; it never means a dead app.

This repository's CI publishes neither the bundles nor the manifest, per decision-3, so the pipeline this consumes lives with the packaging work in TASK-33.6. The CLI's own release path is untouched: nigel binaries still come from this repository for all platforms.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 CLI self-update behavior is unchanged
- [ ] #2 The desktop updater reads the licensed feed and presents the licence token, and an expired licence stops updates without stopping the app
- [ ] #3 Neither the desktop bundles nor the update manifest are published by this repository's CI
- [ ] #4 The desktop app checks, downloads and installs updates with signature verification
<!-- AC:END -->
