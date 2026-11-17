---
id: TASK-109.23
title: 'Documents: an optional title on the signing form'
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
The online accept form asks only for a typed full name. Add an optional Title field (e.g. Director of Operations) so a signer can state the role they sign in.

The title travels with the acceptance everywhere the typed name does: the response the page posts, the Worker's validation and the acceptance object it writes to R2, `document sync`, the stored response, `document accept --title` for a manual acceptance, the API and the web UI's Record acceptance dialog, the stamped recipient page, the signing record (full and redacted) and the confirmation email. The countersign gets the same optional field for the operator.

The Worker in workers/document-response refuses unknown fields today, so this needs a Worker change and a redeploy; an older Worker must keep accepting a response without a title, and Nigel must read old acceptance objects that have none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The accept form has an optional Title field, validated like the typed name (plain text, a length cap, control characters refused) by both the Worker and Nigel
- [ ] #2 An acceptance with a title records it, and one without records none; responses and R2 objects written before the change still sync
- [ ] #3 document accept and document countersign take --title; the API and the web UI's acceptance and countersign dialogs take the same field
- [ ] #4 The stamped page, both signing record variants and the confirmation email show the title beside the name when there is one
- [ ] #5 docs/invoicing.md and docs/api.md describe the field; the Worker's deploy notes say a redeploy is needed
<!-- AC:END -->
