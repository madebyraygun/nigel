---
id: TASK-115.4
title: 'Nigel Cloud v1.1: hosted documents and acceptance'
status: To Do
assignee: []
created_date: '2026-08-17 15:27'
labels:
  - product
  - documents
milestone: m-1
dependencies: []
parent_task_id: TASK-115
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The documents half of hosted delivery, behind epic 109's verbs: document pages, the accept endpoint and the acceptance record served by nigel.works.

- Document publish and withdraw route through the same `NigelCloudPublisher`; the accept form posts to the Cloud's accept endpoint instead of an operator-deployed Worker — the operator deploys nothing.
- `nigel document sync` pulls acceptance records from the Cloud API with the same idempotent, per-document-failures-as-data shape as the Worker path.
- **Acceptance witness**: the Cloud stores the acceptance record (name, timestamp, version checksum) independently of the operator, and the stamped page says the record is held by nigel.works — recorded assent witnessed by a third party, with the same no-legal-claim scope statement as everywhere else.
- The operator-deployed Worker path from task 109.4 remains supported and documented for the bring-your-own-cloud rung.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 In nigel mode a document can be sent, accepted online and carried to executed with no operator-deployed Worker, through the existing lifecycle guards
- [ ] #2 document sync against the Cloud API is idempotent and reports per-document failures as data, sharing its shape with the Worker path
- [ ] #3 The acceptance record names its witness on the stamped page, and the signing-scope statement (recorded assent, no legal claim) appears wherever the record is shown
- [ ] #4 The operator-deployed Worker path keeps working and stays documented for the hosted (bring-your-own-cloud) mode
<!-- AC:END -->
