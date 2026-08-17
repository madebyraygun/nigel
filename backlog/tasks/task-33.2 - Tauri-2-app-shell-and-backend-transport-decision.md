---
id: TASK-33.2
title: Tauri 2 app shell and backend transport decision
status: To Do
assignee: []
created_date: '2026-08-06 16:29'
updated_date: '2026-08-17 01:23'
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
- [x] #5 Download behaviour under the custom scheme is verified on macOS, Windows and Linux before the shell is built around it; if a webview refuses <a download>/Content-Disposition, exportUrl's contract changes in the api seam rather than in screens
- [ ] #6 The Host/Origin guard takes a configured trusted origin instead of a hardcoded loopback list, with tests for both the desktop and the loopback forms
- [ ] #7 The desktop router is constructed without the session guard, as a property of construction rather than a runtime flag, and that is tested
- [ ] #8 The scheme is not registered as a deep link
<!-- AC:END -->



## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
AC #5, case 5 (inline PDF): macOS renders it — a manual click puts the PDF's text on screen and no download event fires, so WKWebView opens its viewer rather than trying to save. The preview.pdf route works as designed there. Linux and Windows are unobserved: NIGEL_PROBE_MODE=case5 drives the case alone, but capturing it headlessly on this machine did not work, so it is left open rather than assumed. WebKitGTK has no built-in PDF viewer, making Linux the platform most likely to differ; the …/{number}/preview HTML route is the fallback if a platform will not render the PDF form. AC #5 is otherwise discharged on all three platforms.
<!-- SECTION:NOTES:END -->
