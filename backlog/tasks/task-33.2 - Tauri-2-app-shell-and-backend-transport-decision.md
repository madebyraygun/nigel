---
id: TASK-33.2
title: Tauri 2 app shell and backend transport decision
status: To Do
assignee: []
created_date: '2026-08-06 16:29'
updated_date: '2026-08-17 00:20'
labels:
  - tauri
dependencies:
  - TASK-33.1
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Scaffold the Tauri 2 app hosting the SPA over the transport settled in decision-1: one custom URI scheme serving both the embedded SPA and the JSON API from the same build_router() that nigel serve builds, driven by register_asynchronous_uri_scheme_protocol rather than a TcpListener. No TCP port and no CORS, because the app and its API are same-origin. FetchApiClient changes by one constructor argument, computed at runtime because the URL form differs by platform (nigel://localhost on macOS and Linux, http://nigel.localhost on Windows). Document the desktop dev workflow.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The desktop app boots the SPA against a local database end to end
- [x] #2 The transport decision is recorded in backlog/decisions with rationale
- [ ] #3 The api client seam holds: web mode still works from the same SPA code
- [ ] #4 Desktop dev workflow is documented
- [ ] #5 Download behaviour under the custom scheme is verified on macOS, Windows and Linux before the shell is built around it; if a webview refuses <a download>/Content-Disposition, exportUrl's contract changes in the api seam rather than in screens
- [ ] #6 The Host/Origin guard takes a configured trusted origin instead of a hardcoded loopback list, with tests for both the desktop and the loopback forms
- [ ] #7 The desktop router is constructed without the session guard, as a property of construction rather than a runtime flag, and that is tested
- [ ] #8 The scheme is not registered as a deep link
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
AC #5, macOS leg (2026-08-16, WKWebView): matches Linux on every deciding case. Every navigation route fails with 'Download failed with error: unsupported URL' — WKWebView's download machinery rejecting the nigel:// scheme itself, before any save dialog, which is why Requested never fires for the anchor cases or window.open. Blob-with-download works and reports success=true; fetch works and exposes Content-Disposition; the native write via a Rust command works. Note the success flag is not portable: macOS reports true where Linux reports false for a byte-identical file, so the file is the evidence and the flag is not. Two platforms therefore settle the shape: exportUrl cannot keep returning an address in the desktop build, and the repair belongs in web/apps/app/src/api/client.ts with screens unchanged. Of the two working save routes, the Rust command is the better seam — its success is a Result rather than a flag the platforms disagree about, and it is the only one that can put the file where the user chose, so a save dialog belongs there. Windows (WebView2, http://nigel.localhost — a real HTTP origin) is outstanding and is the platform most likely to download normally; that would not change the seam, only make uniformity a choice rather than a necessity. Case 5 (inline PDF) is unobserved on both platforms: auto mode never reaches it because case 2 navigates the page away first, so it needs a manual click.
<!-- SECTION:NOTES:END -->
