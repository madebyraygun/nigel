---
id: TASK-96
title: 'Web UI: center the Nothing-to-review empty state'
status: In Progress
assignee: []
created_date: '2026-08-12 17:51'
updated_date: '2026-08-14 05:13'
labels:
  - web
  - ui
  - bug
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The 'Nothing to review' empty state renders in the upper third of the content area, left of center (screenshot). Center the empty-state panel horizontally and vertically in the available content area, and check the other screens using wc-empty-state for the same drift.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The review empty state is centered in the content area
- [x] #2 Other wc-empty-state consumers are checked and consistent
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The empty state had nowhere to centre itself: `wc-app-shell` let the screen be as tall as its own content, and the review screen was a grid pinned with `align-content: start` inside a `max-width: 52rem` host — top of the area, left of centre.

Fixed structurally, in three parts:

- `wc-app-shell` — the content area is a flex column and `.content ::slotted(*)` gets `flex: 1 1 auto`, so every screen (present and future) is stretched to the whole area. Content taller than the area still grows past it and scrolls, since a flex item's automatic minimum size is its content. The `@media print` block already overrides `display`, so paper is untouched.
- `wc-empty-state` — `align-content: center` beside the existing `place-items: center` (centred on both axes in whatever box it gets) plus `flex: 1 1 auto` (in a flex column it takes the height nothing else claimed). In a content-sized box both are no-ops, which is why the same element serves a whole screen and a panel body.
- Screens — every screen element is now a flex column, which is what carries the height from the shell to the empty state. `review.ts` also moves its 52rem reading width off the host onto the things being read (`wc-review-progress`, `wc-review-card`, `wc-review-form`, `wc-panel`); a reading width on the host is what put the panel left of centre. `wc-manager-layout` and `wc-register-table` say `flex: 1 1 auto` as well, so the manager screens' `empty` slot and the register's own empty row centre in the space under their headers.

Verified in a real browser (headless Chrome against the built SPA at `#/review`): before, the panel sat at x 629 / y 200 in a 232–1280 x 88–800 content area; after, x 756 / y 445 — the centre of the area on both axes. Accounts, register, reports, dashboard, reconcile, settings, undo and invoices were screenshotted too; nothing else moved.

Tests: `preview/layout-suite.ts` is a shared suite in the shape of `print-suite.ts` — it resolves a component's own rule through the CSSOM and reads back the values a browser would hand to layout, because jsdom lays nothing out. `apps/app/src/__tests__/screen-layout.test.ts` is the guard that keeps a new screen from stacking its children some other way.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Centres the "Nothing to review" panel — and every other whole-screen empty state — in the content area, by fixing the box it is centred in rather than the panel.

`wc-app-shell`'s content area is a flex column that stretches whatever is in its default slot to the whole area, so a screen has room to centre something in; `wc-empty-state` centres its content on both axes in the box it is given and asks a flex column for the height nothing else claimed; and screens are flex columns, which is what carries the height between the two. The review screen's 52rem reading width moves off the host onto the things being read, since a reading width on the host is what put the panel left of centre. `wc-manager-layout` and `wc-register-table` take the leftover height too, so the manager screens' empty slot and the register's own empty row centre in the space under their headers. In a content-sized box — a panel body, the import and reconciliation histories, the dashboard's cash-flow slot — growing and centring are both no-ops, so those read exactly as they did.

All sixteen `wc-empty-state` call sites were checked: review (empty and error), dashboard (no books), invoices (error), register (error), reports (error), accounts, categories, rules and clients (manager `empty` slot) and `wc-register-table` now centre in the area left to them; reconcile's "No accounts yet", the dashboard's cash-flow slot, `wc-import-history` and `wc-reconciliation-history` sit in panels and are unchanged by design.

Verified in headless Chrome against the built SPA: the panel moves from x 629 / y 200 to x 756 / y 445 in a 232–1280 by 88–800 content area — centred on both axes — with eight other screens screenshotted and unmoved. `preview/layout-suite.ts` resolves a component's own rule through the CSSOM (jsdom lays nothing out) and `apps/app/src/__tests__/screen-layout.test.ts` fails a screen that stacks its children some other way, so a new screen gets the centring without asking for it. web: 2020 tests green, lint and typecheck clean.
<!-- SECTION:FINAL_SUMMARY:END -->
