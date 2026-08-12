---
id: TASK-78
title: >-
  Invoicing: the stock invoice page and the PDF each omit fields they already
  hold
status: In Progress
assignee: []
created_date: '2026-08-10 21:48'
updated_date: '2026-08-12 01:00'
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
- [ ] #1 The stock invoice page renders the company block, the client address and the client email when present
- [ ] #2 The page and the PDF agree on the money breakdown — subtotal, tax and total are consistent between them
- [ ] #3 The PDF renders the company name, the client address and the client email
- [ ] #4 A null or empty value omits its block rather than printing an empty label
- [ ] #5 A custom template exported before this change still loads and renders — newly-used placeholders remain optional
- [ ] #6 The pay link placement on the page is settled deliberately, not left unused
- [ ] #7 A rendered example of both documents is reviewed side by side before this is called done
<!-- AC:END -->
