---
id: TASK-154.2
title: 'App shell: floating sidebar, breadcrumb top bar and window title'
status: Done
assignee: []
created_date: '2026-10-20 22:57'
updated_date: '2026-10-21 14:29'
labels:
  - web
  - design
milestone: m-3
dependencies:
  - TASK-154.1
references:
  - docs/design/app-refresh/README.md
parent_task_id: TASK-154
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Spec sections 1 (App shell) and the interactions summary. The sidebar becomes an inset rounded panel (224px, collapsing to 58px over 180ms, collapsed state persisted) with the hamburger and gradient wordmark in its header, grouped nav (no label; Statements; Billing; Setup), the active item's rainbow hairline, Review's count, and Settings pinned in the footer. The company name leaves the sidebar for a breadcrumb above the content: monogram tile, book name, screen, and an Invoices / #n level on invoice detail. The top bar carries the search pill (opens the palette) and refresh. The window title reads '{Screen} · {Book name}'. A screen change resets main's scroll. In the Swift shell's hosted mode (TASK-144.5) the sidebar is hidden and the breadcrumb stays.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The shell matches the spec's layout, sizes and states in light and dark, expanded and collapsed, and the collapsed state survives a reload
- [x] #2 The breadcrumb shows the open book's monogram and name and the current screen, with the invoice-detail level; until book switching lands, clicking the book opens the palette
- [x] #3 The window title follows '{Screen} · {Book name}' in the browser and in Tauri
- [x] #4 Hosted mode hides the sidebar and keeps the breadcrumb and top bar
- [x] #5 Every new or changed @nigel/ui component has a preview covering its states in light and dark, and describePreviewA11y passes with zero violations
- [x] #6 Inputs, selects and textareas draw the spec's focus ring (border --br plus a 3px --nc-color-ring halo) through controlsCss, and buttons use :focus-visible with a 2px brand outline
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in PR #59.
<!-- SECTION:FINAL_SUMMARY:END -->
