---
id: TASK-91
title: 'Web UI: virtualize or lazy-load the register table'
status: Done
assignee: []
created_date: '2026-08-12 17:50'
updated_date: '2026-08-14 06:46'
labels:
  - web
  - ui
  - performance
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The register renders every row at once — an All Transactions view of 1,872 rows is fully materialized in the DOM and scrollable end to end. Investigate row virtualization (or windowed rendering) for wc-register-table so DOM size stays bounded; keyboard navigation, inline editing, search jump and scroll-to-today must keep working across virtual boundaries. Measure before/after on a multi-thousand-row register.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 DOM node count for the register stays bounded regardless of row count
- [x] #2 Keyboard navigation, inline editing, search and scroll-to-today behave identically to the unvirtualized table
- [x] #3 Before/after render and scroll performance is measured and recorded on a 1,800+ row register
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Measure the unvirtualized table on 1,872 rows first, so "before" is a number and not a memory.
2. Make every row one line tall (fixed table layout, colgroup, clip instead of wrap) — a window cannot place rows it has to measure.
3. Hand-roll the window: a slice plus overscan, two spacer rows carrying the missing height, a scroll listener.
4. Route the four behaviours that cross a boundary through coverIndex + scrollToRowIndex.
5. Re-measure with the identical script.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- No new dependency. The window is about 120 lines: `windowRange`, `coverIndex`, `scrollToRowIndex`, `handleScroll`, `remeasureRowHeight` and two spacer rows.
- Uniform row height is the enabling constraint: `table-layout: fixed` with a colgroup (description the only auto column, the `Fill(1)` the TUI gives it) and text cells that clip with an ellipsis. The full string stays in the DOM for a screen reader and on the cell title for a pointer. Wrapped rows would each need measuring, which is a different and much riskier design.
- Crossing a boundary is `coverIndex` (put the index in the DOM) then `scrollToRowIndex` (move the scroll box by arithmetic, since a row that is not rendered has no box to ask). Arrows, PgUp/PgDn, Home/End, `scrollToIndex` (scroll-to-today) and opening the editors all go through it.
- The row set is identified by length plus both end ids, not array identity: the screen rebuilds its filtered array on every render, so identity changes constantly and resetting on it would send the window to the top on every keystroke.
- A `ResizeObserver` on the host re-renders when the viewport changes size, since no scroll event follows a maximized browser.
- Known limitation, pre-existing and made no worse in kind: printing the register from the browser was already clipped by the 60vh scroller, and a windowed table prints its window. Register export (PDF/text) is the supported path.

Measured on 1,872 rows, jsdom, identical script both sides:

| | before | after |
|---|---|---|
| DOM nodes in the shadow root | 20,989 | 436 |
| tbody rows | 1,872 | 37 |
| first render | 2,414 ms | 106 ms |
| per arrow key | 73.7 ms | 4.6 ms |
| per scroll event | 0.02 ms | 6.7 ms |
| re-filter (one search keystroke) | 14,700 ms | 7.4 ms |
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The register table now windows: 1,872 rows put 37 in the DOM instead of 1,872, with no new dependency.

## How

Only the visible slice plus an eight-row overscan is rendered, with a spacer `<tr>` above and below carrying the height of what was left out — so the scrollbar still measures the whole register, and `aria-rowcount`/`aria-rowindex` still say where a row is. Under 120 rows nothing is windowed: the bookkeeping costs more than the rows do, and the reports screen read-only register is usually one month.

What pays for it is a **uniform row height**: `table-layout: fixed`, a `colgroup` where description is the only auto column (the `Fill(1)` the TUI gives the same column), and text cells that clip with an ellipsis instead of wrapping. The full string stays in the DOM for a screen reader and on the cell title for a pointer. Every row being one line tall is what lets the window place row *n* at `n * rowHeight` by arithmetic — the only way to reach a row that has no box to measure.

Crossing a boundary is two steps, in order: `coverIndex` puts the index in the DOM, `scrollToRowIndex` moves the scroll box. Arrow keys, PgUp/PgDn, Home/End, `scrollToIndex` (scroll-to-today) and opening the inline editors all go through the pair.

The row set is identified by length plus both end ids rather than array identity — the screen rebuilds its filtered array on every render — so a search goes back to the top of its results and a flag toggle does not.

## Measured, 1,872 rows, identical script both sides

| | before | after |
|---|---|---|
| DOM nodes in the shadow root | 20,989 | 436 |
| tbody rows | 1,872 | 37 |
| first render | 2,414 ms | 106 ms |
| per arrow key | 73.7 ms | 4.6 ms |
| per scroll event | 0.02 ms | 6.7 ms |
| re-filter (one search keystroke) | 14,700 ms | 7.4 ms |

Run under jsdom, so the absolute figures are jsdom figures; both columns come from the same script on the same machine, so the ratios are the finding. The scroll row is the cost windowing adds — the unwindowed table had no scroll handler, and paid instead by making the browser paint 1,872 rows.

## Tests

18 in `a register too big to put in the DOM`: bounded slice, slice size independent of row count, a short register rendered whole, spacers summing to the missing height, `aria-rowcount`/`aria-rowindex`, arrows past the window edge, End/Home, PgDn, scroll-to-today at index 1,500, editors on a row the window had left out, flagging a row that scrolled in, the window following the scroller, a search returning to the top, the same rows not moving it, a taller viewport drawing more rows, and the `virtualizeAbove` escape hatch. New preview state `windowed` (1,872 rows in a 22rem box), axe clean.

## Known limitation

Printing the register from the browser prints the window. It was already clipped to one screenful by the 60vh scroller before this change; register export (PDF/text) is the supported path for a printed register.

Verification: `npm run build`, `npm test` (188 + 1122 + 764, no unhandled errors), `npm run lint`, `npm run typecheck`, `cargo build`.
<!-- SECTION:FINAL_SUMMARY:END -->
