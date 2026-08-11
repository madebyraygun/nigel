---
id: TASK-68
title: 'Epic: Invoicing management surface — CLI, TUI, and web UI'
status: Done
assignee:
  - '@claude-orchestrator'
created_date: '2026-08-08 00:27'
updated_date: '2026-08-11 04:46'
labels:
  - epic
  - invoicing
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
PR #172 shipped invoicing as a create-and-send pipeline: client add/list, invoice new/list/show/send/sync/pay/aging/import. What's missing is the management layer around it — nothing can be edited or cancelled after creation (the void status exists in the data model with send/pay guards, but no command sets it, so a mistaken draft is permanent), there is no way to see an invoice before it goes to a client, the HTML/PDF templates are compiled into the binary, the TUI has no invoicing screens, and the web UI has no invoicing endpoints or screens.

This epic completes the surface in three layers: CLI first (data-layer operations every other front end reuses), then TUI screens following the manager-screen pattern, then web UI parity following the task-31 API/SPA conventions.

Folds in TASK-38 (notes/terms flags, client show) and TASK-62 (web UI invoicing), which predate it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Clients and draft invoices can be edited, invoices can be voided, and a client or invoice is inspectable from CLI, TUI, and web
- [x] #2 An invoice can be previewed (HTML and PDF) before anything is published or emailed
- [x] #3 Invoice HTML styling is customizable without rebuilding the binary
- [x] #4 TUI has screens for client management, invoice management, and the aging report
- [x] #5 Web UI reaches feature parity with the CLI invoicing surface
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
All nine subtasks done across 11 merged PRs (#183-185, #187-196). CLI: client show/edit, invoice edit/void with derived voided_at (migration v5), preview with zero network, template override with load-time validation and an 18-placeholder vocabulary, notes/terms end to end. TUI: client and invoice managers with send/pay/void and a draft form with repeatable line items; A/R aging as a full report with a dashboard summary line; demo seeds invoicing data. Web: full parity — wire-safe data layer (token never serialized), read/write API behind the standard guards, step-traced blocking send with wire-level confirm and bounded timeouts, budgeted sync, and SPA screens (11 new components, figure parity vs CLI, user-reviewed visually). Void tears down best-effort: Stripe link deactivated, published page replaced by a voided notice, warnings verbatim across all three front ends. Shared validation (items, payments, dates, names) lives in the data layer once. Follow-ups filed along the way: TASK-69 (date padding), TASK-70 (clients.name UNIQUE), TASK-71 (refresh_status today). Every PR passed an independent Opus review with verified fixes before merge.
<!-- SECTION:FINAL_SUMMARY:END -->
