---
id: TASK-109.24
title: 'Documents: show that the signer signs on behalf of the client'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-17 21:22'
updated_date: '2026-11-18 00:17'
labels:
  - documents
  - signing
dependencies: []
parent_task_id: TASK-109
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Below the signature fields on the accept form, show the client's name, so the page reads as the signer accepting on behalf of that organization: 'Signing on behalf of Globex'. The same line appears under the signer's name wherever the signature is shown afterwards: the stamped recipient page and the signing record.

The client name is the one on the document at send time, frozen into the version the way the rest of the page is, so renaming a client later does not change what a signer saw. A collaborator's page carries no form and needs no line.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The signer's accept form shows 'Signing on behalf of <client>' below the signature fields
- [x] #2 The stamped page and both signing record variants show the client under the signer's name
- [x] #3 The client name is taken from the sent version, not read live, and a later client rename does not change a published page or record
- [x] #4 Tests cover the form, the stamped page and the record; docs/invoicing.md describes it
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in PR #89.
<!-- SECTION:FINAL_SUMMARY:END -->
