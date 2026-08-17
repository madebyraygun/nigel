---
id: TASK-115
title: 'Epic: nigel.works — launch, licensing and Nigel Cloud'
status: To Do
assignee: []
created_date: '2026-08-17 15:26'
labels:
  - epic
  - product
  - architecture
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Nigel goes public: nigel.works is registered, the desktop builds go behind a paywall, and a hosted tier turns the biggest onboarding wall (R2 + Mailgun + DNS + API keys) into sign-in. The strategy this epic implements is written in `docs/product/foundation.md` — the ladder (build-it-yourself / Nigel Desktop / Nigel Cloud), the licensing model, and the promises that bound all of it: MIT forever, no feature gates, no custody of books, absent quietly.

- **The paywall is on the artifact, not the software.** MIT means anyone may build and redistribute; what is sold is the signed, notarized, auto-updating build and its update feed (the packaging machinery of tasks 33.5/33.6). Perpetual license, 12 months of updates, sold through a merchant of record. Trademark policy — not license terms — is what keeps unofficial builds from wearing the name.
- **Nigel Cloud is a third delivery mode, not a new architecture.** `delivery = "hosted" | "local" | "nigel"`: the Cloud client half is one more `AssetPublisher`/`Mailer` pair behind the existing traits (the epic 114 seams), authenticated by the license token. Nothing above the traits knows which mode is running, and the bring-your-own-cloud `hosted` mode stays first-class and free.
- **The public repository carries the client halves only.** The nigel.works service — licensing API, publish/mail endpoints, tenant management, the store — lives in a private repository that depends on this one, never the reverse (the lib+bin precedent). No price, signing key, store credential or service code is committed here; every service endpoint the client uses is overridable in config.
- **Phasing follows liability, not ambition** (the milestones): v1 hosts outbound pages and transactional mail — low custody, high convenience. v1.1 hosts document acceptance, making nigel.works a third-party witness to assent. v1.2 stores ciphertext backups the service cannot read. Multi-tenant hosted live books are a stated non-goal in the foundation document.
- **Launch is gated on the bookkeeper view.** Epic 32 (multiuser level one — shared instance, roles, audit trail) is assigned to the v1 milestone beside this epic's v1 subtasks: a consultancy's first question is whether their accountant can see the books without screen-sharing a laptop.

## Design decisions stated up front

- **License validation is offline for the build, online for the service.** A signed token in config: the updater presents it to the feed; `delivery = "nigel"` presents it to the Cloud API per call. A build never phones home to keep running; an expired key means no new updates and no Cloud, never a dead app.
- **Mail sends from a per-tenant subdomain** (`<tenant>.nigel.works`) with correct SPF/DKIM and reply-to the operator's own address — one tenant's reputation cannot poison another's, and no DNS work lands on the operator.
- **Payments stay the operator's own Stripe.** Nigel Cloud never sits in the money path; the pay button on a hosted invoice page points at the operator's Stripe link exactly as it does today.
- **Absent quietly, priced nowhere in the binary.** An unlicensed install shows no Cloud nags; `delivery = "nigel"` without a valid license is a refusal naming the missing key, exactly parallel to hosted mode's missing-keys refusal. Prices appear on the website only.

## Sequencing

The website and licensing land first (they gate any sale), then hosted email + invoices — those three are the v1 milestone, beside epic 32. Hosted documents (v1.1) follows epic 109's verbs; encrypted backup + sync (v1.2) is independent of both and lands last.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every subtask of this epic is Done
- [ ] #2 docs/product/foundation.md is the single source for positioning and monetization principles, and the website, licensing docs and Cloud docs all trace back to it without contradicting it
- [ ] #3 A build from this public repository remains fully functional with no feature gates, no nags and no compiled-in prices, keys or unoverridable endpoints
- [ ] #4 delivery = hosted and delivery = local behave exactly as before this epic; the nigel mode is additive behind the existing traits
- [ ] #5 No service code, signing key, store credential or price is committed to this repository, and ./scripts/check-no-real-data.sh passes on every commit in the push
- [ ] #6 IMPORTANT: Any PRs created from this epic must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
