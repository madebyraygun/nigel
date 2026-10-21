---
id: TASK-109.16
title: 'Documents: hardening and polish from the PR #53 reviews'
status: Done
assignee: []
created_date: '2026-10-05 23:22'
updated_date: '2026-10-21 14:03'
labels:
  - documents
milestone: m-3
dependencies: []
parent_task_id: TASK-109
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Smaller items deferred from the documents PR reviews.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 record_online_response runs under BEGIN IMMEDIATE and maps a unique-index violation to AlreadyRecorded
- [x] #2 A partial unique index enforces one signer per version in the schema
- [x] #3 The launch sync surfaces refused responses as notices
- [x] #4 build_document_clients warns when documents_base_url does not end in /d
- [x] #5 Pages of an earlier version read as superseded once a later version is sent
- [x] #6 The CLI checks configuration before asking to confirm a send; the send dialog pluralizes 'recipient' correctly; invoicing.md lists the real seeded kinds
- [x] #7 nigel document show passes recipient-supplied text through printable(), and printable() also maps carriage returns and bidi controls
- [x] #8 A deactivated document kind can be reactivated: nigel document kinds reactivate <id>, the matching API action, and a Reactivate control in the web UI; reactivating an active kind is refused like deactivating an inactive one
- [x] #9 Document email subjects lead with the ask: a signer's reads 'Please review and sign: <title> for <company>', a collaborator's 'Please review: <title> for <company>'; with no company name set, the ' for <company>' part is omitted
- [x] #10 The signer's email carries no signature sentence (the recipient page already states it), and neither email repeats the company name at the end of the body
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
#9 and #10 shipped in PR #53.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in PR #58 (migration v16: one signer per version).
<!-- SECTION:FINAL_SUMMARY:END -->
