---
id: TASK-70
title: 'Invoicing: decide on a UNIQUE index for clients.name'
status: In Progress
assignee:
  - '@stream-1'
created_date: '2026-08-08 08:21'
updated_date: '2026-08-11 21:25'
labels:
  - invoicing
dependencies: []
documentation:
  - >-
    docs/superpowers/specs/2026-08-11-task-69-71-63-70-invoice-correctness-design.md
  - docs/superpowers/plans/2026-08-11-task-69-71-63-70-invoice-correctness.md
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
add_client/update_client refuse duplicate names in the data layer (advisory — two racing web clients can still both insert, since clients.name carries no UNIQUE constraint). Decide whether to add the index by migration: existing databases (and InvoiceShelf imports) may already hold duplicates, so the migration needs a dedup or rename strategy before the constraint can land. Surfaced during TASK-68.6 stage 3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Either a UNIQUE index exists with a migration that handles pre-existing duplicates, or the advisory-only behavior is documented as deliberate
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decision: advisory-only, no UNIQUE index, no migration. Orchestrator confirmed, and explicitly ruled out the conditional-INSERT race fix as well.

Four grounds:
1. It is the house pattern. accounts.name and categories.name are TEXT NOT NULL with no unique index, each guarded by the same kind of data-layer check that clients::name_taken's own doc comment cites. Every UNIQUE in this schema is on machine-generated identity (invoices.number, invoices.token, invoice_payments.stripe_checkout_session_id, csv_profiles.name). Constraining all three is a larger task, and would be actively wrong for categories, which soft-delete — a retired Travel and a new Travel must coexist.
2. Nothing resolves a client by name. The only WHERE name = ? on clients in production code is name_taken itself; invoices carry client_id. A duplicate is a confusing picker entry, not a wrong figure or a broken join.
3. import_invoiceshelf inserts customers with raw SQL on purpose — it is a faithful copy of a system that does not guarantee unique names. Under a UNIQUE index one duplicated customer aborts the entire one-time migration, since it is all one transaction.
4. A rename migration would rewrite a name already printed on invoices that have been published and emailed. Compare v6's date migration, which changes no meaning at all — that is the bar a schema-driven data rewrite has to clear.

Also rejected: COLLATE NOCASE, which is stricter than name_taken's binary = and would raise a raw constraint violation (a 500 over HTTP) where the data layer says the name is fine.

The race is left open knowingly: two POST /api/clients against a loopback-bound single-user server, and the cure is renaming one row. The conditional-INSERT fix has no deterministic regression test — proving it needs two threads and a barrier, and such a test passes by luck against unfixed code.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
clients.name uniqueness stays advisory, recorded as deliberate.

AC #1 satisfied by the second branch: no UNIQUE index; the advisory behavior is documented as a decision in three places rather than left as an accident of the schema.

- CLAUDE.md gains a Key Design Constraint carrying the reasoning, so the next reader does not re-litigate it from the schema.
- docs/invoicing.md's Clients section says where the rule lives, what it does not cover, and what a user can actually hit (two clients with one name after a racing web write; rename one).
- two_source_customers_with_the_same_name_import_as_two_clients pins the InvoiceShelf case a UNIQUE index would break — the decision written down where the compiler can see it.

No schema change, no migration, no production code change.
<!-- SECTION:FINAL_SUMMARY:END -->
