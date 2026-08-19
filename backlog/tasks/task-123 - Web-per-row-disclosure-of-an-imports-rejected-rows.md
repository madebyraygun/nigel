---
id: TASK-123
title: 'Web: per-row disclosure of an import''s rejected rows'
status: To Do
assignee: []
created_date: '2026-08-19 20:34'
labels:
  - frontend
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
TASK-52 landed rejects end to end: the schema records each dropped row with its reason, GET /api/imports/{id}/rejects serves them, the CLI reads them (nigel imports rejects <id>), and the web shows counts (the import history's Dropped column and the dashboard notice). What the web cannot do is show WHICH rows dropped — the dashboard tells a browser user their books are incomplete and offers only counts. Design a per-row disclosure in wc-import-history: expanding an import with a non-zero Dropped count fetches its rejects and shows line number, raw content, and reason. Component-first: preview states for loading/populated/empty-error, a11y across them; the client method to reintroduce is the getImportRejects shape that TASK-50/51/52's branch built and removed as dead code (see that branch's api history for the exact wire type).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Expanding a history row with dropped rows shows each reject's line number, content and reason, fetched on demand
- [ ] #2 Loading and error states are visible states of the component with a11y coverage
- [ ] #3 The dashboard notice's journey no longer dead-ends at counts
<!-- AC:END -->
