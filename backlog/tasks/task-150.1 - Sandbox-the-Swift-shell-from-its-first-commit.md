---
id: TASK-150.1
title: Sandbox the Swift shell from its first commit
status: To Do
assignee: []
created_date: '2026-10-07 18:45'
labels:
  - swift
  - macos
  - appstore
  - security
milestone: m-3
dependencies: []
parent_task_id: TASK-150
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Enable App Sandbox in the XcodeGen spec before TASK-144.3's scaffold lands, so every native affordance in epic 144 is built and tested under it rather than retrofitted. Entitlements: com.apple.security.app-sandbox, com.apple.security.files.user-selected.read-write, com.apple.security.files.bookmarks.app-scope, com.apple.security.network.client. Not network.server (the Mac app never hosts a server for others; that is headless nigel serve). No temporary-exception entitlements.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The shell launches, unlocks, imports, exports and sends under the sandbox
- [ ] #2 The entitlements file is committed and contains only the entitlements above
- [ ] #3 CI compiles the sandboxed configuration on the macOS runner and signs nothing (decision-3)
<!-- AC:END -->
