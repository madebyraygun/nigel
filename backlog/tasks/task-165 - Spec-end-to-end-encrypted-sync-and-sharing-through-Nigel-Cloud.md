---
id: TASK-165
title: 'Spec: end-to-end encrypted sync and sharing through Nigel Cloud'
status: To Do
assignee: []
created_date: '2026-10-23 19:47'
labels:
  - spec
  - cloud
  - sync
  - security
  - e2e
milestone: m-7
dependencies:
  - TASK-163
  - TASK-161
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Design the 3.0 model in which a set of books lives on its members' devices and syncs through Nigel Cloud end to end encrypted, per decision-12. Nigel Cloud never holds a key or a readable byte of the books; it holds only what the operator has chosen to publish.

The spec covers:
- Sync: each device keeps a local replica (the desktop's books package; the browser's SQLite in OPFS running nigel-core compiled to WebAssembly); edits travel as an encrypted operation log through Nigel Cloud; ordering, conflict handling and merge rules for concurrent edits by two people; compaction and snapshots; bootstrapping a new device.
- Keys and sharing: the books' data key (TASK-161's envelope) wrapped to each member's public key; invitations, roles, revocation with key rotation; recovery codes in a shared setting.
- The two classes of data: books (always end-to-end) and published items (sent or scheduled invoices, documents out for signature, and their status), which the cloud may hold because they are already addressed to a third party.
- Incoming events with no device online: Stripe webhooks and document responses update the published item (an invoice page reads Paid, reminders stop) and are sealed to the books' public key for the next device to apply.
- Scheduled delivery with no device online: devices prepare upcoming invoices a configurable window ahead (number, payment link, page, PDF) and queue delivery packages; Nigel Cloud sends on time, updates or withdraws on edit or cancel, and warns the operator before the window runs out. Overdue reminders run from the published items.
- What still needs a device (anything that writes to the books) and how it catches up.
- Threat model, what a compromised Nigel Cloud would and would not learn (metadata such as sizes and timing), and how the claim is made checkable (open protocol, published client builds, independent audit).
- Migration from 2.0's single-server model (epic 32) and how a self-hosted server can join as an always-on member.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A spec under docs/superpowers/specs/ covers every point above, with the threat model and the metadata the cloud can observe stated plainly
- [ ] #2 The spec states which automations run with no device online and which wait for a device, and how the operator is told
- [ ] #3 The spec lists the implementation tasks it implies, sized, and is reviewed and approved by the operator
<!-- AC:END -->
