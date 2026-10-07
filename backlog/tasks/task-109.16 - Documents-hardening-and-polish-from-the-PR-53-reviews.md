---
id: TASK-109.16
title: 'Documents: hardening and polish from the PR #53 reviews'
status: To Do
assignee: []
created_date: '2026-10-05 23:22'
updated_date: '2026-10-07 18:44'
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
- [ ] #1 record_online_response runs under BEGIN IMMEDIATE and maps a unique-index violation to AlreadyRecorded
- [ ] #2 A partial unique index enforces one signer per version in the schema
- [ ] #3 The launch sync surfaces refused responses as notices
- [ ] #4 build_document_clients warns when documents_base_url does not end in /d
- [ ] #5 Pages of an earlier version read as superseded once a later version is sent
- [ ] #6 The CLI checks configuration before asking to confirm a send; the send dialog pluralizes 'recipient' correctly; invoicing.md lists the real seeded kinds
- [ ] #7 nigel document show passes recipient-supplied text through printable(), and printable() also maps carriage returns and bidi controls
<!-- AC:END -->
