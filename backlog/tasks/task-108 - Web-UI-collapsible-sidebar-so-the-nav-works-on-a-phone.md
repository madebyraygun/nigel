---
id: TASK-108
title: 'Web UI: collapsible sidebar so the nav works on a phone'
status: To Do
assignee: []
created_date: '2026-08-15 18:05'
labels:
  - web
  - ui
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On a phone the nav sidebar takes roughly 58% of the viewport and never yields it, so the content area is a ~40-character column: the dashboard empty state wraps 'No books yet' onto three lines (screenshot, iPhone at nigel.books). The sidebar is a fixed `--nc-sidebar-width` (232px) at every viewport size — neither wc-app-shell nor wc-nav-sidebar carries a width breakpoint, and nothing in nigel-app ever sets the collapsed state.

Half of this already exists and is unused: wc-nav-sidebar supports `collapsed` (a 56px icon rail with labels hidden and the label moved to a title tooltip), and wc-app-shell declares a reflected `sidebar-collapsed` property. No control toggles either one, and no media query sets them.

A 56px rail is still a poor trade on a 390px screen, so below the breakpoint the sidebar should leave the flow entirely and open over the content as a drawer, with a toggle in the header. Above the breakpoint the current layout stays, with the toggle collapsing to the rail that is already built. wc-send-dialog's `@media (max-width: 48rem)` is the existing breakpoint in the UI package.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Below the breakpoint the sidebar is off-canvas by default and the content area gets the full viewport width
- [ ] #2 A control in the shell header opens the sidebar as an overlay drawer over the content, with a backdrop
- [ ] #3 The drawer closes on nav selection, on Escape, and on backdrop click, and focus returns to the toggle
- [ ] #4 Above the breakpoint the sidebar is docked as it is today and the same control toggles the existing 56px collapsed rail
- [ ] #5 The toggle is keyboard reachable and exposes its expanded/collapsed state to assistive tech
- [ ] #6 The drawer respects prefers-reduced-motion
<!-- AC:END -->
