---
id: TASK-64
title: 'Invoicing: republish HTML and PDF when payments change an invoice'
status: In Progress
assignee:
  - '@stream-2'
created_date: '2026-08-07 23:09'
updated_date: '2026-08-11 23:35'
labels:
  - invoicing
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/pull/172'
documentation:
  - docs/superpowers/specs/2026-08-11-task-67-64-publish-pipeline-design.md
  - docs/superpowers/plans/2026-08-11-task-67-64-publish-pipeline.md
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The published R2 page and PDF are rendered once at send time and never touched again. After a payment lands (via invoice pay or Stripe sync), the public page still shows the original balance, so a client following their link sees an unpaid invoice they already settled.

When a payment is recorded against a published invoice, Nigel should re-render and re-upload i/{token}/index.html and invoice.pdf so the page reflects paid amount, balance, and status. Needs the R2 config at pay/sync time; should be best-effort like the launch sync (a failed republish must not fail the payment recording).

Found during pre-merge testing of PR #172.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Recording a manual payment against a published invoice re-renders and re-uploads the HTML and PDF
- [x] #2 Payments recorded by invoice sync trigger the same republish
- [x] #3 A failed republish leaves the payment recorded and reports a notice rather than an error
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- New `src/invoicing/republish.rs` on `void.rs`'s vocabulary: `Republished` (`NotApplicable`/`Skipped`/`Done { pdf }`/`Failed`), `RepublishOutcome::warnings()` as the one place the sentences live, and `republish_invoice` returning **no `Result`** — the payment is committed money and nothing out here may read as its failure. It dispatches on `(published_at, is_void, publisher)` before rendering, so the ordinary case (a payment on an invoice that was never published) costs nothing.
- `pay_button_for` moved from `cli::invoice` to `invoicing::render`, beside the seam, and gained the paid-in-full arm. `send.rs`'s inline two-arm match is gone, so re-sending a settled invoice now publishes a page with no Pay button — correct, and previously an accident of nobody trying it. `cli::invoice::pay_button_for` is a re-export, so the API preview routes needed no edit.
- Without the `pdf` feature republish falls back to `publish_page`: the page is corrected and the attachment the client was actually sent is left where it is, the rule void already follows. `Done { pdf: false }`, no warning.
- `SyncReport.recorded_invoices` carries the numbers a payment landed against (numbers, not ids — it crosses the wire).
- Six call sites, one line each through `cli::invoice::republish_after_payment` / `republish_all`, which resolve the branding `src/invoicing/` may not read.
- `POST /api/invoices/{n}/pay` answers `PayResult` (the detail flattened plus `republishWarnings`) and gained a `pay_with(conn, number, request, publisher)` seam mirroring `void_with`, so the whole thing is fake-tested with no network. `POST /api/invoices/sync` answers `SyncResult` — `SyncReport` flattened plus `republishWarnings`; the warnings are the route's, not the data layer's, which republishes nothing.
- **TUI departure from the plan.** A payment against a published invoice now reaches the network, so it cannot run from the key handler: `record_pay_form` records the payment inline (fast, all existing refusals unchanged), then returns `InvoiceAction::Perform` with a new `Screen::Republishing` frame, and `perform_pending_republish` does the uploads. An unpublished invoice returns `Continue` and stays the plain write it was. This is the rule `cli/invoice_manager.rs` already applies to send and void; leaving pay single-phase would freeze the terminal on the payment form.
- SPA: `voidWarnings` became the general `actionWarnings` channel (`data-void-warning` → `data-action-warning`), and a pay pushes `republishWarnings` into it. The sentences come from Rust verbatim. They survive the post-payment refetch, the way a void's do.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A payment against a published invoice now puts a corrected page and PDF back where the client is looking.

`src/invoicing/republish.rs` is `void.rs` with a different verb: the write commits first, nothing afterwards can undo it, and every way it can go wrong is a variant plus a sentence rather than an error. Nothing configured is a warning naming the invoice; an R2 refusal is a warning carrying the upstream's own words; a build without the `pdf` feature corrects the page and leaves the attachment the client was actually sent.

What makes the republished page worth publishing is TASK-78's money block, and what makes it honest is `pay_button_for` moving below the seam: void and paid-in-full both omit the button, so a settled invoice's page stops offering to charge it.

All six surfaces go through two CLI-layer helpers, because `src/invoicing/` may not read settings or load a template. `POST /api/invoices/{n}/pay` therefore reaches the network and answers `republishWarnings` beside the refreshed detail, the shape void's `teardownWarnings` established; `POST /api/invoices/sync` does the same for every invoice its run moved, which is why `SyncReport` now names them. The TUI paints a `Republishing` frame before the uploads, for the reason it paints one before a send and a void.
<!-- SECTION:FINAL_SUMMARY:END -->
