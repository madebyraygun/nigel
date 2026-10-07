---
id: TASK-147
title: Paginate the invoice list
status: To Do
assignee: []
created_date: '2026-10-05 17:10'
updated_date: '2026-10-07 18:44'
labels:
  - invoicing
  - web
milestone: m-3
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Invoices screen renders every invoice on one page, and GET /api/invoices returns every invoice in one response. A book with a few years of history is hundreds of rows, so the screen is slow to scan and gets slower with every billing cycle.

Paginate the list in the API and the web UI. Filters (status, client) and sort order must apply across the whole set, not just the page that is loaded, and the open-balance and status figures must stay consistent with nigel invoice list.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 GET /api/invoices accepts page/limit (or cursor) parameters and returns the total count alongside the page
- [ ] #2 The Invoices screen shows a fixed page size with next/previous controls and the current range (e.g. 1–50 of N)
- [ ] #3 Status and client filters apply to the full result set and reset to the first page when changed
- [ ] #4 The page survives a reload and back-navigation through the URL hash
- [ ] #5 docs/api.md documents the new parameters and response shape
- [ ] #6 Component previews cover first, middle, last and single-page states, and describePreviewA11y passes with zero violations
<!-- AC:END -->
