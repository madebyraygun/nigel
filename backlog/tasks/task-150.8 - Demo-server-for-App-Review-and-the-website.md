---
id: TASK-150.8
title: Demo server for App Review and the website
status: To Do
assignee: []
created_date: '2026-10-07 18:46'
updated_date: '2026-10-07 18:46'
labels:
  - appstore
  - server
  - docs
milestone: m-3
dependencies:
  - TASK-32.3
  - TASK-151
parent_task_id: TASK-150
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A nigel serve with the demo database, real TLS, and a read-only reviewer account, reset on a schedule. The infrastructure lives outside this repository; this repository gets a runbook and, if needed, a demo-reset command.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A runbook in docs/ covers standing up, resetting and retiring the demo server
- [ ] #2 A read-only reviewer account can log in and see every screen with no write controls
<!-- AC:END -->
