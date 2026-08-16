---
id: TASK-109.3
title: 'Publish, send and withdraw a document'
status: To Do
assignee: []
created_date: '2026-08-16 04:22'
labels:
  - documents
dependencies: []
parent_task_id: TASK-109
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The invoicing publish machinery, generalized to a second domain:

- Objects at `d/{token}/index.html` + `d/{token}/document.pdf`, named by pure functions beside `r2.rs`'s existing `object_key` trio. The recorded and printed URL names the `index.html` object, never the directory — the invoicing rule, for the same reason.
- The page is Nigel-generated: a viewer wrapper carrying the letterhead (`Branding`), client name, title, kind, dates, the PDF (linked and/or embedded), and the accept control (live or absent — the approval task decides which). Rendered through one seam shared by preview and send (`render_invoice` precedent) so the two cannot drift.
- `nigel document preview <id>` writes the page + PDF locally with no network call and no configuration, and joins `main.rs`'s launch-sync skip list.
- `nigel document send <id>`: confirmation on every surface (`confirm_or_refuse`), renders first, then publishes to R2 and emails the client's billing contact (To, cc the rest) with the PDF attached and the link in the body — traced step by step (`send_invoice_traced` shape: config → load/guards → render → publish → email → mark sent), any failure leaving the document a draft and the trace saying where it stopped. Requires the same Mailgun/R2/`public_base_url` configuration invoicing requires, missing keys named.
- `nigel document withdraw <id>`: terminal. Commits first, then best-effort teardown replaces the published page with a withdrawn notice, the PDF left in place so the token URL keeps resolving to something honest — `void_invoice_with_teardown`'s shape, warnings as data.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Preview writes the page and PDF locally with no network call and no configuration
- [ ] #2 Send is confirmed on every surface, publishes both objects, emails with the PDF attached, marks sent, and reports a step trace; a failure at any step leaves the document a draft and names the step
- [ ] #3 Withdraw commits first, replaces the published page with a withdrawn notice best-effort, and reports teardown warnings as data
- [ ] #4 All outbound traffic goes through the AssetPublisher/Mailer traits and is fake-tested; no test in the module can reach the network
- [ ] #5 Every printed or returned URL names the index.html object, never the directory
<!-- AC:END -->
