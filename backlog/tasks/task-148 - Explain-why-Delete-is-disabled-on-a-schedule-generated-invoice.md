---
id: TASK-148
title: Explain why Delete is disabled on a schedule-generated invoice
status: To Do
assignee: []
created_date: '2026-10-07 18:32'
labels:
  - invoicing
  - web
milestone: m-0
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel/issues/45'
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On the web invoice detail screen, Delete… is disabled for a schedule-generated invoice with no explanation, while the disabled Send… beside it explains itself through sendUnavailableNote() (web/apps/app/src/screens/invoices.ts).

InvoiceDetail.canDelete (web/apps/app/src/api/types.ts) is a bare boolean, so the payload cannot say why. The reason exists only on the refusal: DELETE /api/invoices/{n} answers 409 with details { reason: "from_schedule", canVoid: true }. The CLI already explains the refusal and points at invoice void.

Repro: create a schedule, run nigel invoice schedule run, open a generated invoice in the web UI.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Disabled Delete… on a schedule-generated invoice explains why and points at void
- [ ] #2 The explanation follows the Send… unavailable pattern on the same screen
- [ ] #3 The invoice detail API exposes why delete is blocked, documented in docs/api.md
- [ ] #4 Tests cover the new field and the note; lint and typecheck pass
<!-- AC:END -->
