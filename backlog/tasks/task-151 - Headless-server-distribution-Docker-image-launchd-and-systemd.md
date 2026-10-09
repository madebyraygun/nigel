---
id: TASK-151
title: 'Headless server distribution: Docker image, launchd and systemd'
status: To Do
assignee: []
created_date: '2026-10-07 18:46'
labels:
  - server
  - ci
  - docs
milestone: m-3
dependencies:
  - TASK-32.9
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Make nigel serve easy to run on a home server or small VPS: a Docker image (linux amd64 and arm64) built by public CI, plus launchd and systemd examples. The database unlocks at boot from NIGEL_DB_PASSWORD supplied as a secret. A health endpoint for monitors. Decision-3 covers desktop installers only; the server is the CLI binary.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Public CI publishes the image on a release tag
- [ ] #2 The deployment guide (TASK-32.9) covers Docker, launchd and systemd with TLS
- [ ] #3 A health endpoint answers without authentication and reveals nothing about the books
<!-- AC:END -->
