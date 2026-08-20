---
id: TASK-78
title: >-
  Invoicing: the stock invoice page and the PDF each omit fields they already
  hold
status: Done
assignee:
  - '@stream-2'
created_date: '2026-08-10 21:48'
updated_date: '2026-08-12 00:54'
labels:
  - enhancement
  - invoicing
  - pdf
dependencies: []
documentation:
  - docs/superpowers/specs/2026-08-11-task-78-document-parity-design.md
  - docs/superpowers/plans/2026-08-11-task-78-document-parity.md
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The client-facing documents leave out information Nigel already has, and the two do not even leave out the same things.

The HTML page uses 11 of the 18 placeholders render_html.rs declares. Unused: CLIENT_EMAIL, CLIENT_ADDRESS, COMPANY, DUE_DATE, SUBTOTAL, TAX and PAY_URL. So a client billing address entered against a client renders nowhere — which is how this surfaced, after backfilling addresses onto 20 clients and finding the invoice page never shows them.

The invoice PDF (pdf::render_invoice_pdf) draws client, number, issue date, due date, subtotal, tax, total, notes and terms. It omits the client billing address, the client email and the company name entirely.

The two therefore disagree about the same invoice: the page prints a total with no subtotal or tax line, while the PDF prints all three. A client comparing the emailed attachment against the page sees two different documents.

An invoice with no company block and no client address is unusual for a real business document, and today every user gets that by default rather than by choosing it.

Notes for implementation: render_invoice_pdf takes invoice, client and items but no company, so the metadata company_name has to be plumbed in the way the report renderers already take a company argument. The template keeps its single-pass {{KEY}} expansion and its validation — load_template validates against PLACEHOLDERS and requires NUMBER, CLIENT, ROWS and TOTAL — so newly-used placeholders must stay optional or an exported custom template breaks on upgrade. Missing values need a defined rendering (an omitted block rather than an empty label), since billing address is null for 3 of 23 clients here.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The stock invoice page renders the company block, the client address and the client email when present
- [x] #2 The page and the PDF agree on the money breakdown — subtotal, tax and total are consistent between them
- [x] #3 The PDF renders the company name, the client address and the client email
- [x] #4 A null or empty value omits its block rather than printing an empty label
- [x] #5 A custom template exported before this change still loads and renders — newly-used placeholders remain optional
- [x] #6 The pay link placement on the page is settled deliberately, not left unused
- [ ] #7 A rendered example of both documents is reviewed side by side before this is called done
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- New `src/invoicing/document.rs`: `MoneySummary`/`MoneyLine` decide which money lines a document prints — Subtotal and Tax only when tax is non-zero (the rule the PDF already applied), Total always, Paid and Balance due once anything is paid. A balance inside half a cent of zero is clamped so nothing prints `-0.00`. `address_lines` splits a billing address into the lines it was typed as, dropping blanks. Both renderers consume it, so the two documents agree by construction.
- `render_html.rs`: four new fragment placeholders — `COMPANY_BLOCK`, `CLIENT_ADDRESS_BLOCK`, `CLIENT_EMAIL_BLOCK`, `TOTALS`. Fragments rather than template syntax, because `expand` is single-pass with no conditionals and that property is what keeps a client named `Acme {{ROWS}} Co` literal. The seven previously-unused text keys keep their exact meanings, pinned by `the_bare_text_keys_did_not_change_meaning`.
- Stock template rewritten: company block above the heading, address and email inside the "Billed to" paragraph, `{{TOTALS}}` in a `<tfoot>` of the line-item table (which is what lines the amounts up under the Amount column), `{{PAY}}` unmoved.
- `render_invoice` loads `paid_amount` beside the line items it already loads, so preview, send, republish and the two API preview routes pick the figures up with no signature change anywhere above the seam — `git diff --stat` shows no change to `src/cli/invoice.rs`, `src/invoicing/send.rs` or `src/server/routes/invoices.rs`.
- `render_invoice_pdf` gains the client's address (one row per typed line) and email under `Billed to:`, and draws the shared money rows in place of its own inline `tax != 0.0` block.
- **Spec discrepancy, resolved.** The 78 spec's Decision 4 both drops `{{TOTAL}}` from the stock page and says it "stays required" — the stock page then fails its own validator. Resolved with `REQUIRED_ALTERNATIVES`: `{{TOTALS}}` stands in for `{{TOTAL}}`, since the block *is* the total plus whatever else is true. `REQUIRED` still lists the same four keys, an old template carrying `{{TOTAL}}` still validates, and a template carrying neither is still refused naming `{{TOTAL}}`.
- **Orchestrator override applied.** No pay URL in the PDF at all, rather than the spec's `Pay online: <url>` line. `render_invoice_pdf` therefore took no `pay` parameter. The "print the public page URL instead" option was not taken: the page URL is not at the seam (it needs `public_base_url`), so it would mean a new `Branding` field — and Stream 3 is concurrently editing `Branding{contact_email}`, so growing that struct here buys a conflict for a line the page already carries. `no_live_payment_link_reaches_the_pdf` pins the absence.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The page and the PDF now carry the same facts about the same invoice, and neither prints a label with nothing after it.

The decision that makes them agree is `src/invoicing/document.rs`: `MoneySummary::lines()` is the single answer to "which money lines does this invoice have", and both renderers ask it. The page gained the company block, the client's address and email, and that money block as `<tfoot>` rows under the Amount column; the PDF gained the address and the email under `Billed to:` and swapped its own tax rule for the shared one. `render_invoice` loads `paid_amount` beside the line items, so Paid and Balance due reach preview, send, the API preview routes and TASK-64's republish without a single signature changing above the seam.

Omission is the renderer's job, through four new *fragment* placeholders. `REQUIRED` does not grow, so a template exported from an older Nigel keeps loading and keeps rendering exactly what it did — pinned by a regression test that pastes the pre-change stock page in verbatim.

The PDF deliberately carries no payment link (orchestrator override): an emailed attachment cannot be recalled or republished, so a live charge link in one would outlive the settlement it was created for. Paying online stays the published page's job, since the page is the one artifact a republish can correct.

AC #7 (side-by-side review) is Sam's, on the PR: three rendered pairs — rich, sparse and part-paid — with the reproduction steps and the HTML-vs-PDF field lists in the PR body.
<!-- SECTION:FINAL_SUMMARY:END -->
