---
id: TASK-33.7
title: 'Tauri remote mode against a multiuser server (Windows, Linux)'
status: To Do
assignee: []
created_date: '2026-08-06 16:29'
updated_date: '2026-10-07 18:45'
labels:
  - tauri
  - multiuser
milestone: m-4
dependencies:
  - TASK-33.2
  - TASK-32.2
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Let the desktop app connect to a shared multiuser server instead of its local database: a connection settings screen (server URL plus login), the remote backend for the api client, a clear local-versus-remote indicator, and graceful offline/unreachable handling. Local and remote data never mix. Bridges this epic with the multiuser epic.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The desktop app can connect and log in to a remote multiuser server
- [ ] #2 Switching between local and remote profiles is explicit with a visible mode indicator
- [ ] #3 Unreachable server states degrade gracefully with retry, never data loss
- [ ] #4 Local and remote data are never mixed in one view
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The Mac gets this through the Swift shell (TASK-144.16). This task is the Tauri client on Windows and Linux, against epic 32's server.
<!-- SECTION:NOTES:END -->
