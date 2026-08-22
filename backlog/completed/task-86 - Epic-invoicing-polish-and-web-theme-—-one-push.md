---
id: TASK-86
title: 'Epic: invoicing polish and web theme — one push'
status: Done
assignee: []
created_date: '2026-08-11 19:39'
updated_date: '2026-08-20 14:31'
labels:
  - epic
  - invoicing
  - web
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Umbrella for the August push covering all open invoice and web tasks: TASK-63, TASK-64, TASK-67, TASK-69 through TASK-80 (excluding done TASK-65 and out-of-scope TASK-66, TASK-68.x).

Four workstreams, one Opus writer per worktree, orchestrator merges:

Stream 1 — Invoice engine correctness (TASK-69, TASK-71, TASK-63, TASK-70): validate_date zero-padding, update_invoice wall-clock today, encrypted-db integration test for invoice commands, clients.name UNIQUE decision. One functional PR.

Stream 2 — Publish, documents and send flow (TASK-67, TASK-64, TASK-78, TASK-79): published URL robustness + public_base_url validation, republish on payment, page/PDF field parity, preview-before-send. Sequential PRs on the render/send seam.

Stream 3 — Clients and email (TASK-80, TASK-74, TASK-73, TASK-77): email display name/from/reply-to, client delete on CLI+TUI, client archive, multi-email contacts schema. Sequential PRs.

Stream 4 — Web theme (TASK-72, TASK-75, TASK-76): fix unpainted dialogs (::part() never matches), dark/light mode switcher, mono typeface. Web-only, parallel to Rust streams.

Process: Opus agents write spec + implementation plan per stream, orchestrator approves each before implementation. Merge policy: functional-only PRs with clear AC and tests may be merged by the orchestrator; any PR with a visual component waits for Sam's review.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Every stream has an approved spec and plan in docs/superpowers/ before its implementation starts
- [x] #2 TASK-63, 64, 67, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80 are Done
- [x] #3 Visual PRs (streams 2 documents, 3 UI surfaces, 4 theme) merged only after Sam's review
<!-- AC:END -->
