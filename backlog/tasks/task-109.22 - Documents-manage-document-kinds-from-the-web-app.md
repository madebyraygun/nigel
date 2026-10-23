---
id: TASK-109.22
title: 'Documents: manage document kinds from the web app'
status: To Do
assignee: []
created_date: '2026-10-23 23:29'
labels:
  - web
  - documents
milestone: m-3
dependencies: []
parent_task_id: TASK-109
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Document kinds can only be added, renamed, deactivated and reactivated from the CLI (nigel document kinds …). The web app shows an Inactive kinds panel with Reactivate, but offers no way to add a kind or stop offering one, so a web-only user cannot manage the list at all.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Documents (or Settings, wherever the kinds list reads best) lists every kind, active and inactive, with its document count
- [ ] #2 Add a kind from the web app; duplicate or empty names are refused with a clear message
- [ ] #3 Rename a kind from the web app
- [ ] #4 Deactivate an active kind (it stops being offered for new documents; existing documents keep it) and reactivate an inactive one
- [ ] #5 Uses the existing API if it covers these actions, otherwise adds the endpoints with tests and documents them in docs/api.md
- [ ] #6 Built from @nigel/ui components with previews and describePreviewA11y; docs/architecture.md and README.md describe it
<!-- AC:END -->
