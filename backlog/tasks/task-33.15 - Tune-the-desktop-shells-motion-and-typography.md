---
id: TASK-33.15
title: Tune the desktop shell's motion and typography
status: To Do
assignee: []
created_date: '2026-08-18 01:56'
updated_date: '2026-08-20 23:50'
labels:
  - tauri
  - ui
dependencies: []
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The shell reads as more native since the web tells were removed, and what remains is typographic and motion work rather than convention-following.

Three specific pieces the operator named after using the desktop app:

The sidebar toggle snaps. It should animate, respecting prefers-reduced-motion, which the app already honours in seventeen places.

The hamburger sits after the company name. On a desktop window it belongs at the far left, before the name, where a title bar's controls live.

The typeface balance leans mono. IBM Plex Mono is currently the default body face — --wa-font-family-sans falls back to a mono stack — so chrome, labels and prose all render in it. A native app uses the system face for its chrome. Plex Mono should stay where it earns its place: figures, where digits must align, and the brand's own character. This is a change to the theme tokens rather than to components, since a component reads the token and never a hardcoded stack.

The third is the substantial one and the biggest remaining tell after the conventions work: system-ui for chrome is most of what makes an app look like it belongs on the machine.

Two notes from a later read of the code. wc-app-shell already renders the nav toggle before the header title (wc-app-shell.ts:312-320), and the title there is the screen title with the company name living in the sidebar — so AC #2 may already hold; verify in the running shell rather than re-doing it. And while in the tokens: native macOS chrome text is 13px (NSFont.systemFontSize), the base here is 14px today — worth trying 13px for chrome-level text alongside the face change.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The sidebar toggle animates, and does not animate under prefers-reduced-motion
- [ ] #2 The menu control sits at the far left of the header, before the company name
- [ ] #3 Chrome, labels and prose render in the system face; figures and the brand keep IBM Plex Mono
- [ ] #4 The change is made in @nigel/theme tokens; no component names a font stack directly
- [ ] #5 Money columns still align digit for digit, and the mono-glyph coverage test still passes
<!-- AC:END -->
