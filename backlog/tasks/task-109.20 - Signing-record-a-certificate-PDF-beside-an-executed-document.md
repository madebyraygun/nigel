---
id: TASK-109.20
title: 'Signing record: a certificate PDF beside an executed document'
status: To Do
assignee: []
created_date: '2026-10-19 18:28'
updated_date: '2026-10-19 19:16'
labels:
  - documents
  - pdf
  - web
milestone: m-3
dependencies:
  - TASK-109.4
parent_task_id: TASK-109
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When a document is executed, Nigel generates a one-page signing record PDF and publishes it beside the signed file. The signed PDF itself never changes: every signature is bound to its SHA-256 checksum, and the record names that checksum so anyone can verify the file they hold.

The record carries the document title, kind, client, version and checksum, then each signature in order: the signer's name and typed name, method (online or manual), time, and for an online acceptance the IP address and browser; then the countersignature with its name, method and time. Change requests on earlier versions are out of scope; the record covers the executed version.

Published as d/{token}/v{n}/signing-record.pdf in the public bucket, named from the document title for downloads (e.g. Website-rebuild-v3-signing-record.pdf). The executed recipient page gets a second button, Download the signing record, beside Download the PDF. The web UI's document detail offers the same download, and GET /api/documents/{id}/signing-record.pdf serves it. TASK-109.19 attaches it alongside the executed document.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Countersigning a document generates the signing record PDF and publishes it beside the version's PDF; the signed PDF's bytes and checksum are unchanged
- [ ] #2 The record shows title, kind, client, version and the signed file's checksum, then each signature: name, typed name, method, time, and IP address and browser for an online acceptance
- [ ] #3 The executed recipient page offers Download the signing record beside Download the PDF
- [ ] #4 The web UI's document detail and GET /api/documents/{id}/signing-record.pdf serve the same file, documented in docs/api.md
- [ ] #5 A failed record upload never undoes the countersignature: the operator gets a warning and a way to republish it
- [ ] #6 Recipient-supplied text (typed name, user agent) is printed as plain text, with control characters removed
- [ ] #7 docs/invoicing.md describes the signing record; tests cover generation, publish and the page link
- [ ] #8 When a document is executed, every recipient of the executed version gets a confirmation email ('Signed: <title> for <company>') with the signed PDF and the signing record attached; it is sent after the countersignature commits, best-effort, and a failure or missing mail configuration is reported to the operator without undoing anything
- [ ] #9 Mail carries more than one attachment: OutgoingMail holds a list, every existing email is unchanged, and a message whose attachments exceed the provider's size limit is refused with a clear sentence before sending
<!-- AC:END -->
