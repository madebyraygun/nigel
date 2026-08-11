---
id: TASK-75
title: 'Web UI: dark and light mode switcher'
status: In Progress
assignee:
  - '@stream-4'
created_date: '2026-08-09 00:46'
updated_date: '2026-08-11 20:59'
labels:
  - enhancement
  - web
  - ui
  - theme
dependencies: []
documentation:
  - docs/superpowers/specs/2026-08-11-task-75-mode-switcher-design.md
  - docs/superpowers/plans/2026-08-11-task-75-mode-switcher.md
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The theme already defines a full light palette and a dark one — @nigel/theme tokens/color.ts carries both, and every solid colour in each is held to WCAG AA by a contrast test. What is missing is any way to choose between them: the mode follows the OS and nothing else.

Wants an explicit three-state control — light, dark, and follow the system — rather than a two-way toggle, so that following the OS stays reachable once a choice has been made. The choice needs to outlive a reload.

Two things worth settling in the design: where the preference lives (settings.json is shared with the CLI and the TUI, which have no use for it, so localStorage is likely the right home), and that print styling must keep winning — theme/src/print.ts is composed last for exactly that reason and a mode override must not defeat it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A control offers light, dark, and follow the system
- [x] #2 The choice persists across a reload
- [x] #3 Following the system tracks prefers-color-scheme changes live, without a reload
- [x] #4 Both palettes keep passing the existing contrast test
- [x] #5 Print output is unaffected by the selected mode
- [x] #6 The control is reachable by keyboard and passes axe in both modes
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Placement follows the orchestrator's ruling rather than spec §2.7: the switcher
is on the settings screen, not the shell header. `wc-app-shell`'s
`header-actions` slot stays unused.

AC #5 turned out to be a live bug, not a precaution, and the diagnosis is
arithmetic rather than observation (no browser here): `printCss` redefined the
tokens under a bare `:root` (0,1,0) while the dark palette is selected by
`:root:not(.light-mode)` and `:root.dark-mode`, both (0,2,0). Specificity is
resolved before source order, so composing print last never did anything, and a
dark-OS machine printed dark pages. `:root:root` ties the specificity and the
composition order breaks the tie. `print.test.ts` asserted the ordering and
called it "last wins in a flat cascade" — true only if the specificities
matched, which they did not.

AC #3 needs no code: `system` writes no class, so the CSS media query does the
tracking and the app follows an OS change with no listener and no reload. The
one `matchMedia` listener is for the "currently dark" hint only, and it is
injectable because jsdom's `matchMedia` always answers `matches: false`.

AC #4 holds by construction — no palette value moved, and the two additions
(`color-scheme` pins) carry no hex, so `contrast.test.ts`'s nth-`#rrggbb`
indexing is untouched. It passes unchanged.

Notable while working:

- Inserting an Appearance panel broke three data-directory tests that looked
  panels up by position. Those now find their panel by heading.
- The inline head script originally built the class as `m + '-mode'`, which
  passed by eye and defeated the whole point of the drift test — the literal
  strings were not in the file to match. Spelled out now, and the guard is
  confirmed to fail on a deliberately altered key.

Not done: the plan's Step 1 (reproduce the dark print in a browser) and Task 7's
manual pass — both need a browser and a dark OS. AC #5 in particular is
argued from specificity and asserted in the sheet, but the printer itself has
not been watched. That, the keyboard walk, and looking at dark mode across a
few screens are the review.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The theme has carried both palettes and the full three-state class contract
since it was written — no class means follow the system, `.light-mode` opts
out, `.dark-mode` forces dark — and nothing had ever set either class. This
adds the writer, the control, and one repair.

`@nigel/theme/color-mode.ts` is the package's first behaviour module:
`readMode`/`writeMode`/`applyMode`/`resolveMode`/`initColorMode` over
`localStorage`'s `nigel.color-mode`. It imports nothing, not even Lit, and it
lives beside the class names and the media query it writes against rather than
in `@nigel/ui`, because splitting a writer from its contract is how the two
drift. Every storage access is wrapped: `localStorage` throws on access when
storage is disabled, and a colour preference must not stop the app booting.

Not `settings.json`. Every `/api/settings/*` route is behind the locked guard,
so a server-stored preference could not be honoured on the unlock screen an
encrypted database shows first — a theme that snaps into place only after the
password is typed is worse than none. It is also per-browser by nature.

`system` writes no class, which is what makes AC #3 free: the browser
re-evaluates `prefers-color-scheme` itself, so the app follows an OS change
live with no listener and no reload. The single `matchMedia` listener exists
only to keep the "currently dark" hint honest; if it failed the colours would
still be right.

`wc-mode-switcher` is a `wa-radio-group` — three states need a third option,
and a toggle has nowhere to put "go back to following the OS". Fully
controlled, so the settings screen owns persistence and the preview harness
cannot change the real app. Five preview states, axe clean.

AC #5 was a live bug. The print block redefined the tokens under a bare
`:root` (0,1,0) while both dark selectors are (0,2,0), and specificity is
settled before source order — so a dark OS printed dark pages, and the
README's "black on white in both modes" checklist item had been failing.
`:root:root` ties the specificity; being composed last breaks the tie.

Also: `color-scheme` is pinned under an explicit choice, so scrollbars and the
reconcile screen's native month picker follow the app rather than the OS; and
a blocking inline script in `index.html` applies the stored mode before first
paint, with a test that fails if its duplicated key or class names drift.
<!-- SECTION:FINAL_SUMMARY:END -->
