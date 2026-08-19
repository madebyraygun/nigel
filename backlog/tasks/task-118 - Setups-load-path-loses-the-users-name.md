---
id: TASK-118
title: Setup's load path loses the user's name
status: To Do
assignee: []
created_date: '2026-08-19 15:02'
labels:
  - frontend
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The SPA setup flow (TASK-33.17) collects the user's name at the identity step, but the load-an-existing-directory exit only performs the data-directory switch: PUT /api/settings/app refuses userName, and POST /api/setup would create the books the user is trying to avoid creating. The typed name is dropped, and a load-path user is greeted namelessly until they set it by hand — which the web settings screen cannot do either.

Give the name somewhere to land on the load path: the natural shape is letting the app-settings write accept userName (it is settings.json data, not book data), with the setup screen sending it alongside the switch.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A user who loads existing books during setup is greeted by name afterwards
- [ ] #2 userName is writable from the web settings surface, with the TUI and CLI reading the same settings.json value
<!-- AC:END -->
