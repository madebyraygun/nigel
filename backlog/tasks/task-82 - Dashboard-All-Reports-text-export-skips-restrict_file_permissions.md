---
id: TASK-82
title: Dashboard All-Reports text export skips restrict_file_permissions
status: To Do
assignee: []
created_date: '2026-08-11 06:02'
labels: []
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The dashboard's All-Reports text export (do_text_export in src/cli/dashboard.rs) writes each report file without calling crate::settings::restrict_file_permissions, unlike the CLI bulk path (export_all_text in src/cli/report/mod.rs), which restricts both the directory and every file it writes. Financial exports produced from the TUI land world-readable on multi-user systems. The two paths are otherwise deliberately mirrored, so the fix is to restrict the directory and each written file the same way the CLI path does.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Dashboard All-Reports text export restricts permissions on the export directory and every file it writes, matching export_all_text
- [ ] #2 A test pins the permission bits on the dashboard path's output
<!-- AC:END -->
