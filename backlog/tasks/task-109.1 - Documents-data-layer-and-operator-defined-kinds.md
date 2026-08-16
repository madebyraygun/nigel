---
id: TASK-109.1
title: Documents data layer and operator-defined kinds
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
The foundation: a `documents` table and a `document_kinds` table, in a schema migration (append to `MIGRATIONS`, bump `LATEST_VERSION`).

- `document_kinds`: name, slug, active flag — seeded with Proposal, Estimate and Agreement as editable defaults, seeded exactly once the way the chart of accounts is (`init_db_with_profile` precedent: re-running init never reseeds). No compiled-in enum anywhere; every surface reads the table.
- `documents`: client_id (FK), kind_id, title, stored file path + sha256 checksum, 16-char random token (invoices precedent), issue date, sent_at / accepted_at / declined_at / voided_at timestamps, acceptance fields (accepted_by, method), derived status.
- Status is derived, never hand-set: a `refresh_status`-shaped function computes draft/sent/accepted/declined from the timestamps and withdrawn from voided_at — the invoicing precedent, including `validate_date`-style normalization for any stored date.
- Guards live in the data layer, not the callers: accepted/declined/withdrawn are terminal (no edit, no re-send); an archived client refuses a new document (`ensure_client_active` precedent); `clients::delete_blocker` counts documents as well as invoices, so deleting a client with documents is refused with a structured `DeleteBlock`.
- Structs derive camelCase `Serialize` following the task-31.2 pattern, since the API task will put them on the wire.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The migration creates documents and document_kinds and seeds Proposal, Estimate and Agreement exactly once; re-running init or the migration never reseeds
- [ ] #2 Kinds are rows: adding, renaming and deactivating a kind are data operations exercised by tests, and no Rust enum mirrors the kind list
- [ ] #3 Status is derived by one function from the stored timestamps; no code path writes status directly, pinned by a test
- [ ] #4 Terminal-state and archived-client guards are data-layer functions raising typed NigelError variants, and client delete is blocked while documents exist, with the count in the DeleteBlock
- [ ] #5 All fixtures use the fictional cast
<!-- AC:END -->
