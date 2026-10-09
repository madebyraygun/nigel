---
id: decision-9
title: 'Mac App Store first: the store is the 2.0 merchant and updater'
date: '2026-10-07 18:43'
status: accepted
---
## Context

Nigel 2.0 ships native apps with the Mac app in the Mac App Store, a server/client model,
invoices and contracts. `docs/product/foundation.md` describes paid builds sold from
nigel.works through a merchant of record, unlocked by a license key, and kept current by a
licensed update feed (TASK-115.2, Sparkle in TASK-144.14). None of that machinery exists
yet.

The App Store rules decide most of the shape. Guideline 2.4.5 requires the App Sandbox and
forbids a store app from shipping its own update mechanism or requiring a license key to
unlock it. Guideline 3.1.1 makes unlocking paid functionality inside the app an in-app
purchase. Guideline 3.1.3(b) lets a multiplatform service sold elsewhere be used in the app
when the user signs in to an account bought outside it, provided the app does not point to
that purchase. Apple is the merchant of record and the updater for a store app, so a store
build needs none of the licensing machinery to ship.

## Decision

**The Mac App Store is the 2.0 paid channel. Direct downloads and licensing follow in 2.1.**

- **Store build first, paid up front.** The Swift shell (TASK-144) ships through the Mac
  App Store at a one-time price, with no in-app purchase. Store buyers update through the
  store for the life of that App Store app; there is no "12 months of updates" on the
  store. A future paid upgrade means a new App Store app.
- **The store build is the same app with three differences:** it runs in the App Sandbox
  from the first commit, it has no update mechanism of its own (no Sparkle, no updater
  path, no update notification), and it asks for no license key. A `store` build flavor
  compiles those paths out; `/api/status` reports the distribution so the SPA hides
  update and licensing UI without detecting its host.
- **Entitlements stay minimal:** user-selected file access, app-scoped bookmarks, and
  `network.client`. No `network.server` — the server is headless `nigel serve`, and the
  Mac app is its client or runs local books. No temporary-exception entitlements.
- **Bundle ID `com.madebyraygun.nigel`.** It is permanent once the App Store Connect
  record exists. The intended store listing is "Nigel: Small business accounting,
  invoices, and contracts"; App Store names and subtitles are each limited to 30
  characters, so the split between name and subtitle (for example "Nigel" plus a subtitle
  of at most 30 characters) is chosen with the store record.
- **Nigel Cloud delivery (TASK-115.3) is in 2.0, and the store build signs in to it.**
  Sign-in is framed as signing in to an existing nigel.works account, under guideline
  3.1.3(b): the store build carries no pricing, no purchase or "subscribe" link and no
  call to action for Cloud. The subscription is sold on nigel.works only, never by in-app
  purchase.
- **Local delivery (epic 114) also ships in 2.0**, so a store user sends invoices and
  contracts with no accounts at all.
- **Direct downloads in 2.1:** signed builds for macOS, Windows and Linux from nigel.works,
  the merchant of record, license keys (TASK-115.2) and the Sparkle and Tauri updaters
  (TASK-144.14, TASK-33.5, TASK-33.6).

## Consequences

- **Risk: App Review may treat Cloud sign-in as unlocking a paid service and require
  in-app purchase.** The fallback is to compile Cloud sign-in out of the store build for
  that submission; local and hosted delivery are unaffected, and the store build keeps
  working with no accounts. In-app purchase is not the fallback.
- `docs/product/foundation.md` gains the store rung: store buyers update through the
  store, and "perpetual license with 12 months of updates" applies to direct builds.
  Cloud phases are 2.0 (delivery), 2.2 (hosted documents) and 2.3 (encrypted backup and
  sync).
- TASK-115.2 states that the store build never asks for a key and that direct downloads
  with license keys are 2.1. TASK-144.14 (Sparkle) and the update-available notification
  in TASK-144.12 are direct-build only.
- The Swift shell spec carries an App Store section, and the store work is tracked in
  "Epic: Nigel on the Mac App Store". Under decision-3, packaging, signing and upload
  happen in the private pipeline; this repository carries the sandbox-safe code, the
  build flavor and the docs.
- In the sandbox the home directory resolves inside the app container, so the shell owns
  the books location: a security-scoped bookmark to the books folder, never a fallback to
  a new database.
