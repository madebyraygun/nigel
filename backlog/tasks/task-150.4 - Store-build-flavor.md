---
id: TASK-150.4
title: Store build flavor
status: To Do
assignee: []
created_date: '2026-10-07 18:46'
labels:
  - appstore
  - rust
  - swift
  - web
milestone: m-3
dependencies:
  - TASK-144.2
parent_task_id: TASK-150
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A store cargo feature on nigel-core / nigel-ffi, and a matching Xcode configuration. It compiles out the self-update path (updater.rs, update_available in AppState and /api/status), Sparkle, and any license-key requirement or license screen. /api/status reports the distribution (store, direct or source) so the SPA hides update and licensing UI without detecting its host. The direct and source builds are unchanged.

Nigel Cloud stays in the store build (decision-9): sign in to an existing nigel.works account only, with no pricing, purchase or subscribe links or calls to action (guideline 3.1.3(b)). The flavor keeps a switch that compiles Cloud out, the fallback if App Review requires in-app purchase.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A test asserts the store feature contains no reachable update check or download
- [ ] #2 The store build shows no license prompt and no update or licensing UI
- [ ] #3 The store build offers Cloud sign-in with no pricing, purchase or subscribe links
- [ ] #4 A build switch removes Cloud from the store flavor entirely
- [ ] #5 Direct and source builds are unaffected in behavior
<!-- AC:END -->
