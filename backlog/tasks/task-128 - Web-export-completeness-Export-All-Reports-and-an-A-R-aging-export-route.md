---
id: TASK-128
title: 'Web export completeness: Export All Reports and an A/R aging export route'
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
The TUI report picker ends with Export All Reports — every report for the chosen period as PDF (Enter) or text (t) in one go — and the A/R aging viewer exports like any other report. The web has neither: /api/exports carries only the eight report slugs (nigel-core/src/server/routes/exports.rs), the aging view says so outright (web/apps/app/src/screens/reports.ts ~630, "Aging is not an export route... so there is no file to link to"), and no screen offers a bundle export. The year-end "dump everything" flow is TUI-only.

Prerequisite for deprecating the TUI dashboard: this is one of the flows with no web path. Aging likely wants an export route like its siblings; the bundle could be a zip download or a sequence of downloads — decide during design. Desktop should route through the native save dialog (nigel-desktop/src/save.rs) like existing exports.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A/R aging exports to PDF and text from the web, with the same export links the other reports carry
- [ ] #2 A single affordance on the Reports screen exports every report for the chosen period, PDF or text, matching the TUI's Export All matrix (profile-aware: K-1 only for business books)
- [ ] #3 Desktop saves through the native dialog like existing report exports
<!-- AC:END -->
