---
id: TASK-77
title: 'Invoicing: a client needs more than one email address'
status: Done
assignee:
  - '@stream-3'
created_date: '2026-08-10 19:19'
updated_date: '2026-08-12 23:52'
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
- [x] #1 A client can hold more than one email address
- [x] #2 Exactly one address is identifiable as the billing recipient, and require_email is expressed in terms of it
- [x] #3 A sent invoice reaches the additional recipients — the Mailgun call carries them rather than a single To
- [x] #4 The design decision is recorded, including why a comma-separated column was or was not chosen
- [x] #5 Existing single-email clients migrate without anyone re-entering an address
- [x] #6 The CLI, the TUI and the web can all read and edit the full set, not just the first one
- [x] #7 The InvoiceShelf importer maps its one address to the new shape without losing it
- [x] #8 It is settled and documented whether a cc recipient can pay the invoice from the link they receive
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Migration **v8** creates `client_contacts` (one row per address: name, email,
title, `is_billing`, `position`) with a partial unique index for at most one
billing contact per client and an expression index for per-client
`lower(email)` uniqueness, backfills it from `clients.email` (trimmed, blanks
skipped) and drops that column. `ON DELETE CASCADE` is live — `db.rs` opens
every connection with `PRAGMA foreign_keys=ON` — so `delete_client` needed no
new statement, and a test pins it. A test also pins that the bundled SQLite is
past 3.35, which is what `DROP COLUMN` needs; the documented `RENAME COLUMN`
fallback was not needed.

`models::Client` is unchanged: `Client.email` became a correlated-subquery
projection of the billing contact, which is why every pre-existing test in
`clients.rs`, `cli/client.rs` and `routes/clients.rs` passes untouched and the
`clients.json`/`clients.txt` fixtures did not move. `set_billing_email` is the
one private writer `add_client`, `update_client` and the InvoiceShelf importer
go through; setting the email to an address the client already holds as a cc
moves the flag rather than colliding with the unique index, and clearing it
promotes the next contact by position.

`set_contacts` is a whole-list replacement validated before anything is
deleted; `validate_contacts` refuses a blank address, a duplicate under
`to_lowercase`, more than one `is_billing`, and any field carrying a control
character (these strings become mail headers). Normalization runs after
validation: a list naming no billing recipient makes its first row one.

`Mailer::send_invoice` gained `cc: &[String]`; `message_fields` emits one
comma-joined `cc` field and none at all when the list is empty.
`send::require_recipients` wraps `require_email` — so the refusal, its code and
its sentence are the ones that already ship — and formats each entry with
TASK-80's `format_address`, at the existing `Precheck` step, so no network call
happens before the recipients are known.

Surfaces: `--contact "email[:name[:title]]"` on `client add`/`edit`
(repeatable, whole-list, `conflicts_with = "email"`, `splitn(3, ':')` so a
title keeps a colon); a contacts table in `client show`; a `c` sub-screen in
the TUI client manager (a/e/d/b/Esc); `contacts` on `POST`/`PATCH
/api/clients` and on `ClientDetail`, with `email` + `contacts` in one body a
400; and on the web a contacts repeater in `wc-client-form` with a radio for
the billing recipient and up/down reordering, the edit dialog fetching
`GET /api/clients/{id}` first.

AC #8 settled and documented: every recipient gets the same document and can
pay it. One render, one message, `To` plus `Cc`, and the published page names
the billing contact alone.
<!-- SECTION:NOTES:END -->
