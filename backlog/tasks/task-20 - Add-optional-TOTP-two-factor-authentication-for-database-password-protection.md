---
id: TASK-20
title: Add optional TOTP two-factor authentication for database password protection
status: To Do
assignee: []
created_date: '2026-04-25 18:06'
updated_date: '2026-08-11 17:08'
labels: []
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/issues/166'
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

Add optional TOTP-based two-factor authentication as a second gate when unlocking an encrypted database. Originally issue #166; a first implementation (draft PR #170, branch `feature/166-totp-2fa`, March 2026) was never merged and has drifted ~300 commits behind — it predates the web server, so its design no longer covers every unlock surface. This task is the reimplementation against the current codebase; PR #170 is closed but its branch is kept as a design quarry.

## What exists to salvage from PR #170

- `src/totp.rs` — a self-contained ~390-line module (secret generation, otpauth URI, code verification via `totp-rs`, keyring-backed storage). Mostly reusable as-is.
- `docs/plans/2026-03-03-totp-2fa-design.md` and `-plan.md` on that branch — the TUI/UX flows (onboarding toggle, splash code prompt, settings management entry) still describe what we want the terminal side to feel like.

## Design questions to settle before implementation

1. **The web unlock must be covered or the feature is theater.** `POST /api/unlock` (added after PR #170) accepts the bare password; the SPA has an unlock screen. TOTP must gate that route too — likely an optional `totpCode` field, refused with a distinct error code when 2FA is on and the code is absent or wrong, drawing on the same throttling budget as failed passwords. `GET /api/status` (or the unlock error itself) needs to tell the SPA a code is required without advertising 2FA state to unauthenticated callers more than necessary.
2. **`NIGEL_DB_PASSWORD` unattended unlock** (cron backups, CI — added after PR #170) breaks under mandatory TOTP. Decide the policy explicitly: e.g. the env path stays password-only by design (documented as such — the threat model for a scheduled job on the owner's own machine differs from an interactive unlock), or an accompanying `NIGEL_TOTP_SECRET` is required. Silent bypass and silent breakage are both wrong; pick one on purpose and write it down.
3. **Secret storage.** The original issue proposed the `metadata` table inside the encrypted database; PR #170 built OS-keyring storage instead. They gate differently: a keyring secret is checked before the app proceeds but lives outside the database; an in-database secret can only be checked after the password already opened the file. Either way TOTP here is an app-level gate, not key material — be honest about that in the docs (someone with the password and `sqlcipher` bypasses it). Keyring needs a headless story (Linux without a secret service, servers); in-database needs nothing new but is the weaker gate. Decide and record the reasoning.
4. **Schema/migrations**: any schema change now lands as migration v6+ (the PR predates v2). Feature-flagging (`2fa` cargo feature) is still an open option but no longer obviously worth the cfg surface — decide.

## Scope (unchanged in spirit from the original)

Onboarding toggle with secret display and code verification; splash prompt after password acceptance; settings entry to enable/disable/regenerate (password manager requires a code while 2FA is active); CLI password prompts gain a code prompt when applicable; SPA unlock screen gains a code field.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A design decision record settles the storage (keyring vs in-database), the NIGEL_DB_PASSWORD policy, and the feature-flag question before code is written
- [ ] #2 Onboarding password step has an optional toggle to enable 2FA; when enabled it displays the TOTP secret (copyable; QR is a stretch goal) and verifies a code before proceeding
- [ ] #3 Splash screen prompts for a TOTP code after successful password entry when 2FA is active
- [ ] #4 POST /api/unlock requires a valid TOTP code when 2FA is active, refusing with a distinct machine-readable error, sharing the failed-attempt budget, and the SPA unlock screen collects it
- [ ] #5 The NIGEL_DB_PASSWORD unattended path behaves per the recorded policy and the docs state it explicitly
- [ ] #6 Settings screen gains a 2FA management entry (enable/disable/regenerate); password change/removal require a valid code while active, in the TUI and over HTTP
- [ ] #7 CLI subcommands that prompt for a password also prompt for a code when applicable
- [ ] #8 The TOTP secret never lands on disk in plaintext, and the docs state plainly that 2FA is an app-level gate, not encryption key material
- [ ] #9 Tests cover the unlock surfaces (TUI logic seams and server routes) for both the enabled and disabled states
- [ ] #10 Documentation updated (README, docs/api.md, CLAUDE.md), removing anything the old PR's design implied
- [ ] #11 IMPORTANT: Any PRs created from this task must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
