---
id: TASK-106
title: 'Invoicing: delete a draft invoice that was entered by mistake'
status: Done
assignee: []
created_date: '2026-08-14 00:05'
updated_date: '2026-08-14 17:55'
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
- [x] #1 A draft invoice with no payments that was never published can be deleted
- [x] #2 A published, paid or void invoice is refused, in one sentence shared by the CLI, the TUI and the web
- [x] #3 The refusal carries a machine-readable reason the way a refused client delete does
- [x] #4 Deleting removes the invoice and its line items in one transaction
- [x] #5 Whether the invoice number is reused is decided, documented and tested
- [x] #6 Deleting requires confirmation on the CLI and the TUI, consistent with void
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Delete is now the counterpart to void: void is for the invoice a client has seen, delete is for the draft that should never have existed.

## The rule

An invoice is deletable only while it is a draft that was never published and carries no payments. Published, sent, partial, paid, overdue and **void** all refuse — each means somebody outside this machine has seen it, or that the row is a record something happened, and the tombstone argument wins for void.

The guard is `invoicing::invoices::delete_blocker`, in the `clients::delete_blocker`/`DeleteBlock` mold, so a caller reaching the data layer directly cannot bypass it. Every refusal is one reason (`not_deletable`) and one sentence, written once and printed verbatim by the CLI, the TUI and the API:

    Cannot delete: invoice has been sent, paid or voided — only an unsent draft with no payments can be deleted

`delete_invoice` runs that guard **inside** its own transaction, next to the line-item cascade, so a delete races nothing. Payments are asserted absent rather than cascaded: a payment row of exactly 0.00 is the case a `paid_amount > 0.0` guard alone would let through.

## The number is not reused

`next_invoice_number` is deliberately untouched. A gap in a numbering sequence is normal and auditable; reissuing a number that may already have been exported or quoted is not. Pinned by `deleting_the_newest_draft_does_not_move_the_invoice_number_counter`, by a CLI test that drafts again and gets the next number, and by an API test on `GET /api/invoices/next-number`. `nigel invoice delete` says so on success.

## Surfaces

- **CLI** — `nigel invoice delete <number>` with `--yes`, through `cli::confirm_or_refuse`, so it refuses on a pipe exactly as `invoice void` and `client delete` do. The blocker is asked before the prompt, so a refusable invoice is never offered a dialog; the "void it instead" pointer is suppressed for an invoice that is already void.
- **TUI** — `d` on the invoice detail view beside `s`/`p`/`v`. One-phase, not two: a delete reaches no network, so a "Deleting…" frame would promise a wait that never happens. The footer advertises `d` only when `Detail.deletable` — `delete_blocker` called at load time — says so.
- **API** — `DELETE /api/invoices/{number}` answering the house `{ id, deleted }` body, a 409 carrying the data layer's reason and sentence, and a 404 `invoice_not_found`. `canDelete` joins the detail's `can*` flags as that same guard called.
- **SPA** — a Delete… action on the invoice detail gated by the server's `canDelete`, behind `confirmDialog`. A success navigates back to the list with a toast, because a route change drops the notice state a success message would have lived in; a 409 lands in the danger notice above the invoice, rendered from `invoicing-errors.ts`'s own sentence rather than the server's.

## Type change

`BlockReason` now carries its count on the variants that count something (`HasTransactions`/`HasActiveRules`/`HasInvoices`), so `NotDeletable` can carry none and the API stops putting a `"count": 0` nobody chose on the wire.

## Tests

- `cargo test -- --test-threads=1` — 1399 lib + 123 cli_dispatch, 0 failed
- `cargo test --no-default-features -- --test-threads=1` — 0 failed
- `cargo test --no-default-features --features serve -- --test-threads=1` — 0 failed
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` — clean
- `web/`: `npm test` 188 + 1058 + 766 passed, no Unhandled Errors; `npm run build`, `npm run lint`, `npm run typecheck` clean

Docs: docs/invoicing.md (the rule, the refusal table, the counter decision), docs/api.md (the DELETE route, `canDelete`, `not_deletable` in the conflict table), CLAUDE.md, README.md.
<!-- SECTION:FINAL_SUMMARY:END -->
