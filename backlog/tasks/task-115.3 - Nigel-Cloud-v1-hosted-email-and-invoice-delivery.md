---
id: TASK-115.3
title: 'Nigel Cloud v1: hosted email and invoice delivery'
status: To Do
assignee: []
created_date: '2026-08-17 15:27'
labels:
  - product
  - invoicing
milestone: m-0
dependencies: []
parent_task_id: TASK-115
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The third delivery mode, invoicing first: `delivery = "nigel"` replaces the R2 + Mailgun + DNS onboarding wall with the license key.

**Client half (this repository):**
- A `NigelCloudPublisher` and `NigelCloudMailer` implementing the existing `AssetPublisher`/`Mailer` traits, calling the Cloud API with the license token — the epic 114 rule holds: nothing above the traits knows the mode. The API base URL is overridable in config (no unoverridable endpoint compiled in).
- Invoice pages publish to the operator's tenant space and the printed URL is the nigel.works address; mail sends from `<tenant>.nigel.works` with reply-to the operator's configured address. The send trace, confirmation, and failure semantics are exactly hosted mode's; a missing or invalid license is a refusal naming the key, parallel to the missing-keys refusal.
- Payments remain the operator's own Stripe: the pay button and `invoice sync` work in nigel mode exactly as in hosted mode.

**Service half (private repository — specified here, built there):**
- Tenant provisioning keyed by license token; per-tenant subdomain with SPF/DKIM; publish and send endpoints mirroring what the traits need; rate limits and outbound-mail abuse controls; deletion of a tenant's published objects on subscription end, with an export window.
- The service holds published pages and sent mail — outbound artifacts the operator already chose to make public or send — and never books, keys or Stripe credentials.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 With delivery = nigel and a valid license, invoice send publishes to the tenant space and mails from the tenant subdomain through the existing traced send, fake-tested with no real network in this repository's tests
- [ ] #2 A missing or invalid license refuses send with a sentence naming the key; hosted and local modes are pinned unchanged
- [ ] #3 The pay button and invoice sync behave in nigel mode exactly as in hosted mode — the money path stays the operator's Stripe
- [ ] #4 The service contract (endpoints, auth, tenancy, abuse limits, end-of-subscription export) is specified in the docs even though the implementation lives in the private repository
<!-- AC:END -->
