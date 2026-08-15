---
id: TASK-33.8
title: Desktop remote mode against a standalone nigel serve
status: To Do
assignee: []
created_date: '2026-08-15 23:38'
labels:
  - tauri
  - backend
  - frontend
dependencies:
  - TASK-33.2
parent_task_id: TASK-33
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Point the desktop client at a nigel serve running somewhere else — a home server, a box on a tailnet — instead of its own embedded database. Single-user: the instance is the operator's own, so this needs none of the multiuser epic.

decision-1 makes the client half cheap. FetchApiClient already takes a baseUrl, so a remote backend is that base URL pointed at a real HTTP origin rather than the custom scheme, and the same sixty typed methods, download links and preview frames work unchanged.

The server half is the actual work, because nigel serve is deliberately unreachable from another machine:

- It binds 127.0.0.1 only, and the Host/Origin guard accepts nothing but loopback, so today a remote instance is reachable only behind a reverse proxy that rewrites Host and Origin — which is a working arrangement, not a supported one, and it defeats the rebinding defence at the same time.
- The session token is minted per run and printed to stdout, so a client on another machine has no way to obtain one, and a restart invalidates whatever it was given.

So this task settles how a remote instance is addressed and how a client authenticates to it across restarts, and either supports that posture in the server or documents the proxy arrangement as the supported one. decision-1's configured trusted origin is the same knob the first half turns.

Distinct from task-33.7, which is the same screens against a shared multiuser server with real logins and is gated on task-32.2. This one is gated on nothing but the transport, and 33.7 should build on whatever connection UI, mode indicator and offline handling land here.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The desktop app connects to a nigel serve on another machine and drives every screen against it
- [ ] #2 How a remote instance is addressed and authenticated across server restarts is decided and documented, including whether the server gains a bind/trusted-host option or a reverse proxy is the supported arrangement
- [ ] #3 Switching between the embedded database and a remote instance is explicit, with a visible indicator of which one is in use
- [ ] #4 Local and remote data are never mixed in one view
- [ ] #5 An unreachable or restarted server degrades gracefully with a retry, never silent staleness
<!-- AC:END -->
