---
id: TASK-115.2
title: 'Licensing: merchant of record, keys, updater feed and trademark policy'
status: To Do
assignee: []
created_date: '2026-08-17 15:27'
labels:
  - product
  - desktop
milestone: m-0
dependencies: []
parent_task_id: TASK-115
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
How a purchase becomes a working, updating install — the mechanics behind the foundation document's licensing section.

- **Purchase**: the merchant of record hosts checkout, handles VAT/sales tax and refunds, and issues the license key on completion. The store configuration lives in the private repository; this repository documents the flow.
- **The key is a signed token** (operator id, entitlement, updates-until date) verifiable offline against a public key shipped in the app. It lands in config next to the other keys; `nigel license show` prints its status, and every surface shows it in settings.
- **The updater feed** (task 33.5's Tauri updater) checks the token: valid-and-current serves the latest build, expired serves nothing newer than the entitlement date — the app itself never expires, never phones home to run, and says plainly in settings when updates have lapsed and what renewing gets.
- **App-store builds** are the same app with store-managed updates; the token is not required to run them (the store handled payment) but still unlocks `delivery = "nigel"`.
- **Trademark policy** committed at the repository root: unofficial builds do not use the Nigel name or icon; everything else MIT allows is fine and stated to be fine.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A completed checkout yields a key that validates offline, shows its status in nigel license show and in every surface's settings, and gates nothing but updates and Cloud
- [ ] #2 An expired key leaves the installed app fully functional; the updater refuses newer builds with a sentence naming the entitlement date
- [ ] #3 The trademark policy is committed and linked from the README, and no signing key, store credential or price is committed to this repository
- [ ] #4 The purchase-to-running-app flow is documented end to end where the operator docs live
<!-- AC:END -->
