---
id: TASK-144.5
title: >-
  Hosted mode in the SPA: hide the rail, post the nav registry and the current
  screen
status: To Do
assignee: []
created_date: '2026-09-15 16:06'
labels:
  - web
  - ui
dependencies:
  - TASK-144.4
documentation:
  - docs/superpowers/specs/2026-09-14-swift-shell-design.md
parent_task_id: TASK-144
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 1. With ?shell=native on the initial URL the SPA hides its sidebar rail and nav toggle, posts its nav registry (id, label, order, profile-filtered) through invoke('nav', items) on boot and whenever it changes, and posts navigated {screen, title} on every hash change. This is the whole SPA change for the epic: one query flag, two informational messages and the createApiClient detection from the bridge task. No wc-* component learns it is hosted; the flag is read in nigel-app and the messages go through the api client seam.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Under ?shell=native the sidebar rail and nav toggle are not rendered and the content area takes the full width; without it nothing changes
- [ ] #2 nav is posted on boot and when the registry changes (profile switch); navigated is posted on every hash change, with the screen title
- [ ] #3 No file under web/packages/ui reads the flag or the host; web tests cover both messages through the fake api client
<!-- AC:END -->
