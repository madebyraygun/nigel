---
id: TASK-108
title: 'Web UI: collapsible sidebar so the nav works on a phone'
status: Done
assignee: []
created_date: '2026-08-15 18:05'
updated_date: '2026-08-15 19:33'
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
- [x] #1 Below the breakpoint the sidebar is off-canvas by default and the content area gets the full viewport width
- [x] #2 A control in the shell header opens the sidebar as an overlay drawer over the content, with a backdrop
- [x] #3 The drawer closes on nav selection, on Escape, and on backdrop click, and focus returns to the toggle
- [x] #4 Above the breakpoint the sidebar is docked as it is today and the same control toggles the existing 56px collapsed rail
- [x] #5 The toggle is keyboard reachable and exposes its expanded/collapsed state to assistive tech
- [x] #6 The drawer respects prefers-reduced-motion
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- The rail was already built and unreachable: wc-nav-sidebar had collapsed, wc-app-shell had a reflected sidebar-collapsed, and nothing set either. The work was a toggle, a breakpoint, and a drawer.
- One boolean, two appearances. nigel-app owns it, the shell asks by nc-sidebar-toggle, and both the shell and the sidebar are bound to the answer — a shell that flipped its own copy would be a second source of truth.
- Above 48rem collapsed is the 56px rail. Below it the shell lifts the slotted sidebar out of the flow (position: fixed, translateX(-100%) while collapsed, backdrop while open) and the sidebar cancels its own rail styling at the same width, so the drawer shows full labels.
- narrowViewport()/NARROW_QUERY name the breakpoint once. The query is injectable because jsdom answers matchMedia false to everything — color-mode.ts already does this.
- Escape, backdrop and nc-navigate close the drawer; each is a no-op on a docked sidebar, which covers nothing. Focus returns to the toggle only on the narrow path, since collapsing to the rail is not a dismissal.
- Added wc-icon-menu; the icon set had no hamburger.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The sidebar now gets out of the way on a phone, and can be folded to the icon rail on a desktop.

Half of this already existed and was unreachable: wc-nav-sidebar supported a 56px collapsed rail and wc-app-shell declared a reflected sidebar-collapsed property, but nothing set either and no breakpoint existed. What was missing was a control, a viewport, and somewhere for the sidebar to go.

Changes:
- wc-app-shell renders a labelled toggle in the header (aria-expanded carries the state) and asks for a change with nc-sidebar-toggle. It owns no copy of the state: the sidebar is nigel-app either slotted child, so nigel-app holds the boolean and passes it to both.
- Below 48rem the shell takes the slotted sidebar out of the flow — fixed, translateX(-100%) while collapsed, a backdrop over the content while open — and wc-nav-sidebar stands its rail styling down at the same width, so what slides in is the full nav rather than 56px of icons.
- Escape, the backdrop, and choosing a screen all close the drawer, and each is a no-op on a wide viewport where the sidebar covers nothing. Focus returns to the toggle when the drawer closes, and only there. The slide is dropped under prefers-reduced-motion.
- The breakpoint is named once (NARROW_QUERY); narrowViewport() is how nigel-app seeds the state. The query is injectable, because jsdom answers matchMedia false to everything.
- New wc-icon-menu glyph; the set had no hamburger.

Tests: 21 new specs across the shell, sidebar and root container — the event contract in both directions, the three ways the drawer closes, the two it must not, focus return and focus restraint, and the CSS read off the adopted stylesheet the way the register fill rules are. Four new preview states carry the drawer through describePreviewA11y.

Verification: npm run build, npm test (270 + 1230 + 788 passing), npm run lint, npm run typecheck, cargo build --release. Deployed to the tailnet instance at https://nigel.books with demo data and checked on the phone that reported it.
<!-- SECTION:FINAL_SUMMARY:END -->
