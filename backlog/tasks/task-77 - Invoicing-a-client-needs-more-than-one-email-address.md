---
id: TASK-77
title: 'Invoicing: a client needs more than one email address'
status: To Do
assignee: []
created_date: '2026-08-10 19:19'
updated_date: '2026-08-11 20:03'
labels:
  - enhancement
  - invoicing
  - schema
dependencies: []
documentation:
  - docs/superpowers/specs/2026-08-11-task-77-client-contacts-design.md
  - docs/superpowers/plans/2026-08-11-task-77-client-contacts.md
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
clients.email is a single TEXT column, so a client has exactly one address and an invoice goes to exactly one person. Real client records do not look like that — an organisation typically has a billing or AP contact plus the person who commissioned the work, and both want the invoice.

Measured against the Harvest contact export used to backfill these addresses: 123 clients carry 187 contact emails. 41 of those clients (a third) have more than one, 105 of the 187 addresses belong to a multi-contact client, and the largest has nine. Of the 23 clients imported from InvoiceShelf, 12 had two or three Harvest contacts and a person had to choose one by hand, discarding the rest. That choice is currently unrecorded and unrecoverable — the column keeps the winner and nothing remembers there were others.

Worth noting Harvest models this the way we probably want to: contacts are their own records, each with a name and title, and the client points at one of them as the invoice default. Our export had that flag empty on all 187 rows, which is why the backfill needed manual picks.

Surfaces that assume one address: the clients schema and its migration, invoicing::clients (add_client/update_client/client_summary), require_email and the Conflict { code: client_missing_email } guard, mailgun.rs (a single To with no cc), render_html.rs and the published page contact block, nigel client add/edit --email, the TUI client form, the Client serde struct and PATCH /api/clients/{id}, wc-client-form, and import_invoiceshelf.rs.

Design is the substance of this task, not the plumbing. Options include a primary address plus a cc list, or a client_contacts table with a billing-default flag mirroring Harvest. A comma-separated column is the tempting shortcut and should be rejected explicitly if so, because require_email and the Mailgun To both need to know which address is which.

Open questions to settle before implementing: what require_email means when a client has contacts but none is marked for billing; whether a cc recipient sees the same payment link (they can pay, which may be intended); whether the published invoice page names one contact or none; and how an existing single email migrates without a person re-picking all 23.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A client can hold more than one email address
- [ ] #2 Exactly one address is identifiable as the billing recipient, and require_email is expressed in terms of it
- [ ] #3 A sent invoice reaches the additional recipients — the Mailgun call carries them rather than a single To
- [ ] #4 The design decision is recorded, including why a comma-separated column was or was not chosen
- [ ] #5 Existing single-email clients migrate without anyone re-entering an address
- [ ] #6 The CLI, the TUI and the web can all read and edit the full set, not just the first one
- [ ] #7 The InvoiceShelf importer maps its one address to the new shape without losing it
- [ ] #8 It is settled and documented whether a cc recipient can pay the invoice from the link they receive
<!-- AC:END -->
