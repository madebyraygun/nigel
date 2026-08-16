---
id: TASK-89
title: 'Web UI: register table should extend the full height of the window'
status: Done
assignee: []
created_date: '2026-08-12 17:50'
updated_date: '2026-08-14 04:56'
labels:
  - web
  - ui
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On the register screen the table's scroll container ends partway down the viewport and dead space runs below it (screenshot: 1872-row register, table stops ~70% down the window). The table should flex to fill the available height between the toolbar and the Net footer, with its own scrollbar.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The register table fills the vertical space between the toolbar and the bottom of the window at any viewport height
- [x] #2 The Net summary row stays visible (pinned or at table end) and no dead space renders below the table
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Make wc-app-shell content area a flex column so a screen can be handed a definite height.
2. Give wc-register-table a `fill` mode: host grows, scroller takes the leftover height, cap removed.
3. Make nigel-register-screen a flex column that grows, with the table filling it.
4. Cover it: paging follows the measured scroller viewport; fill reflects; the height rules are asserted (jsdom has no layout engine).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- wc-app-shell: `.content` is now `display: flex; flex-direction: column; min-height: 0` beside its existing `flex: 1; overflow: auto`. Without this a slotted screen has no definite height to divide.
- wc-register-table: new reflected `fill` property. `:host([fill])` grows, `:host([fill]) .scroller` grows with `min-height: 0` (the automatic minimum would otherwise size the scroller to every row) and `max-height: none`. Unset, the 60vh cap stays — the reports screen embeds this table in a page that scrolls as a whole.
- nigel-register-screen: `:host` is a growing flex column instead of `display: grid; align-content: start`, and passes `fill` to the table.
- The Net row was already `position: sticky; bottom: 0` inside the scroller, so filling is what makes it land at the bottom of the window.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The register table now ends at the bottom of the window instead of partway down it.

What changed:
- `wc-app-shell`'s content area became a flex column, so a screen can grow into it. Screens that do not ask stay content-sized, as block children were.
- `wc-register-table` gained a reflected `fill` attribute: the host grows, the scroller takes the leftover height with `min-height: 0`, and the `--nc-register-height` cap (60vh) comes off. Unset, nothing changes — the reports screen's read-only register still sizes to its rows inside a page that scrolls as a whole.
- `nigel-register-screen` is a growing flex column and passes `fill`, so the toolbar keeps its height and the table gets everything under it. The Net row was already sticky to the bottom of the scroller, so it now sits at the bottom of the window with nothing below it.

Tests:
- `pages by the rows its own scroller shows, not by a fixed count` — stubs two different viewport heights and asserts PgDn moves by what actually fits, which is the behaviour "at any viewport height" produces.
- `reflects fill, which is what the height rules select on`, `hands the table the height left under the toolbar` (screen).
- `grows into the space a flex-column parent has left, and scrolls inside it`, `keeps the capped, content-sized shape when it is not filling`, `keeps the Net row against the bottom of the scroller`, `lets a screen ask for the whole window below the header`, `still scrolls a screen that is taller than the window`. These read the adopted stylesheet: the UI package runs under jsdom, which has no layout engine, so the repo already asserts layout this way for the print sheet.
- New preview state `filling-its-parent` (32 rows in an 18rem flex column), covered by `describePreviewA11y`.

Verification: `npm run build`, `npm test` (1066 + 761 + 188 passing, no unhandled errors), `npm run lint`, `npm run typecheck`, `cargo build`.
<!-- SECTION:FINAL_SUMMARY:END -->
