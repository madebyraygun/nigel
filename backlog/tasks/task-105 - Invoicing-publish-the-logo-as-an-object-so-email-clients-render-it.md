---
id: TASK-105
title: 'Invoicing: publish the logo as an object so email clients render it'
status: Done
assignee: []
created_date: '2026-08-13 23:51'
updated_date: '2026-08-14 19:35'
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
- [x] #1 A sent invoice's logo renders in a mail client that displays remote images
- [x] #2 A failed logo upload never fails the send
- [x] #3 The preview path still needs no network and no invoicing configuration
- [x] #4 Whatever difference remains between a previewed and a published page is deliberate and documented
- [x] #5 A void's republished page is settled either way and tested
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decisions resolved, as ruled:

- **One object at a stable key.** `i/logo.png` / `i/logo.jpg` under `public_base_url`, beside the token directories rather than inside one — the operator's own mark, carrying no client data. A per-token copy would rewrite the same bytes for every invoice ever sent.
- **Uploaded once per content change.** `invoicing::logo::publish_letterhead_logo` records `"<sha256> <url>"` in the `published_logo` metadata key and uploads only when that pair differs from what this send would publish. The URL is half the identity on purpose: `public_base_url` repointed at another bucket leaves a stale address, which is as wrong as a stale image.
- **The stored value stays the data: URI.** `company_logo` is the one source of truth and `parse_logo` the one validation path; the publisher derives bytes from it. Only a logo `render::usable_logo` passes is uploaded, so nothing reaches the bucket that either document would refuse to draw.
- **AssetPublisher grew two methods.** `publish_logo` (the upload) and `logo_url` (pure — where the object is addressed). The second is on the trait rather than derived by callers because only the publisher knows its base, and the once-per-content check has to compare the address as well as the bytes.
- **A failed upload never fails the send.** `HostedLogo.url` stays `None`, that render falls back to the inline data: URI (page still self-contained), and the sentence travels as data: `SendOutcome.warnings`, `RepublishOutcome.logo`, `warnings` on the send response, `notice:` on CLI stderr, the TUI's result screen.
- **Preview keeps the data: URI.** `Branding.logo_url` is `None` from `company_profile(...).branding(...)`; only `send` and a republish set it via `with_logo_url`. Preview's no-network / no-config invariants hold untouched.
- **A void's page keeps the logo.** `voided_page_html(number, logo_url)` reads `logo::published_logo_url(conn)` — an object a send actually published, never one derived from configuration — so the notice needs no settings and can never carry a broken link.

## Design correction: content-addressed, immutable logo objects

Review found four correctness defects that all traced to one mutable object at a stable key (`i/logo.png`). The key is now the content — `i/logo-<sha256[..8]>.<ext>` — and a published object is **never overwritten and never deleted**.

- **A delivered document cannot change after delivery.** A rebrand writes a different object; the invoice a client is looking at keeps the mark it was sent with. That is correct, not a stale cache. The MIME-switch asymmetry (a PNG replaced by a JPEG leaving a stale `logo.png`) disappears because nothing is ever overwritten.
- **Clearing `company_logo` clears the `published_logo` record.** `logo::set_company_logo` is the one writer both settings screens go through. The objects stay where delivered pages point; what must not survive is the claim that this installation currently publishes a mark.
- **The recorded URL is read only under a matching base.** `published_logo_url(conn, public_base)` takes the base as a required argument, so no caller can take it unguarded — after a bucket move the void notice omits the logo rather than naming a decommissioned domain. `AssetPublisher::public_base` is how void asks without reading settings.
- **The upload runs after the render succeeds**, in both send and republish, so a failed render writes nothing at all. The address is pure, which is what makes that ordering possible: the page is rendered against a URL before the object exists. A failed upload re-renders with the bytes inline, because a published page may only point at an object that is there.

Old objects accumulate — one per rebrand, ≤128 KiB each. Accepted and documented; they are load-bearing for delivered pages, so there is deliberately no cleanup.

Cleanups: `SendResult.warnings` always serializes (matching `configWarnings`), with the TS type non-optional — which immediately caught the SPA mock; the logo verdict is computed once per document and threaded through the new `render::render_with_logo` rather than decoded three times; the hosted-logo prologue lives in `invoicing::logo` as `pending`/`publish`; a `fake_logo_publishing!` macro replaces the copy-pasted fake bodies; `HostedLogo.uploaded` is gone (tests read the fakes).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
**Design correction (second round).** The published logo object is content-addressed (`i/logo-<sha256[..8]>.<ext>`) and immutable — never overwritten, never deleted. A rebrand writes a new object, so an already-delivered invoice keeps the mark it was sent with; clearing the stored logo clears the `published_logo` record so later documents carry none; the recorded URL is only ever read under a matching `public_base_url`; and the upload waits until the render has succeeded, so a failed render writes nothing. Old objects accumulate by design and are documented as load-bearing.
<!-- SECTION:FINAL_SUMMARY:END -->
