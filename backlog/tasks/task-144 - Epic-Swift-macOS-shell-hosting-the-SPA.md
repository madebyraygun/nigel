---
id: TASK-144
title: 'Epic: Swift macOS shell hosting the SPA'
status: To Do
assignee: []
created_date: '2026-09-15 16:05'
labels:
  - epic
  - swift
  - macos
dependencies: []
documentation:
  - docs/superpowers/specs/2026-09-14-swift-shell-design.md
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A native macOS shell in Swift that hosts the existing SPA in a WKWebView and owns everything around it: menu bar, sidebar, toolbar, command palette, menu bar extra, notifications, file dialogs, window lifecycle, unlock and updates. The screens stay web. On macOS it replaces the Tauri shell; Tauri stays for Windows and Linux.

Design: docs/superpowers/specs/2026-09-14-swift-shell-design.md. The transport is decision-1's, unchanged: the SPA and the JSON API over one in-process custom scheme, answered by build_desktop_router through a one-method UniFFI crate. The SPA's only shell knowledge is a hosted-mode flag, the nav registry it posts, and the invoke/listen seam DesktopApiClient already takes.

A full SwiftUI port was sized and rejected (about 2.5 to 3 times the wren app and a permanent second frontend). This shell absorbs the chrome-shaped subtasks of TASK-33 (33.4, 33.5, 33.14, 33.19, 33.20, 33.22 through 33.27) with public APIs, which also un-forecloses the App Store that 33.26's private-API route would have closed.

Phases: 1 host (parity with Tauri), 2 chrome, 3 beyond Tauri, 4 retire Tauri on macOS. Remote transport and iOS stay with TASK-33.8 and TASK-33.18; the Transport protocol this epic introduces is what those build on.

Constraints: the shell cannot be built or run on the Linux development machine; CI compiles it on a macOS runner and signs or publishes nothing (decision-3). Each phase ends with device-only checks that need a person at a Mac.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Phases 1 through 4 are complete: the Swift shell reaches parity with the Tauri shell, ships the chrome and the native affordances the spec lists, and Tauri is gated to Windows and Linux
- [ ] #2 The SPA's shell contract is exactly the spec's five-row table; no @nigel/ui component detects the host
- [ ] #3 docs/desktop.md, docs/native-feel.md, docs/architecture.md and README describe the Swift shell as the macOS app
<!-- AC:END -->
