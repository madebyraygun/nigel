---
id: TASK-144.13
title: Keychain and Touch ID unlock
status: To Do
assignee: []
created_date: '2026-09-15 16:07'
labels:
  - swift
  - macos
  - security
dependencies:
  - TASK-144.4
documentation:
  - docs/superpowers/specs/2026-09-14-swift-shell-design.md
parent_task_id: TASK-144
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 3. When /api/status reports an encrypted database, the shell calls POST /api/unlock itself before the page loads, with the password from an opt-in Keychain item released by Touch ID through LocalAuthentication. The SPA's unlock screen remains the fallback and the browser path. The password never touches disk outside the Keychain and a clear toggle removes the item. Absorbs TASK-33.4 for macOS.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 With remembering on, launch unlocks after Touch ID and the page never shows the unlock screen; with it off or on failure, the SPA's unlock screen appears
- [ ] #2 Turning remembering off removes the Keychain item; the password is never written anywhere else
- [ ] #3 TASK-33.4 is closed for macOS against this task
<!-- AC:END -->
