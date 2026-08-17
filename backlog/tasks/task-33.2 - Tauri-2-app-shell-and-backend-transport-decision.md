---
id: TASK-33.2
title: Tauri 2 app shell and backend transport decision
status: To Do
assignee: []
created_date: '2026-08-06 16:29'
updated_date: '2026-08-17 19:26'
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
ACs #1, #4 and #8 landed in the desktop shell branch (PR #29). #1 is confirmed by the operator running the app on a real display: it launches, browses, and both text and PDF exports download through the native save dialog. #4 is docs/desktop.md plus a pointer row in CLAUDE.md's table. #8 is guarded by crates/nigel-desktop/tests/no_deep_link.rs, proven by registering a deep link and watching the test fail.

Four defects only a real webview could surface, each now with a regression test proven by making it fail. The router's guard refuses a request carrying no Host header, and a custom scheme is not HTTP so the webview sends none — every screen answered 403; the transport now carries the URI's authority into the header. The guard's origin check understood only http(s), so the page's own nigel://localhost origin was refused on every script and stylesheet — the shell loaded and hydrated into nothing; TrustedOrigins gained a list of whole origins matched verbatim. The report formatters are shared with the terminal, so a text export carried colour escapes into the downloaded file; build_desktop_router now disables them the way serve's entry point does. And the crate took nigel-core with default features off, so the UI reported that this build could not render a PDF; it now requests the same set the binary ships.

Worth recording for the next shell task: the integration tests all set a Host header explicitly and built their own TrustedOrigins, so none of them exercised the shape the webview actually sends. The shell and its tests now share one trusted_origins() function for that reason.
<!-- SECTION:NOTES:END -->
