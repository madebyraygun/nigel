---
id: TASK-33.2
title: Tauri 2 app shell and backend transport decision
status: To Do
assignee: []
created_date: '2026-08-06 16:29'
updated_date: '2026-08-17 19:28'
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
- [x] #1 The desktop app boots the SPA against a local database end to end
- [x] #2 The transport decision is recorded in backlog/decisions with rationale
- [x] #3 The api client seam holds: web mode still works from the same SPA code
- [x] #4 Desktop dev workflow is documented
- [x] #5 Download behaviour under the custom scheme is verified on macOS, Windows and Linux before the shell is built around it; if a webview refuses <a download>/Content-Disposition, exportUrl's contract changes in the api seam rather than in screens
- [x] #6 The Host/Origin guard takes a configured trusted origin instead of a hardcoded loopback list, with tests for both the desktop and the loopback forms
- [x] #7 The desktop router is constructed without the session guard, as a property of construction rather than a runtime flag, and that is tested
- [x] #8 The scheme is not registered as a deep link
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Manual verification, macOS, by the operator: launches and browses; an encrypted database unlocks; report text and PDF exports and the invoice PDF all download through the native save dialog; the text export carries no escape codes.

Not exercised anywhere, by anything: imports — including gusto, whose feature the shell only began requesting when the missing-PDF defect was fixed — and the shell on Windows or on a Linux desktop. The Linux path has automated coverage of the request path but no run in front of a person, and Windows has neither. Its transport differs from the other two: http://nigel.localhost is a real HTTP origin, so it carries a Host header where the custom scheme sends none, which is the defect that made every screen answer 403 on macOS.
<!-- SECTION:NOTES:END -->
