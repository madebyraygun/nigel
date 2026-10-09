---
id: TASK-144.16
title: RemoteTransport and the connection flow
status: To Do
assignee: []
created_date: '2026-10-07 18:46'
labels:
  - swift
  - macos
  - multiuser
milestone: m-3
dependencies:
  - TASK-144.3
  - TASK-32.2
  - TASK-32.9
parent_task_id: TASK-144
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Resolve nigel:// against a remote nigel serve, proxying the SPA from the server so page and API never disagree on version. Connection settings take a URL, username and password; the session credential from TASK-32.2 lives in the Keychain and is attached natively so the page never holds it. A visible mode indicator shows local or remote. Replaces TASK-33.7 and 33.8 on the Mac.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The Mac app connects and logs in to a remote server and drives every screen against it
- [ ] #2 Switching between local books and a server is explicit, with a visible indicator
- [ ] #3 Local and remote data are never mixed in one view
- [ ] #4 An unreachable or restarted server degrades to a retry, never silent staleness
- [ ] #5 Roles from TASK-32.3 are respected: a read-only user sees no write controls
<!-- AC:END -->
