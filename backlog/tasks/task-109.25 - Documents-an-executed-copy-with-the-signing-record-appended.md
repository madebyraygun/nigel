---
id: TASK-109.25
title: 'Documents: an executed copy with the signing record appended'
status: To Do
assignee: []
created_date: '2026-11-17 21:22'
labels:
  - documents
  - signing
dependencies: []
parent_task_id: TASK-109
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When a document is executed, also produce an executed copy: the signed PDF with the signing record appended as its last pages, in one file, as e-signature services hand back.

The signed PDF stays exactly as sent. Every signature is bound to its SHA-256 checksum, and appending pages changes the bytes, so the executed copy is a new file with its own checksum, never a replacement. The appended record names the signed file's checksum, which is what a reader verifies against; the copy cannot name its own checksum, so Nigel stores it beside the version and shows it in the web UI and 'document show'.

The executed copy becomes the default download and the confirmation email's attachment, replacing the two separate PDFs; the separate signing record stays available. The signer and the operator get the copy with the full record; collaborators and the public link get the copy with the redacted record.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Countersigning produces an executed copy: the signed PDF's pages followed by the signing record's pages, stored beside the version with its own checksum
- [ ] #2 The signed PDF's bytes and checksum are unchanged, and the appended record names the signed PDF's checksum
- [ ] #3 There are two copies, matching the two record variants: the full one for the signer and operator, the redacted one for collaborators and the public page
- [ ] #4 The recipient page, the web UI, the API and the confirmation email offer the executed copy; the separate signed PDF and signing record stay downloadable
- [ ] #5 document signing-record regenerates and republishes the executed copy too; documents executed before this change get one the next time it runs
- [ ] #6 Tests cover the merge (page count, the original pages unchanged, the checksum named) and the publish; docs/invoicing.md and docs/api.md describe it
<!-- AC:END -->
