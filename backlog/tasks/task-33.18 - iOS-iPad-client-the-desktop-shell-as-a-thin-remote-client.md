---
id: TASK-33.18
title: 'iOS (iPad) client: the desktop shell as a thin remote client'
status: To Do
assignee: []
created_date: '2026-08-19 20:55'
labels:
  - tauri
dependencies:
  - TASK-33.8
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
An iPad build of the desktop shell, as a **thin remote client only**: the app hosts the SPA and points it at a `nigel serve` elsewhere (a home server, a box on the tailnet) using the connection flow, credential storage and offline handling TASK-33.8 establishes. No local database is created on the device — the iPad is a window onto the books, not a host for them.

The architecture already permits this. Tauri 2's iOS support rides on `WKURLSchemeHandler`, the same mechanism behind the `register_asynchronous_uri_scheme_protocol` transport decision-1 chose, so the shell's serving model carries over; the SPA is responsive and touch-capable; the api seam means every screen works against the remote backend unchanged.

## The work

1. **Target bootstrap.** `tauri ios init`, the `aarch64-apple-ios` target, Xcode project, signing. Distribution is governed by decision-3 — this repository's CI does not package, sign or publish installers, and that applies to iOS exactly as to desktop. CI keeps *compiling* what it can; running on the operator's own iPad uses a personal provisioning profile or TestFlight, outside this repo.
2. **Plugin and capability audit.** Desktop-only shell code — native menus, window management, the desktop save path — gets `cfg`-gated so the desktop builds are byte-for-byte unaffected and the iOS build compiles without them.
3. **The export probe, before any screens.** Same discipline as decision-1's download probe: verify how a fetched PDF/CSV/report actually reaches the user on iPadOS (share sheet, document picker) before building around an assumption. The "an export is a target, not a URL" seam exists to absorb whatever the answer is; the probe decides which target kind the iOS client answers.
4. **Touch polish.** Safe-area insets, on-screen-keyboard avoidance on forms, hit-target review. The SPA's narrow-viewport handling (drawer sidebar) already covers layout.

## Explicitly out of scope

- **Local books on the iPad.** CSV import workflows, snapshots and backups inside an app sandbox, and app-lifecycle suspension make the device a poor host for the database; the remote model avoids all of it. If this ever changes it is its own task with its own arguments.
- **App Store distribution and the paywall.** decision-3 and TASK-115 own where paid artifacts are built and sold; nothing here decides that.
- **Anything the PWA already proves.** The Safari home-screen client (web-app manifest task) validates every screen this shell would host, at zero native cost. If the PWA turns out to be enough, this task's priority should drop rather than its scope grow.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The shell builds and runs on an iPad (tauri ios) and drives every screen against a remote nigel serve through TASK-33.8's connection flow, with the credential in the iOS keychain
- [ ] #2 No local database is created on the device; local-books mode is not offered on iOS
- [ ] #3 Desktop-only shell code is cfg-gated: desktop builds are unaffected and CI keeps compiling the desktop crate exactly as before; no iOS packaging, signing or publishing enters this repository's CI (decision-3)
- [ ] #4 An export probe establishes how a PDF/CSV reaches the user on iPadOS before screens depend on it, and the export-target seam answers the kind the probe chose
- [ ] #5 Safe areas, keyboard avoidance and touch targets pass a review on device
- [ ] #6 Update test coverage
- [ ] #7 Create or update documentation, making sure to remove any out of date information
- [ ] #8 All linting checks pass
- [ ] #9 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
