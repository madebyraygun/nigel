---
id: TASK-101
title: 'Invoicing: redesign the invoice page and PDF to match the house invoice'
status: In Progress
assignee:
  - '@task-101'
created_date: '2026-08-12 23:53'
updated_date: '2026-08-14 00:21'
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

## Review round — 13 findings, all fixed

The one that mattered: deleting the stock page's hardcoded bank-transfer paragraph was a silent regression. Migration **v9** now seeds `payment_instructions` from that sentence for a database that was already invoicing (address from the same `contact_email` -> `from_email` fallback `{{CONTACT}}` used; nothing written when the key is set, when there is no address, or when the books have never invoiced), and `cli::invoice::payment_instructions_notice` puts one stderr line on `preview`/`send` when a document would go out with no way to pay on it.

PDF geometry: the phone joined the From block's lines (labelled block for a phone-only company, rule brackets everything); party lines wrap in their column; the wordmark shrinks then cuts; metadata values are cut at the party column; the divider guard compares the page the table started on. Two new `cfg(test)` seams — `drawn_text` and `drawn_lines` — make those geometry assertions rather than text assertions.

The logo verdict is reached once: `parse_logo` gained the `IEND`/`FFD9` completeness check (no decoder, holds in every build), `pdf::logo_is_embeddable` is the decode, and `render_invoice` clears `Branding.logo` on failure so the page cannot disagree with the PDF.

SPA: client-side `MAX_LOGO_BYTES` check, a load-failure state with retry, hoisted field handlers, and a new mode-independent `--nc-color-document-bg` token in place of an inline white. TUI: the `\n` escape is symmetric. `Branding` lost `Default`.

**Binary measurement corrected.** The first-round baseline embedded the placeholder SPA while the branch embedded the real one, so ~740 KiB of web assets sat inside a number meant to be about printpdf. Like for like: feature off 26,075,256; feature on, unused 26,159,752 (**+84,496**); this branch 27,064,640 (+989,384, every line of the task).

Verified after the fixes: 1343+117 / 995+117 / 1268+116 cargo, clippy and fmt clean, web 760 tests with build/lint/typecheck clean.

## Side-by-side review round — five layout changes

Plan Task 10 ran; five changes came back and all five landed on both documents, with no parity exception.

1. **Smaller logo.** Both caps are now `document::LOGO_WIDTH_FRACTION`/`LOGO_HEIGHT_FRACTION` — a fifth of each document's own measure, since the page counts in rem against its body width and the PDF in mm against its printable width. Page 14rem/4rem -> 8.8rem/2.5rem; PDF box 60x16mm -> 35.6x10mm. Aspect ratio and wordmark fallback untouched.
2. **The number is printed once.** The page's `<h1>` and the PDF's title line are gone; the metadata band carries the identifier. `{{NUMBER}}` still satisfies REQUIRED from the `<title>` element (asserted, plus `validate_template` on the stock page), and the PDF keeps `Invoice #N` as its Info title. Nine existing tests were rewritten to the new order rather than deleted.
3. **More space before the item table** on both — PDF gap 6mm -> 14mm under the band, measured by a test rather than eyeballed.
4. **One medium grey for every border** (`document::BORDER_GRAY`), replacing `#111`/`#ddd`/`#eee`. Body type stays dark. A test reads the hex back out of the static template and fails on any remaining near-black `border-*`.
5. **Row rules and zebra striping.** `document::row_is_shaded` decides which rows on both. printpdf fills rects, so no exception was needed; the fill colour is restored to black after each band because `use_text` inherits it, and the band is drawn after `ensure_space` and before the cells so striping and rules carry across a page break. Verified end to end on a 45-line invoice: 36 rows/18 bands on page 1, rows 37-45/4 bands on page 2, alternation continuing by row index rather than restarting.

Verified after the changes: 1351+117 / 999+117 / 1272+116 cargo, clippy and fmt clean, web 760 tests with build/lint/typecheck clean.

Still halted at Task 10 for re-review. AC #9 stays unchecked.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-08-13 20:13
---
Implementation complete through plan Task 9 and Task 11. PR #3 opened against main and NOT merged: https://github.com/madebyraygun/nigel/pull/3

Task 10 is the halt. AC #9 is unchecked and stays unchecked until the four rendered pairs have been looked at side by side. The PR body carries the reproduction steps and the three things to look at first.
---
<!-- COMMENTS:END -->
