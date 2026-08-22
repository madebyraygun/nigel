---
id: TASK-33.26
title: Sidebar vibrancy behind the navigation plane
status: To Do
assignee: []
created_date: '2026-08-20 23:49'
labels:
  - tauri
  - macos
dependencies:
  - TASK-33.20
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The deluxe tier, for after the unified title bar (TASK-33.20), the menu bar (TASK-33.22) and the system-face typography (TASK-33.15) land: a translucent sidebar that picks up the desktop tint the way Finder's and Mail's do. The window gains the sidebar vibrancy material behind a navigation plane whose background goes transparent in the desktop shell; the content area stays opaque.

The mechanics are known: window effects in Tauri core (or the window-vibrancy crate, which also carries the macOS 26 liquid-glass API), a transparent window, and macOSPrivateApi — acceptable for direct distribution per decision-3, though it forecloses the Mac App Store, which is worth recording when the flag is flipped. NSVisualEffectView honors Reduce Transparency on its own. The sidebar's transparency is shell-owned: a CSS custom property the shell sets and the sidebar consumes, so @nigel/ui reads a token and never the platform.

Hold it until the tiers above are in — vibrancy over a stock title bar and a mono chrome face would be lipstick.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The sidebar plane shows window vibrancy on macOS in light and dark; the content area stays opaque
- [ ] #2 Reduce Transparency yields today's opaque sidebar
- [ ] #3 Windows, Linux and browser presentation are unchanged, and no @nigel/ui component detects the platform
- [ ] #4 The macOSPrivateApi tradeoff (direct distribution only) is recorded alongside the change
<!-- AC:END -->
