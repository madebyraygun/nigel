---
id: TASK-84
title: Period-stamp export filenames
status: To Do
assignee: []
created_date: '2026-08-11 15:57'
updated_date: '2026-08-21 00:21'
labels:
  - reports
milestone: m-0
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Export filenames are date-stamped (k1-prep-2026-08-05.pdf), so exporting two different periods the same day silently overwrites the first — now the headline use case of viewer exports (export 2025 then 2026 K-1). Stamp with the report period instead (e.g. k1-prep-FY2025.pdf, pnl-2025-03.pdf); falls back to run date for period-less reports (flagged, balance). Applies to src/cli/report/mod.rs and src/cli/export.rs paths, CLI and dashboard alike. Related: export::all fails fast on the first broken report (unlike the text bulk path, which skips and reports) — worth aligning while in these files.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Exporting the same report for two different periods on the same day produces two files
- [ ] #2 Period-less reports (flagged, balance) keep a sensible unique name
<!-- AC:END -->
