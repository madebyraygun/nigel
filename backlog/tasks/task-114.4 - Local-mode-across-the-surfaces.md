---
id: TASK-114.4
title: Local mode across the surfaces
status: To Do
assignee: []
created_date: '2026-08-17 13:27'
updated_date: '2026-08-21 00:21'
labels:
  - invoicing
  - documents
  - spa
  - desktop
milestone: m-0
dependencies: []
parent_task_id: TASK-114
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Every surface sends, attests and degrades the same way.

- CLI, TUI, SPA and desktop all drive the local send: confirmation, compose, the attested mark-sent step, the outbox path shown where the public URL would be.
- Absent capabilities are absent quietly on every surface (the PayButton live/inert/absent precedent): no pay button, no accept form, no sync action in local mode — the control does not render, and no surface offers an action that cannot complete. Guards keep living in the data layer so all four surfaces refuse in the same words.
- Recurring invoices degrade to prepare-and-prompt: the schedule still fires, but produces a ready-to-send draft and a prompt rather than an automated send, on every surface that shows recurring state.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The full local send flow works from CLI, TUI, SPA and desktop with the same confirmation and attestation steps
- [ ] #2 Capabilities absent in local mode render nowhere, and no surface offers an action that cannot complete
- [ ] #3 Recurring invoices in local mode produce a prepared draft and a prompt, never an automated send
<!-- AC:END -->
