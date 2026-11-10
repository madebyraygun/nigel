---
id: TASK-176
title: 'SimpleFIN timer runbook: network ordering and the password-file note'
status: To Do
assignee: []
created_date: '2026-11-10 02:07'
labels:
  - docs
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
docs/runbooks/schedule-simplefin-sync.md uses `Wants=`/`After=network-online.target`, which has no effect in a user unit, so the first run after boot can fail before the network is up. Its commented `NIGEL_DB_PASSWORD_FILE` line works at head but not on older installed builds.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The runbook's unit waits for the network in a way that works for a user unit (or the service retries), and says which
- [ ] #2 The password-file line names the version that supports it
<!-- AC:END -->
