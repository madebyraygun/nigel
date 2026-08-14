---
id: TASK-90
title: >-
  Web UI: register keyboard shortcuts broken, and the shortcut legend should be
  a popover
status: Done
assignee: []
created_date: '2026-08-12 17:50'
updated_date: '2026-08-14 05:09'
labels:
  - web
  - ui
  - bug
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Two defects on the register screen. (1) The documented register shortcuts (arrows, PgUp/PgDn, Home/End, Enter, Esc, f, /) do not work — keystrokes do nothing on the table. (2) The 'Keyboard' disclosure expands inline as a plain block that overlaps/pushes the layout (screenshot); it should render as a proper popover anchored to the trigger, dismissable with Esc and outside-click, keyboard reachable.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Every shortcut the legend lists works on the register table
- [x] #2 The legend renders as an anchored popover that does not disturb the page layout
- [x] #3 The popover is keyboard reachable and dismissable (Esc, outside click), and axe passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Diagnose why the keys are dead before changing anything — write throwaway assertions against the current component.
2. Fix the cause: listener placement, the roving tabindex home, and Escape destroying the tab stop.
3. Land the cursor on the opening row after a load, without stealing focus.
4. Replace the inline `details` legend with an anchored popover component in @nigel/ui.
5. Drive the legend off the table own key list, so a documented key that does nothing fails the suite.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Root cause (confirmed against the unfixed component before touching it):
- The `keydown` listener sat on the `.scroller` div inside the shadow root. Nothing gives that div focus, so keys only arrived when a row had focus.
- The roving tabindex was `tabindex=${selected ? 0 : -1}` and `activeId` starts null. With nothing selected — every date-filtered register, since `landOnOpeningRow` returns early when a date filter is in play — **no element in the table was in the tab order**, so Tab could never reach it and no key could ever fire.
- On an unfiltered register a row was selected but never focused, so focus sat on `document.body` and the arrows did nothing until the user clicked a row or tabbed past the account select, the period pager and the search box.
- `Escape` cleared `activeId`, which removed the table only tab stop; after one Esc the register was unreachable for the rest of the session.

Fix: listener on the host (a native key event is `composed`, so it crosses the boundary); `tabStopId` falls back to the first row; `Escape` left to the two inline editors, which is what the legend says it does; `focusSelectedRow()` called by the screen after a load, only when `document.activeElement === document.body`.

Legend: `REGISTER_SHORTCUTS` lives beside the switch in `wc-register-table.ts` and carries real `KeyboardEvent.key` values. New `wc-shortcut-help` renders it as an absolutely-positioned panel behind a plain `<button>` (a wa-button keeps its own button in a shadow root, and `aria-expanded`/`aria-controls` have to land on the real one).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The register keys work, and the legend no longer moves the page when you open it.

## Why they were dead

Three things, each fatal on its own, confirmed against the unfixed component before anything changed:

1. The `keydown` listener was on the `.scroller` div inside the shadow root. Nothing focuses that div, so a key only arrived when a row already had focus.
2. The roving tabindex was tied to the selection (`tabindex=${selected ? 0 : -1}`) and `activeId` starts null. A register opened with a date filter selects nothing, so **no element in the table was in the tab order** — Tab could not reach it and no key could ever fire.
3. `Escape` cleared the selection, which removed the only tab stop. One Esc and the table was unreachable for the rest of the session.

On an unfiltered register a row *was* selected on load but never focused, so focus sat on `document.body` and the arrows did nothing until the user clicked a row or tabbed past the account select, the period pager and the search box.

## The fix

- The listener moved to the host. A native key event is `composed`, so it crosses the shadow boundary; a key now fires from the focused row, the flag button on it, or the scroller.
- `tabStopId` falls back to the first row, so the roving tabindex always has a home.
- `Escape` is left to the two inline editors, which is what the legend says it does.
- `focusSelectedRow()` lands the cursor after a load — only when `document.activeElement` is still `body`, so a reload never pulls the caret out of the search box.

## The legend

`REGISTER_SHORTCUTS` now lives beside the switch that implements it and carries the real `KeyboardEvent.key` values. New `wc-shortcut-help` renders it: a plain `<button>` (a `wa-button` keeps its own button inside a shadow root, and `aria-expanded`/`aria-controls` have to land on the real one) over an absolutely-positioned panel, so opening it moves nothing. Escape and an outside pointerdown both close it from document capture listeners, and closing returns focus to the trigger. A disclosure rather than a dialog, because the panel is a definition list with nothing focusable in it.

## Tests

- `the documented shortcuts` — one proof per `KeyboardEvent.key` in `REGISTER_SHORTCUTS`, plus `is a list every key on which has a proof below`, which fails if a legend line gains or loses a key without a proof.
- `reaching the register from the keyboard` — in the tab order before anything is selected; exactly one tab stop; a tab stop survives Escape; the stop moves with the selection; a key on the flag button still works; `focusSelectedRow` on a selected row, on a fresh table, and on an empty one.
- `wc-shortcut-help` — 19 tests: opens/closes on the trigger, Escape from anywhere, outside pointerdown, a press on the panel does not close it, focus returns to the trigger, unique panel ids, listeners removed on disconnect, and the anchored-not-inline style contract.
- Screen: `offers the legend as a popover carrying the table own list of keys` (and no `details` left), `lands the keyboard on the opening row`, `leaves focus where it is when something else already has it`.
- axe: four `wc-shortcut-help` preview states, zero violations.

Verification: `npm run build`, `npm test` (188 + 1104 + 764, no unhandled errors), `npm run lint`, `npm run typecheck`, `cargo build`.
<!-- SECTION:FINAL_SUMMARY:END -->
