---
id: TASK-73
title: 'Invoicing: archive a client to hide it from the list without deleting it'
status: To Do
assignee: []
created_date: '2026-08-09 00:46'
updated_date: '2026-08-11 20:03'
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
- [ ] #1 A client can be archived and unarchived
- [ ] #2 Archived clients are hidden from the default client list on every surface that lists clients — CLI, TUI and web
- [ ] #3 Archived clients remain visible wherever their invoices are shown, including the invoice list and the aging report
- [ ] #4 Archiving is not deletion: the row, its invoices, its payments and its history are all untouched
- [ ] #5 An archived client cannot be the target of a new invoice, or the refusal names the reason
- [ ] #6 The list can be asked to include archived clients
<!-- AC:END -->
