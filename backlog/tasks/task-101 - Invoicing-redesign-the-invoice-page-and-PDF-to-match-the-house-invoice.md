---
id: TASK-101
title: 'Invoicing: redesign the invoice page and PDF to match the house invoice'
status: To Do
assignee: []
created_date: '2026-08-12 23:53'
updated_date: '2026-08-13 00:10'
labels:
  - invoicing
  - pdf
  - ui
  - enhancement
dependencies: []
documentation:
  - docs/superpowers/specs/2026-08-12-task-101-house-invoice-layout-design.md
  - docs/superpowers/plans/2026-08-12-task-101-house-invoice-layout.md
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The stock invoice page and PDF do not look like the invoices this business already sends. The house layout, taken from a real Bluepeak invoice, is:

- A logo at the top left.
- A From block at the top right, set off by a vertical rule: company name in bold, then street address lines, then a phone line, then optional extra payment-routing lines.
- A left metadata column of label/value pairs — Invoice ID (bold value), Issue Date, Due Date with its terms in parentheses ('09/05/2026 (Net 30)'), and a Subject line describing the billing period.
- An Invoice For block to its right, same vertical rule: client name in bold, then the client's address lines.
- The line-item table with a ruled header, column dividers, and right-aligned figures.
- An Amount Due block right-aligned below the table, with a pay link beside the figure.
- A Notes section under a full-width rule at the foot.

It does not need to be an exact copy — the shape, the blocks and the hierarchy are what matter, in both documents.

What the data layer already has: number, issue date, due date, terms, line items with quantity and unit amount, notes, client name and billing address, company name. What it does not have: a logo, a company address or phone, and a per-invoice subject.

Three obstacles the design has to answer rather than assume:

1. A logo in the PDF runs into a recorded decision against it — printpdf's embedded_images feature pulls nine crates, and its soft-mask path sizes a transparent image's mask from the image's width, so a wide wordmark embeds wrong. The HTML page has no such problem. Options include embedding on the page only and drawing the company name as a wordmark in the PDF, restricting PDF logos to opaque images, or revisiting the feature decision with the binary-size cost measured.
2. A company address and phone are new configuration. They belong wherever company_name now lives rather than in a tenth invoicing key, and every surface that renders a company block has to read them from one place.
3. The reference prints a pay link inside the PDF. The current rule is that the PDF carries no live payment link, because an emailed attachment cannot be recalled or republished and a live charge link would outlive settlement. Either that rule holds and the PDF points at the invoice page instead, or it is deliberately reversed here.

Note the email body is the invoice page itself — mailgun is handed render_invoice's HTML — so there is no third template. Changing the page changes what a client receives in their inbox.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Both documents render the house layout: logo or wordmark, From block, invoice metadata, Invoice For block, ruled item table, Amount Due, Notes
- [ ] #2 The company address and phone are configurable, resolved from one place, and rendered on both documents
- [ ] #3 A logo can be configured and appears on the page; the PDF's treatment is decided explicitly and documented
- [ ] #4 Due date renders with its terms when terms are set, and without them when they are not
- [ ] #5 A missing value omits its block rather than printing an empty label, as the current renderers already do
- [ ] #6 The page and the PDF agree on every figure and every block they both carry
- [ ] #7 A custom template exported before this change still loads, and REQUIRED does not grow
- [ ] #8 The pay-link-in-PDF rule is either upheld or reversed on the record, with the reasoning written down
- [ ] #9 A rendered example of both documents is reviewed side by side before this is called done
<!-- AC:END -->
