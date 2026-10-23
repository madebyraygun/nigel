---
id: TASK-164
title: Bookkeeper access to self-hosted books in 2.0
status: To Do
assignee: []
created_date: '2026-10-23 17:18'
updated_date: '2026-10-23 19:47'
labels:
  - multiuser
  - web
  - docs
milestone: m-3
dependencies:
  - TASK-32.3
  - TASK-32.5
  - TASK-32.8
  - TASK-32.9
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
In 2.0 a bookkeeper reaches the operator's books by logging in to the operator's own nigel serve (epic 32): a named user with the bookkeeper role, at the server's public address (TASK-32.9), with every write attributed and audited. Nigel Cloud plays no part in 2.0 bookkeeper access; the end-to-end model that removes the need for a self-hosted server is 3.0 (decision-12, the end-to-end sync spec task).

This task makes that path a first-class, documented experience rather than an incidental consequence of multiuser: the operator invites a bookkeeper from Settings, the bookkeeper sets their own password through a one-time link, sees the bookkeeper role's screens, and the operator can see what they changed and revoke access.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 From Settings the operator invites a bookkeeper by email address and role; the invitee receives a one-time link to set their own password, and the link expires
- [ ] #2 The bookkeeper role can do the bookkeeping work (categorize, review, reconcile, import, reports) and cannot change users, security settings or service keys; read-only exists for an accountant who only looks
- [ ] #3 The operator sees each user's recent changes (from the audit log) and can revoke access, which ends that user's sessions at once
- [ ] #4 docs (deployment guide and README) walk through giving a bookkeeper access to a self-hosted server, end to end
<!-- AC:END -->
