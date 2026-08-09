---
id: TASK-74
title: 'Invoicing: client delete on the CLI and the TUI, refused while invoices exist'
status: To Do
assignee: []
created_date: '2026-08-09 00:46'
labels:
  - enhancement
  - invoicing
  - cli
  - tui
dependencies: []
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
- [ ] #1 nigel client delete <id> removes a client that has no invoices
- [ ] #2 It refuses a client with invoices of any status, in the same words the web and the data layer already use, naming the count
- [ ] #3 The refusal points at the invoices, the way the web guardrail does
- [ ] #4 The TUI client manager offers delete with a confirmation, and refuses on the same terms
- [ ] #5 No new guard logic is introduced — both surfaces call clients::delete_blocker/delete_client
- [ ] #6 Deleting requires confirmation on the CLI, consistent with the other destructive commands
<!-- AC:END -->
