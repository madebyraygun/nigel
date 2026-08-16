---
id: TASK-109.6
title: 'TUI: documents manager screen'
status: To Do
assignee: []
created_date: '2026-08-16 04:22'
labels:
  - documents
dependencies: []
parent_task_id: TASK-109
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`cli/document_manager.rs` on the `invoice_manager` pattern: a dashboard key on the Home chooser, a list (kind, title, client, status, date) opening a detail view (file, dates, acceptance record, public URL once published), and the actions on the detail only — send, accept, decline, withdraw — each behind a confirmation, each pre-flighted through the same data-layer guards the CLI uses so both front ends refuse the same things in the same words.

- Network actions are two-phase (`InvoiceAction::Perform` precedent): the key handler returns the intent, the dashboard paints the blocking frame, then performs, then drains buffered input so a mashed Enter cannot dismiss the result unread.
- Guards are asked *before* a confirmation is offered (the client_manager delete precedent) — a document that would refuse gets the block's sentence on the status line, never a dialog that would fail.
- Filing a document is a form (file path input + client selector + kind selector + title), `import_manager`'s shape.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The screen lists, files, and drives the full lifecycle with the same refusal sentences the CLI prints
- [ ] #2 Send, withdraw and sync paint a blocking frame before network work and drain buffered input after (the two-phase Perform precedent)
- [ ] #3 Guards are asked before a confirmation is offered, so a refusal lands on the status line and no dialog can fail
- [ ] #4 The dashboard Home menu and CLAUDE.md name the new key
<!-- AC:END -->
