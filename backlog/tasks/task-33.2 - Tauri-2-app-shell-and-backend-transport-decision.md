---
id: TASK-33.2
title: Tauri 2 app shell and backend transport decision
status: To Do
assignee: []
created_date: '2026-08-06 16:29'
updated_date: '2026-08-17 03:36'
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
- [x] #3 The api client seam holds: web mode still works from the same SPA code
- [ ] #4 Desktop dev workflow is documented
- [x] #5 Download behaviour under the custom scheme is verified on macOS, Windows and Linux before the shell is built around it; if a webview refuses <a download>/Content-Disposition, exportUrl's contract changes in the api seam rather than in screens
- [x] #6 The Host/Origin guard takes a configured trusted origin instead of a hardcoded loopback list, with tests for both the desktop and the loopback forms
- [x] #7 The desktop router is constructed without the session guard, as a property of construction rather than a runtime flag, and that is tested
- [ ] #8 The scheme is not registered as a deep link
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
ACs #3, #6 and #7 landed in PR #25 (merge 08861d7). Exports are now an ExportTarget union the client picks the arm of — the web client always answers href so the browser keeps its real anchors, and a desktop client answers an action that fetches bytes and hands them to the native side, which the probe showed is the only portable save route. The type is declared in both the app and @nigel/ui because the UI package must not depend on the app; export-target-seam.test.ts fails on any drift between the two. TrustedOrigins replaces the hardcoded host list, loopback() holding exactly the previous three names and exactly() not implicitly trusting loopback. build_desktop_router carries no session layer as a property of construction, shares finish_router with build_router so the two cannot drift, and sits behind a  feature that is not in default — a session-guard-free router must not exist in a serve build, since the host guard does not defend a TCP bind. Remaining here: #1 the shell boots the SPA against a local database, #4 the dev workflow docs, #8 no deep-link registration.
<!-- SECTION:NOTES:END -->
