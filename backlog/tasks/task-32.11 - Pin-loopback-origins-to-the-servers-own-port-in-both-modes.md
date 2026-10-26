---
id: TASK-32.11
title: 'Pin loopback origins to the server''s own port, in both modes'
status: To Do
assignee: []
created_date: '2026-10-26 04:00'
labels:
  - multiuser
  - security
  - web
milestone: m-3
dependencies: []
parent_task_id: TASK-32
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
nigel serve trusts a loopback Origin on any port, in local and shared mode. A page served from another port on the same machine is same-site, so it can carry the Strict session cookie and a trusted Origin; JSON routes are protected by the CORS preflight, but body-less POSTs and multipart uploads are simple requests. Pinning only shared mode would break the vite dev loop (its proxy rewrites Host but not Origin) and leave local mode open.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A loopback Origin is trusted only when its port is the server's own port, in local and shared mode
- [ ] #2 The vite dev proxy rewrites Origin to the backend so the documented dev loop (npm run dev on :5173 against nigel serve) keeps working
- [ ] #3 Unsafe methods require a custom request header the SPA always sends (defence in depth), or the decision not to is recorded
- [ ] #4 Tests cover a foreign loopback port refused and the dev loop accepted; docs/api.md security model updated
<!-- AC:END -->
