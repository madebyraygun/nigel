---
id: TASK-144.3
title: >-
  Scaffold apps/Nigel: Xcode project, NigelKit with a Transport protocol, and a
  WebHost that loads the SPA
status: To Do
assignee: []
created_date: '2026-09-15 16:06'
labels:
  - swift
  - macos
dependencies:
  - TASK-144.2
documentation:
  - docs/superpowers/specs/2026-09-14-swift-shell-design.md
parent_task_id: TASK-144
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 1. apps/Nigel is an XcodeGen project (macOS 15+, Swift 6 strict concurrency) with Packages/NigelKit, a platform-neutral SwiftPM package holding the Transport protocol (a request in, a response out, keyed by the nigel://localhost URL) and InProcessTransport over NigelHost behind a Swift actor. Nigel/Web holds WebHost (one WKWebView per window with nigel:// registered on its configuration) and SchemeHandler, which hands every request to the transport and streams the response back. The window opens at nigel://localhost/?shell=native and shows the SPA. No cookies, no session, no CORS: the origin is the process, exactly as with Tauri. The Transport protocol is the seam TASK-33.8's remote mode and TASK-33.18's iOS client build on, so it is in from the first commit.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The app opens a window showing the SPA served by the in-process router, with API calls answered on the same origin
- [ ] #2 Transport is a protocol in NigelKit with InProcessTransport as its only implementation; SchemeHandler is tested against an in-memory transport
- [ ] #3 NigelKit has no AppKit or UIKit imports
- [ ] #4 Swift tests run through build-ffi.sh --test; the project builds warning-free
<!-- AC:END -->
