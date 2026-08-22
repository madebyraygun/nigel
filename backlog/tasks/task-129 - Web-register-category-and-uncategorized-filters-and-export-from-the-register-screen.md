---
id: TASK-129
title: >-
  Web register: category and uncategorized filters, and export from the register
  screen
status: To Do
assignee: []
created_date: '2026-08-22 14:02'
labels:
  - frontend
  - api
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The register API deliberately refuses category and uncategorized filters — routes/reports.rs rejects them with "the category filters are CLI-only" (nigel-core/src/server/routes/reports.rs ~57-77, ~337) — and the register screen has no export control at all; the only web register export is Reports > Transaction Register, which carries account and period only. A user browsing "everything uncategorized in this account" and exporting it has a TUI/CLI path (nigel report register --category / --uncategorized) but no web path.

Prerequisite for deprecating the TUI dashboard. Filters should be deep-linkable like the existing account and period hash params, and the export should encode active filters in the default filename the way the CLI does.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The register API accepts category and uncategorized filters with the same semantics as the CLI flags
- [ ] #2 The register screen exposes both filters, composed with account and period, and reflects them in the hash for deep links
- [ ] #3 The register screen exports what it is currently showing to PDF and text, active filters encoded in the default filename
<!-- AC:END -->
