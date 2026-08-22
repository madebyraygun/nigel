---
id: TASK-33.21
title: Let demo users return to Setup
status: To Do
assignee: []
created_date: '2026-08-20 21:47'
updated_date: '2026-08-21 00:21'
labels:
  - frontend
  - ui
  - onboarding
  - demo
milestone: m-0
dependencies: []
parent_task_id: TASK-33
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
After choosing View the demo during onboarding, Nigel boots into initialized demo books and the setup screen becomes unreachable because initialized currently implies ready. In both the local macOS app and the browser UI, demo mode should carry a persistent escape hatch: a Return to Setup action pinned to the upper-right corner. Re-entering setup must explicitly recognize the initialized-demo state, rather than treating it as ordinary completed onboarding, so someone can move from exploring the demo to starting fresh or loading existing books without using the CLI. Ordinary initialized books must not be offered this reset path.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 While Nigel is using the demo data directory, a Return to Setup action is pinned to the upper-right corner in both the macOS desktop app and the web app
- [ ] #2 The action is absent for ordinary initialized books and does not displace or scroll with the current screen's content
- [ ] #3 Activating Return to Setup launches the shared start screen even though the demo database is already initialized; the boot/setup state explicitly distinguishes initialized demo books from completed onboarding
- [ ] #4 From the reopened start screen, starting fresh or loading an existing data directory completes normally and leaves demo mode, while choosing the demo again remains safe and idempotent
- [ ] #5 Re-entering or cancelling the start flow does not delete or mutate the demo database before a replacement choice succeeds, and the behavior is covered across shared SPA state plus desktop and browser client paths
<!-- AC:END -->
