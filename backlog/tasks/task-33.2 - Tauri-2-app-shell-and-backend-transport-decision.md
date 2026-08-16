---
id: TASK-33.2
title: Tauri 2 app shell and backend transport decision
status: To Do
assignee: []
created_date: '2026-08-06 16:29'
updated_date: '2026-08-16 23:59'
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
AC #5, Linux leg (2026-08-16, WebKitGTK 2.52.3, headless): navigation to a custom-scheme URL does not download. DownloadEvent::Requested never fires for an anchor — with or without the download attribute, with or without Content-Disposition — nor for window.open; Finished reports success=false with no path and no file appears. Because Requested is a webview-level decision taken before any save dialog, the headless environment does not explain it. Fetch-then-blob is the only route that wrote a correct file. A plain fetch reads both the bytes and the Content-Disposition header, so a filename can be recovered rather than guessed. Note that the blob case reports success=false while writing a correct file: trust the file, not the flag. macOS (WKWebView) and Windows (WebView2) still to run — the probe is on branch probe/33.2-download-scheme under probes/download-scheme, with a README carrying the case table and what to record. If the pattern holds, exportUrl stops returning an address under the desktop transport and the repair lands in web/apps/app/src/api/client.ts, leaving screens binding one thing to one attribute as AC #5 requires.
<!-- SECTION:NOTES:END -->
