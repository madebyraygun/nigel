---
id: TASK-105
title: 'Invoicing: publish the logo as an object so email clients render it'
status: To Do
assignee: []
created_date: '2026-08-13 23:51'
labels:
  - invoicing
  - email
  - enhancement
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The letterhead logo is stored and rendered as a data: URI, which makes the published page self-contained but means Gmail — which proxies and displays remote images by default — strips it, so a Gmail recipient sees the alt text where the logo should be. The PDF attachment is unaffected.

Publishing the image as its own object beside the page would fix it: the send order is Stripe link, render, R2 publish, email, so an object uploaded with the page exists before the message goes out, and an absolute URL in the img src renders in every mail client that shows remote images.

The design has one real cost to resolve, and it is the reason this is a task rather than a patch. `invoice preview` renders through the same render_invoice seam `send` publishes through, and CLAUDE.md records that the only differences between a previewed and a published invoice are the Pay placeholder on an unsent draft and the absent PDF in a no-pdf build. A published page pointing at a hosted logo while a previewed page carries the data URI adds a third difference. That precedent exists, so the difference can be taken deliberately and documented — but it must be a decision, not a drift.

Points the design has to settle:
- Where the object lives. A per-token sibling duplicates the same bytes for every invoice ever sent; a single object at a stable key under the base URL is uploaded once and cached, and carries no client data, since a logo is the operator's own brand.
- What AssetPublisher's contract becomes. It publishes exactly two objects today, and publish_page deliberately rewrites the page alone so a void leaves the PDF where it is. A third object means a new upload, a new failure mode, and a decision about whether a failed logo upload is a failed send (it should not be).
- What preview renders, and whether the page falls back to the data URI when no public_base_url is configured.
- Whether the stored value stays a data URI with the publisher deriving bytes from it, which keeps one source of truth and one validation path.
- Whether a void's republished page keeps the logo.

Surfaced while reviewing the house invoice layout.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A sent invoice's logo renders in a mail client that displays remote images
- [ ] #2 A failed logo upload never fails the send
- [ ] #3 The preview path still needs no network and no invoicing configuration
- [ ] #4 Whatever difference remains between a previewed and a published page is deliberate and documented
- [ ] #5 A void's republished page is settled either way and tested
<!-- AC:END -->
