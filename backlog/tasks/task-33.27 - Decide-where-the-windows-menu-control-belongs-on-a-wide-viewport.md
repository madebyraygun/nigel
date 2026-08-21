---
id: TASK-33.27
title: Decide where the window's menu control belongs on a wide viewport
status: To Do
assignee: []
created_date: '2026-08-21 00:12'
labels:
  - tauri
  - ui
dependencies: []
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On a wide desktop viewport the top 48px of the window is two boxes side by side, not one bar. wc-nav-sidebar's brand row is height var(--nc-header-height) and the sidebar is a flex sibling before .main, so the band reads left to right as [company name] [menu toggle] [screen title]. The toggle already leads wc-app-shell's header — it is the header's first child, before the screen title, and no company name is in the header at all (nigel-app passes the company name to the sidebar as app-name). TASK-33.15 verified that and left the structure alone, because the ordering inside the header is already right and the rest is a design decision.

The observation that started this is still true though: the operator reads the company name first and the hamburger second, and on a native window the control that collapses the sidebar sits at the far left of the window's top band.

Two candidate fixes, both larger than a token or ordering change.

A. Move the toggle into the sidebar's brand row for wide viewports, before the company name, and keep wc-app-shell's header toggle for the drawer below 48rem. Cost: two controls with one hidden per breakpoint, because below 48rem the sidebar slides off-canvas and a toggle inside it would go with it. Cheap in layout terms, and the collapsed 56px rail already centres an icon nicely.

B. Restructure wc-app-shell so a full-width header sits above a [sidebar | content] row, which is the true unified-toolbar arrangement and puts the toggle at the window's actual top-left. Cost: rewrites the drawer overlay (the sidebar is currently fixed inset 0 auto 0 0 and would now have to clear the header), the print rules that hide the header and sidebar, and the .main flex structure.

The choice is the operator's. Whichever way it goes, it intersects TASK-33.20's unified title bar on macOS — that task moves the traffic lights into the same 48px band, so whichever of the two lands first shapes what the other can do.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The operator has chosen between the brand-row toggle and the full-width header, and the choice is recorded with its reasoning
- [ ] #2 The chosen arrangement is implemented, with the drawer below 48rem still openable and closable
- [ ] #3 TASK-33.20's traffic-light placement still works in the same band, or the conflict is recorded
<!-- AC:END -->
