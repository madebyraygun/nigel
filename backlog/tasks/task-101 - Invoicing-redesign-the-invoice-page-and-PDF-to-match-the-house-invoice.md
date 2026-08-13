---
id: TASK-101
title: 'Invoicing: redesign the invoice page and PDF to match the house invoice'
status: In Progress
assignee:
  - '@task-101'
created_date: '2026-08-12 23:53'
updated_date: '2026-08-13 20:02'
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
- [x] #1 Both documents render the house layout: logo or wordmark, From block, invoice metadata, Invoice For block, ruled item table, Amount Due, Notes
- [x] #2 The company address and phone are configurable, resolved from one place, and rendered on both documents
- [x] #3 A logo can be configured and appears on the page; the PDF's treatment is decided explicitly and documented
- [x] #4 Due date renders with its terms when terms are set, and without them when they are not
- [x] #5 A missing value omits its block rather than printing an empty label, as the current renderers already do
- [x] #6 The page and the PDF agree on every figure and every block they both carry
- [x] #7 A custom template exported before this change still loads, and REQUIRED does not grow
- [x] #8 The pay-link-in-PDF rule is either upheld or reversed on the record, with the reasoning written down
- [ ] #9 A rendered example of both documents is reviewed side by side before this is called done
- [x] #10 Payment instructions are configurable text, not a sentence hardcoded in the stock template
- [x] #11 Payment instructions render on both the page and the PDF, or on neither
- [x] #12 An installation that takes no bank transfers can omit the block entirely
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Payment-instructions gap found while reviewing #204, folded into this task rather than filed separately since it lives in the same blocks this redesign rewrites.

## What is done

The document layer, both renderers, the seam, the storage and editing surfaces, the pre-#204 compatibility regression, and the documentation. Plan tasks 1-9 and 11 are complete; task 10 is the halt.

- **document.rs** is the single place every shared decision lives: `CompanyBlock`/`company_block`, `MetaRow`/`meta_rows`, `due_value`/`terms_block_text`, `payment_lines`, `parse_logo`/`Logo`/`MAX_LOGO_BYTES`, beside the `MoneySummary` and `address_lines` that were already there. Both renderers consume them, so the page and the PDF agree by construction.
- **The page** gained `LOGO`, `META_ROWS`, `TERMS_BLOCK`, `PAYMENT_BLOCK`, `COMPANY_ADDRESS`, `COMPANY_PHONE` and `PAYMENT_INSTRUCTIONS`. `REQUIRED` is unchanged and no shipped key changed meaning.
- **The PDF** draws the same blocks from the same functions, embeds the real logo, and prints no URL of any kind.
- **The letterhead** is five metadata keys resolved once by `cli::invoice::company_profile`, edited from the TUI settings screen and from `GET`/`PUT /api/settings/company` (replacing `PUT /api/settings/company-name`).
- **Docs**: docs/invoicing.md (placeholder table, a Letterhead section with the Gmail caveat, a rewritten PDF section), docs/api.md, CLAUDE.md, README.md.

## Measured, not estimated

`printpdf`'s `embedded_images` adds nine crates (240 -> 249 in `cargo tree --no-default-features --features pdf`). Release binary: 25,317,000 bytes on main with the feature off, 25,401,352 with it on and nothing using it, 26,332,728 on this branch with it on and used. The flag alone costs 84,352 bytes; the whole change costs 1,015,728 bytes, about 992 KiB on a 24 MiB binary, against a ~984 KiB estimate. `base64` named directly added no crate: 0.22.1 was already in the graph via hyper-util and reqwest.

## What the reviewer has to look at

AC #9 is not something an implementer can sign. Four rendered pairs, plus a fifth from a data directory with no letterhead at all, are what the PR body's reproduction steps produce. Three things to look at first:

1. Line-item figures read differently on the two documents: the page prints `150.00`, the PDF prints `$150.00`. This predates this branch (`{{ROWS}}` has always been raw decimals with the currency named once in the total row) and belongs to TASK-87, but it is the first difference the eye lands on.
2. The foot rule prints on a sparse invoice with nothing under it, identically on both documents. It is a literal element of the stock template rather than a renderer decision.
3. The TUI's `\n` escape: a two-line address is typed as `Line one\nLine two` in a single-line field. The web form uses a textarea and needs no escape.
<!-- SECTION:NOTES:END -->
