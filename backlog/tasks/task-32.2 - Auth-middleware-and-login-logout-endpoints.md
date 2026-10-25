---
id: TASK-32.2
title: Auth middleware and login/logout endpoints
status: To Do
assignee: []
created_date: '2026-08-06 16:27'
updated_date: '2026-10-25 03:39'
labels:
  - multiuser
  - backend
milestone: m-3
dependencies:
  - TASK-32.1
parent_task_id: TASK-32
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Session-cookie authentication in front of the API when multiuser mode is enabled: login and logout endpoints, HttpOnly SameSite cookies, session expiry, and failed-attempt throttling with lockout. Multiuser mode is an explicit setting; when off, the current single-user tokenized-URL behavior is unchanged. In multiuser mode the database unlock endpoint becomes admin-only (the key is held server-side after an admin unlocks at boot).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Login and logout work with HttpOnly SameSite session cookies and server-side expiry
- [ ] #2 Failed logins are throttled and repeated failures lock the account temporarily
- [ ] #3 All /api routes require an authenticated session in multiuser mode; logout invalidates the session server-side
- [ ] #4 Sessions expire after 30 days idle and after 90 days regardless of activity
- [ ] #5 Local single-user mode stays the default with behaviour identical to before; shared mode begins automatically when the first user exists (no setting), and only shared mode accepts the server_public_url host
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Design: multiuser phase 1 spec (PR #74). Shared mode is derived from users existing, not an explicit setting.

Unlock is not admin-only over HTTP: a shared server unlocks at boot from a credential (NIGEL_DB_PASSWORD_FILE) and never over HTTP. Until roles (32.3) land, every non-auth route needs admin.

Design: docs/superpowers/specs/2026-10-23-multiuser-phase-1-design.md (approved, PR #74). Implementation sequence in §17.

Carried from PR #79's review, to do in PR 2: (1) fsync the config dir after server.key's hard_link, add the path to hard_link errors, test the race-loser and cleanup paths, and warn at serve boot if server.key's permissions are loose — all before any TOTP secret is sealed in production; (2) cap the login request body before verify_password / verify_dummy, and collapse verify_credentials into one verify call with dummy_phc warmed at startup; (3) cap the NIGEL_DB_PASSWORD_FILE read at a few KiB; (4) refuse TOTP enrollment for disabled users and tighten the TOTP key-error and clear tests; (5) exercise the sessions constraints (kind CHECK, token_hash UNIQUE, FK) once sessions are written over HTTP; (6) give Role Display/serde when the API serializes it; (7) create_session is called only after verify_credentials / verify_totp; (8) call local_mode before create_user so the CLI-flag wording never reaches the API.
<!-- SECTION:NOTES:END -->
