---
id: TASK-33.1
title: 'Workspace restructure: nigel-core, nigel-cli, nigel-desktop'
status: To Do
assignee: []
created_date: '2026-08-06 16:29'
updated_date: '2026-08-16 23:45'
labels:
  - tauri
  - backend
dependencies:
  - TASK-31.1
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split the crate into a cargo workspace: nigel-core holds the data layer, importers, reports, migrations, and settings with no ratatui/crossterm/clap dependencies; the nigel crate keeps the CLI/TUI binary (name and behavior unchanged); nigel-desktop is the Tauri shell scaffold. Builds on the lib/bin split from the web epic. Release workflows keep producing the same CLI artifacts.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Workspace builds with core free of TUI and CLI dependencies
- [ ] #2 The nigel binary keeps its name, features, and behavior; cargo test passes across the workspace
- [ ] #3 Release CI still produces the existing CLI binaries for all platforms
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The workspace half landed in PR #24: nigel-core and nigel are separate crates, core builds free of clap/ratatui/crossterm, the binary keeps its name, features and behaviour, and the release workflow's build path was run to confirm it still produces target/<triple>/release/nigel. AC #1 and #2 are met. The task stays open for the nigel-desktop scaffold it also names, which lands with the app shell and the custom-scheme transport.
<!-- SECTION:NOTES:END -->
