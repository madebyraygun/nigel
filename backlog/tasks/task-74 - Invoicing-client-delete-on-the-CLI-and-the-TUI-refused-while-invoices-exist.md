---
id: TASK-74
title: 'Invoicing: client delete on the CLI and the TUI, refused while invoices exist'
status: Done
assignee:
  - '@stream-3'
created_date: '2026-08-09 00:46'
updated_date: '2026-08-12 23:52'
labels:
  - enhancement
  - invoicing
  - cli
  - tui
dependencies: []
documentation:
  - docs/superpowers/specs/2026-08-11-task-74-73-client-lifecycle-design.md
  - docs/superpowers/plans/2026-08-11-task-74-73-client-lifecycle.md
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Deleting a client is already built and shipping on the web, and it already refuses a client that has invoices. invoicing::clients::delete_blocker/delete_client hold the guard, DELETE /api/clients/{id} exposes it, and the clients screen offers it per row — a refusal answers 409 with reason has_invoices and the count, and links to the invoices behind it. Verified against the imported books: deleting a client with 8 invoices answers "Cannot delete: client has 8 invoices" with details.count = 8.

So the answer to "not possible if the client has invoices?" is yes, that is already the behaviour, and every status counts, not only unpaid.

What is missing is the other two surfaces. nigel client has add/show/edit/list and no delete. The TUI client manager (k on the dashboard) has add and edit and no delete — the reasoning recorded at the time was that a delete would be a no-op for every client anyone has ever billed, which is worth revisiting now that the web offers it and states the refusal plainly.

The data layer and the guard already exist, so this is surface work: no new refusal logic, and no new wording — the CLI and TUI should refuse in the same sentence the web already uses.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 nigel client delete <id> removes a client that has no invoices
- [x] #2 It refuses a client with invoices of any status, in the same words the web and the data layer already use, naming the count
- [x] #3 The refusal points at the invoices, the way the web guardrail does
- [x] #4 The TUI client manager offers delete with a confirmation, and refuses on the same terms
- [x] #5 No new guard logic is introduced — both surfaces call clients::delete_blocker/delete_client
- [x] #6 Deleting requires confirmation on the CLI, consistent with the other destructive commands
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Delete introduces no guard and no sentence. `nigel client delete <id> [--yes]`
and the TUI's `d` both call `clients::delete_blocker` and `clients::delete_client`
and print what those already say — the CLI adds only the pointer
`Run \`nigel client show <id>\` to see them.`, which is where the web guardrail
already points.

Both surfaces ask the block **before** the confirmation, so a client that cannot
be deleted never sees a dialog: the CLI exits non-zero with the block's sentence,
and the TUI sets it on the status line and stays on the list
(`account_manager`'s precedent).

`confirm_void`'s body moved to `cli::confirm_or_refuse(question, refusal, yes)`,
shared with `invoice void`, so the two destructive commands behave identically
on a pipe. The void wording is pinned byte-for-byte by
`invoice_void_requires_confirmation_without_a_tty`.

Migration: none for this half. Shipped in PR #199 with TASK-73.
<!-- SECTION:NOTES:END -->
