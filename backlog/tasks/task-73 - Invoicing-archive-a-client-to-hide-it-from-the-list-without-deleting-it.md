---
id: TASK-73
title: 'Invoicing: archive a client to hide it from the list without deleting it'
status: Done
assignee:
  - '@stream-3'
created_date: '2026-08-09 00:46'
updated_date: '2026-08-12 23:52'
labels:
  - enhancement
  - invoicing
dependencies: []
documentation:
  - docs/superpowers/specs/2026-08-11-task-74-73-client-lifecycle-design.md
  - docs/superpowers/plans/2026-08-11-task-74-73-client-lifecycle.md
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A client that is no longer billed should be able to leave the working list without leaving the database. Deleting is the wrong tool: a client with invoices must not disappear from under them, and the invoice history has to keep naming who it billed.

This is the natural companion to delete rather than a replacement for it — archive is for the client you finished working with, delete is for the one entered by mistake. The InvoiceShelf import brought in 23 clients at once, most of them historical, which is what makes the list worth filtering.

Needs a decision on scope before implementation: archived is a column on clients plus a default filter, and it has to be answered consistently by the CLI list, the TUI client manager and GET /api/clients. Note that the aging report and the invoice list must keep showing an archived client name wherever its invoices appear.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A client can be archived and unarchived
- [x] #2 Archived clients are hidden from the default client list on every surface that lists clients — CLI, TUI and web
- [x] #3 Archived clients remain visible wherever their invoices are shown, including the invoice list and the aging report
- [x] #4 Archiving is not deletion: the row, its invoices, its payments and its history are all untouched
- [x] #5 An archived client cannot be the target of a new invoice, or the refusal names the reason
- [x] #6 The list can be asked to include archived clients
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Migration **v6** adds `clients.archived_at`, a nullable timestamp following
`voided_at`'s derived-state precedent, probed before the `ALTER` so a replay is
harmless. No backfill: every existing client is active, which NULL already says.

`invoicing::clients` gained `ClientScope::{Active,All}` (no default — every
surface states the scope it wants), `archive_client` (idempotent by
`AND archived_at IS NULL`, so archiving twice keeps the first date),
`unarchive_client`, and `ensure_client_active`, which `create_invoice` calls
beside `ensure_client_exists`, before its transaction opens. The refusal is
`Conflict { code: "client_archived" }` naming the client.

Archiving changes no figure: `list_invoices`, `ar_aging_detail` and
`client_summary` are untouched, and a test pins that an archived client's name
still appears in the invoice list and on the aging report.

Surfaces: `nigel client archive|unarchive`, `nigel client list --all` (the
Archived column appears only when the slice carries an archived row, so the
default output and the `clients.txt` fixture are byte-identical); the TUI's `x`
and `A` with a `(archived)` marker inside the name column's own budget;
`GET /api/clients?includeArchived=true` plus `POST /api/clients/{id}/archive`
and `…/unarchive`; and on the web a `#/clients?archived=1` filter, a per-row
Archive/Unarchive action (`ManagerRow.actions`, new) and a `wc-row-badge` in
the name cell.

`server::testutil::seed_invoicing` gained an archived fourth client, named last
alphabetically and given no invoices, so the fixtures cover the state without
moving the default-scope list.
<!-- SECTION:NOTES:END -->
