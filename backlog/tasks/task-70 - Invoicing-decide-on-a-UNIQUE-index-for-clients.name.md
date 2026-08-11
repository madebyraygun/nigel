---
id: TASK-70
title: 'Invoicing: decide on a UNIQUE index for clients.name'
status: In Progress
assignee:
  - '@stream-1'
created_date: '2026-08-08 08:21'
updated_date: '2026-08-11 20:58'
labels:
  - invoicing
dependencies: []
documentation:
  - >-
    docs/superpowers/specs/2026-08-11-task-69-71-63-70-invoice-correctness-design.md
  - docs/superpowers/plans/2026-08-11-task-69-71-63-70-invoice-correctness.md
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
add_client/update_client refuse duplicate names in the data layer (advisory — two racing web clients can still both insert, since clients.name carries no UNIQUE constraint). Decide whether to add the index by migration: existing databases (and InvoiceShelf imports) may already hold duplicates, so the migration needs a dedup or rename strategy before the constraint can land. Surfaced during TASK-68.6 stage 3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Either a UNIQUE index exists with a migration that handles pre-existing duplicates, or the advisory-only behavior is documented as deliberate
<!-- AC:END -->
