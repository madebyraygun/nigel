---
id: TASK-138
title: The published invoice page has no PDF download link
status: To Do
assignee: []
created_date: '2026-09-11 16:11'
labels:
  - invoicing
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The PDF is already on R2: `R2Publisher::publish` puts `i/{token}/invoice.pdf` beside `i/{token}/index.html` on every send (`crates/nigel-core/src/invoicing/r2.rs:210`). Nothing links to it. A client who wants the PDF has to find the emailed attachment, and anyone the page was forwarded to has no way to get one at all.

The template (`crates/nigel-core/src/invoicing/templates/invoice.html`) mentions the PDF nowhere. Adding the link is most of the work; the rest is deciding what to do when the object beside the page is not the document the page shows:

- `void` republishes only the page (`invoicing/void.rs:184`), so a voided invoice keeps the PDF from before it was voided. A Download PDF link there hands out a document that does not say VOID.
- `republish` after a payment sends both artifacts only when the `pdf` feature is built in; without it, `publish_page` corrects the page and deliberately leaves the emailed PDF alone (`invoicing/republish.rs:148`). The page would then link to a PDF showing a balance that is no longer right.

So the link is conditional, not unconditional, and the condition is what this task has to settle.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The published invoice page offers a Download PDF link pointing at the invoice.pdf object beside it
- [ ] #2 A voided invoice's page does not offer a PDF that fails to say it is void — the behaviour is decided and documented either way
- [ ] #3 A page republished without the pdf feature does not link to a PDF that contradicts the balance the page states
- [ ] #4 docs/invoicing.md describes what the published page offers
- [ ] #5 Tests cover the link present, and each case where it is withheld
<!-- AC:END -->
