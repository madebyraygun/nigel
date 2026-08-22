---
id: TASK-126
title: 'Two clamp_day functions, one hardened and one not'
status: To Do
assignee: []
created_date: '2026-08-20 21:48'
labels:
  - invoicing
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The recurring-schedules work added nigel_core::invoicing::schedules::clamp_day and hardened its month-out-of-range fallback to refuse rather than silently answer 31. crates/nigel/src/cli/demo.rs carries a private clamp_day with the original un-gated fallback: written for December, it fires for any month chrono refuses and quietly returns 31.

Not a live bug — demo.rs's only caller (make_date) passes 1-12 — but the codebase now holds two definitions of one function with different safety properties, which is exactly the drift that makes a later reader trust the wrong one. Delete the demo.rs copy and call the shared pub fn.

Sequencing note: demo.rs is relocated to nigel-core by the first-run onboarding branch, so do this after that lands to avoid a rename-plus-edit conflict.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 One clamp_day exists in the workspace and every caller uses it
- [ ] #2 The surviving implementation refuses an out-of-range month rather than answering 31
<!-- AC:END -->
