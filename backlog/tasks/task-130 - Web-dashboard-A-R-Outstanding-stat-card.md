---
id: TASK-130
title: 'Web dashboard: A/R Outstanding stat card'
status: To Do
assignee: []
created_date: '2026-08-22 14:02'
labels:
  - frontend
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TUI home screen shows A/R Outstanding with the oldest-bucket note alongside YTD income/expenses/net (ar_summary in crates/nigel/src/cli/dashboard.rs). The SPA dashboard shows only the three YTD figures — dashboard-store.ts never fetches /api/invoices/aging, so a browser user has no at-a-glance receivables signal.

Prerequisite for deprecating the TUI dashboard; the data half of the dashboard gap (the voice half is TASK-116). The endpoint already exists (GET /api/invoices/aging).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The dashboard shows outstanding A/R with the oldest-bucket note, sourced from /api/invoices/aging
- [ ] #2 Books with no invoice activity render gracefully — no empty-noise card
- [ ] #3 The card links through to the invoices or aging view
<!-- AC:END -->
