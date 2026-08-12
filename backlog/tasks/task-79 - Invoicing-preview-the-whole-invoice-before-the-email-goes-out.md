---
id: TASK-79
title: 'Invoicing: preview the whole invoice before the email goes out'
status: In Progress
assignee:
  - '@stream-2'
created_date: '2026-08-10 21:48'
updated_date: '2026-08-12 00:52'
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
- [x] #1 The web send flow shows the rendered invoice before the send is confirmed
- [x] #2 nigel invoice send requires an explicit confirmation, with --yes to skip it, matching void
- [x] #3 The preview comes from the same render_invoice seam send uses, so the two cannot drift
- [x] #4 Previewing makes no network call and creates no Stripe link
- [x] #5 The recipient address and the total are stated where the operator confirms
- [x] #6 A build without the pdf feature previews the HTML and says why there is no PDF, rather than blocking
- [x] #7 A broken custom template is caught at preview, before any gateway is called
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
**The CLI.** `write_preview` is the second half of `preview()` extracted verbatim, so `preview` and the confirmation `send` shows cannot differ about paths, permissions or the `Wrote`/no-PDF wording — `tests/cli_dispatch.rs`'s seven `invoice_preview_*` tests pass untouched, which is the proof. `send` gained `--yes`, worded exactly as `void`'s, and in the interactive path renders through the seam, prints `send_summary` (client, total, currency, issue date, recipient), writes the artifacts, prints `send_consequences` (first send vs re-send, naming the publish host), and asks.
- **Order departure from the plan.** The non-TTY refusal happens *before* the summary and before `write_preview`, not inside `confirm_send` after them — the plan's own test asserts a refused send writes nothing. `refuse_unconfirmed_send` is the shared guard; `confirm_send` still calls it, so the unit under test is unchanged.
- Every pre-existing `invoice send` test in `cli_dispatch.rs` gained `--yes`: they all drive non-interactive runs and would otherwise stop at the new refusal.

**The web.** New `wc-document-frame` owns every iframe in the app and the one `PREVIEW_SANDBOX` constant, taking either `srcdoc` or `src`, with the spinner and a `--nc-document-frame-height`. `wc-invoice-preview` delegates to it and keeps its disclosure, its lazy rule, its links and its notice.
- **Test departure.** The plan wanted `wc-invoice-preview`'s six specs unchanged. Two of them read the iframe's own attributes, and after delegation the iframe is one shadow root deeper, so those two now reach through a small `innerFrame()` helper. Every assertion is the same claim about the same element; nothing else in the file moved.
- `ApiClient.invoicePreviewHtml` fetches the page as text against the address `invoicePreviewUrl` already spells (one literal, so `api-seam.test.ts` stays satisfied) and raises the usual `ApiError` so the dialog can print the server's sentence.
- `wc-send-dialog` gained the recipient-and-amount line (`wc-money` for the figure), the framed document in the confirm phase only, `previewLoading`/`previewError`, the PDF link or its absence, and `configCautions` — which is where TASK-67's `/i` warning lands on the web, per the review ruling. The dialog widens to `min(60rem, 92vw)` and the frame is bounded at 24rem (16rem below 48rem), so the document scrolls inside the frame and the dialog body scrolls once.
- `emailSubject(number, companyName)` in `invoice-data.ts` mirrors `send.rs`'s rule, sourced from `status.companyName` — deliberately *not* `appStore.companyName`, whose 'Nigel' fallback is a display default the server does not use.
- **Blocking departure.** A build without `pdf` disables the Send button with the reason in its title, the way unset keys already do, rather than opening a dialog with an inert confirm. The dialog supports and tests `blocked` beside a rendered frame; the screen just never gets there. AC #6 holds either way: the detail view's preview still renders, and the reason is stated before anything is attempted.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Neither surface can send an invoice without showing the operator what the client is about to receive.

`nigel invoice send` renders the document through the same seam the send publishes through, writes it to `invoice preview`'s own paths, states the recipient and the total, and asks — with `--yes` to skip the prompt and the files, matching `void` exactly, and a non-TTY refusal that happens before either. The render happens either way, so a broken custom template is caught before any gateway is constructed.

The web send dialog frames the rendered page above the confirm button, with the PDF offered beside it. It takes the HTML as `srcdoc` through a new `ApiClient.invoicePreviewHtml` rather than pointing an iframe at the route, because an iframe cannot report a failure: a broken template arrives as the server's sentence instead of as an error envelope drawn in a box. The iframe itself moved into a new `wc-document-frame`, so the app has one iframe implementation and one sandbox constant, and `wc-invoice-preview` delegates to it.

No new render path exists anywhere: everything shown is `render_invoice`, directly or behind the preview route, and no gateway is constructed on either path.

The web half is visual and wants Sam's eye — the preview harness carries five new dialog states (`confirm-with-preview`, `preview-loading`, `preview-failed`, `blocked-no-pdf`, `config-caution`) and three for the frame.
<!-- SECTION:FINAL_SUMMARY:END -->
