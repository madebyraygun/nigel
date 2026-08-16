---
id: TASK-109.5
title: 'HTTP API: /api/documents'
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
The route family, on the invoices routes' rules:

- `GET /api/documents` (status/kind/clientId filters; an unknown client is a 404, not an empty array — the `ensure_account_exists` reasoning) and `GET /api/documents/{id}` flattened, with `token` skipped and a computed `publicUrl` carried instead, and `can*` flags that are the data-layer guards *called*, never re-derived from status.
- `POST /api/documents/upload` through the uploads spool (multipart, PDF only by magic bytes, size-limited), then `POST /api/documents` filing the spooled upload — reusing `uploads.rs`, not duplicating it.
- `POST …/send` requiring `{"confirm": true}` (400 `confirmation_required` without it), answering the refreshed detail plus the step trace; failures map through the `SendFailure → ApiError` seam (502 with the upstream named, config refusals with missing key names only).
- `POST …/accept`, `…/decline`, `…/withdraw` (teardown warnings as data on a 200 — the void precedent), and `POST /api/documents/sync` with a deadline and per-document failures as data.
- Preview routes `…/{id}/preview` (CSP sandbox, `SAMEORIGIN`) and `…/{id}/preview.pdf` — the invoice preview precedent, no gateway reachable, 501 for PDF-less builds where that applies.
- Guardrail refusals as 409 `details.reason` with the new codes; every route behind the locked guard by default (`data_router`). `docs/api.md` updated.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every route validates then calls the same data layer the CLI uses; guardrail refusals carry structured details.reason with the Display sentences unchanged
- [ ] #2 Send requires confirm:true at the wire level, answering 400 confirmation_required without it
- [ ] #3 token never crosses the wire; a computed publicUrl carries the address instead
- [ ] #4 Upload reuses the uploads spool (sanitized name, 0600/0700, purge rules) and refuses non-PDF content
- [ ] #5 docs/api.md documents the new routes
<!-- AC:END -->
