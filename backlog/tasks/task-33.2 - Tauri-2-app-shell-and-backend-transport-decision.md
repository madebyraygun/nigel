---
id: TASK-33.2
title: Tauri 2 app shell and backend transport decision
status: To Do
assignee: []
created_date: '2026-08-06 16:29'
updated_date: '2026-08-17 00:49'
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
AC #5, Windows leg (2026-08-16, WebView2 on GitHub windows-latest): Windows is the mirror image of the other two. Its custom scheme is http://nigel.localhost, a real HTTP origin, and WebView2 downloads from it normally — case 1 fired Requested, resolved report.csv out of Content-Disposition on its own, and wrote the file with success=true and a real path. But the blob route that works on Linux and macOS produced no event and no file on Windows. Case 9, handing the bytes to a Rust command, works on all three. AC #5 is answered: navigation works on exactly one platform, the blob on exactly two and not the same two, and only the native write is portable. The client-side blob fallback that looked like the obvious repair after the Linux and macOS legs would have broken on Windows, and neither of those platforms could have revealed it — which is the case for having run all three before building the shell. Decision: the desktop build fetches bytes and hands them to Rust, which writes the file; the seam is web/apps/app/src/api/client.ts and screens are unchanged; the web build keeps today's href behaviour. Two properties come free: the outcome is a Result rather than a success flag the platforms disagree about, and the destination is ours, so a save dialog can put the file where the user wants it. Still unobserved: case 5 (inline PDF) on every platform, which needs a manual click and governs the invoice preview; and cases 3, 4 and 6 on a real Windows desktop rather than CI, which would at most add a second working option.
<!-- SECTION:NOTES:END -->
