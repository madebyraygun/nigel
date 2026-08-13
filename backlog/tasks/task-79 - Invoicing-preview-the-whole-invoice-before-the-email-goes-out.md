---
id: TASK-79
title: 'Invoicing: preview the whole invoice before the email goes out'
status: In Progress
assignee: []
created_date: '2026-08-10 21:48'
updated_date: '2026-08-12 01:00'
labels:
  - enhancement
  - invoicing
  - cli
  - web
dependencies: []
documentation:
  - docs/superpowers/specs/2026-08-11-task-79-send-preview-design.md
  - docs/superpowers/plans/2026-08-11-task-79-send-preview.md
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Sending is the one irreversible step in the system — once Mailgun accepts the message the client has the invoice, and nothing retries or recalls it — and neither surface shows the operator what the client is about to receive.

nigel invoice send <NUMBER> takes no flags, asks nothing, and prints "Sent invoice #N: <url>" after the fact. There is no confirmation at all, which is out of step with the rest of the CLI: void confirms and takes --yes, and recategorize by filter refuses without --yes.

On the web, wc-send-dialog explains what will happen — create the payment link, publish to the host, email the recipient — and shows the subject line, but never renders the document itself.

The capability already exists and is simply not on the path: nigel invoice preview renders the HTML and PDF locally through the same render_invoice seam send publishes through, makes no network call and needs no invoicing config. It is a separate command a person has to remember, and the web has wc-invoice-preview collapsed on the detail view, which is a different screen from the one that sends.

What is wanted is the real document in the send flow, not a summary of it: the same bytes that would be published and attached, so a wrong figure, a missing address or a broken custom template is caught before the client sees it rather than after.

Worth deciding: whether the CLI shows a rendered text form, opens the HTML, or simply requires an explicit confirmation naming the recipient and total; and whether the web preview shows the HTML page, the PDF attachment, or both, given they can differ.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The web send flow shows the rendered invoice before the send is confirmed
- [ ] #2 nigel invoice send requires an explicit confirmation, with --yes to skip it, matching void
- [ ] #3 The preview comes from the same render_invoice seam send uses, so the two cannot drift
- [ ] #4 Previewing makes no network call and creates no Stripe link
- [ ] #5 The recipient address and the total are stated where the operator confirms
- [ ] #6 A build without the pdf feature previews the HTML and says why there is no PDF, rather than blocking
- [ ] #7 A broken custom template is caught at preview, before any gateway is called
<!-- AC:END -->
