---
id: TASK-177
title: Revoked-access message tells web users to run the CLI
status: To Do
assignee: []
created_date: '2026-11-10 02:07'
labels:
  - web
  - cli
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
simplefin/client.rs returns "Run `nigel sync setup` with a new setup token." for revoked access, and the web API passes that 400 message straight to Settings, where there is no command line to run.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The revoked-access message reads correctly in both the CLI and the web app (e.g. "set up SimpleFIN again with a new setup token")
- [ ] #2 The test pinning the old wording is updated
<!-- AC:END -->
