---
id: TASK-106
title: 'Invoicing: delete a draft invoice that was entered by mistake'
status: In Progress
assignee: []
created_date: '2026-08-14 00:05'
updated_date: '2026-08-14 04:56'
labels:
  - invoicing
  - cli
  - tui
  - web
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Void is the only way an invoice leaves the working set, and it is the wrong tool for one that should never have existed. Void is a statement: it writes voided_at, deactivates the Stripe payment link, and republishes the public page as a voided notice, precisely so a token URL a client filed still resolves to something honest. A draft created by mistake — wrong client, duplicated by a mis-keyed command, a test row on real books — has published nothing, told nobody, and deserves to leave without a tombstone.

This is the same distinction already settled for clients, where archive is for the client you finished working with and delete is for the one entered by mistake. Nothing analogous exists for invoices: there is no delete on the CLI, the TUI or the web, and no DELETE FROM invoices anywhere in the tree. The only route today is SQL against a backed-up database, clearing invoice_line_items and invoice_payments by hand first, outside every guard the data layer has.

Points the design has to settle rather than assume:

- What is deletable. A draft that was never published and carries no payments is the clear case. A published invoice must not be, because its token URL and its emailed PDF are already in somebody's hands — that is what void exists for. Whether a void invoice can be deleted is a real question: it is a record that something happened, which argues for keeping it.
- Whether the number is reused. next_invoice_number only moves forward, so deleting the most recent invoice leaves a gap unless the counter is rolled back. A gap in a numbering sequence is normal and auditable; reissuing a number that may already have been exported or referenced is not. Recommend leaving the counter alone and documenting why.
- The refusal shape. clients::delete_blocker and DeleteBlock already exist for exactly this, so a refused invoice delete should carry a machine-readable reason and count the way a refused client delete does, with one sentence shared by every surface.
- Cascade and atomicity. Line items go with the invoice; a guard should mean payments never exist on a deletable one, but the delete still belongs in a single transaction.
- Where it appears. CLI with a confirmation matching void's --yes shape, the TUI invoice detail view beside its existing send/pay/void actions, and DELETE /api/invoices/{number} on the web.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A draft invoice with no payments that was never published can be deleted
- [ ] #2 A published, paid or void invoice is refused, in one sentence shared by the CLI, the TUI and the web
- [ ] #3 The refusal carries a machine-readable reason the way a refused client delete does
- [ ] #4 Deleting removes the invoice and its line items in one transaction
- [ ] #5 Whether the invoice number is reused is decided, documented and tested
- [ ] #6 Deleting requires confirmation on the CLI and the TUI, consistent with void
<!-- AC:END -->
