---
id: TASK-144.14
title: Sparkle updater hook against the licensed feed
status: To Do
assignee: []
created_date: '2026-09-15 16:07'
labels:
  - swift
  - macos
dependencies:
  - TASK-144.3
documentation:
  - docs/superpowers/specs/2026-09-14-swift-shell-design.md
parent_task_id: TASK-144
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 3. A Sparkle integration whose feed URL is configuration the paid pipeline supplies, per decision-3 and TASK-115.2; the repository carries the hook, a Check for Updates... item, and no feed, key or signing step. Absorbs TASK-33.5 for macOS.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Check for Updates... in the app menu drives Sparkle against a feed URL read from configuration; with none configured the item is disabled
- [ ] #2 No feed URL, public key or signing identity is committed; CI is unchanged
- [ ] #3 TASK-33.5 is closed for macOS against this task
<!-- AC:END -->
