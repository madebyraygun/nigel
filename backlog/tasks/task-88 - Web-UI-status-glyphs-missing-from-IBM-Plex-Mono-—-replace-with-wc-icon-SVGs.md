---
id: TASK-88
title: 'Web UI: status glyphs missing from IBM Plex Mono — replace with wc-icon-* SVGs'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-11 21:23'
updated_date: '2026-08-14 04:56'
labels:
  - web
  - ui
  - theme
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
IBM Plex Mono has no glyph for the status/UI characters ✗ ⟳ ◑ ● ◆ ▲ ⊘ ◻ (verified against the complete upstream font, not a subsetting artifact). With the mono typeface as primary (TASK-76, PR #201), wc-invoice-status, wc-send-dialog and wc-reconciliation-history render these via per-glyph font fallback, which breaks visual consistency. Replace the character glyphs with wc-icon-* SVG icons through the existing WcIconBase. Surfaced during TASK-76 implementation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Every one of the eight characters (✗ ⟳ ◑ ● ◆ ▲ ⊘ ◻) is gone from the web sources; each is drawn by a wc-icon-* element built on WcIconBase
- [x] #2 wc-invoice-status, wc-send-dialog and wc-reconciliation-history render icons, not characters, and no other component draws one of the eight
- [x] #3 Each icon sits inline with the mono text beside it and inherits currentColor, so every status keeps the colour semantics it had
- [x] #4 The icons are decorative: the word beside each one is what assistive tech announces, and describePreviewA11y reports zero violations for every affected preview
- [x] #5 The previews of the affected components (and the icon gallery) show the new icons in every state they can take
- [x] #6 The docs that describe the gap — web/README.md, subset-fonts.mjs, CLAUDE.md — describe the icons that are there now
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Grep every one of the eight characters across web/ to find all consumers, not just the three named
2. Add the missing icons to the wc-icon set on WcIconBase — six status shapes plus a neutral dot; reuse check/close/refresh where the shape already exists
3. Render them from wc-invoice-status, wc-send-dialog and wc-reconciliation-history at 1em, currentColor, decorative
4. Update the three previews and the icon gallery; keep describePreviewA11y at zero violations
5. Add a source sweep so the characters cannot come back
6. Update web/README.md, subset-fonts.mjs and CLAUDE.md to describe the icons that are there now
7. npm ci / build / test / lint / typecheck
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Swept web/ for all eight characters plus the near neighbours (✓ ○ ◇ △ ▼ ■ □). The three named components are the whole list: wc-invoice-status (◻ ◆ ◑ ● ▲ ⊘ and a • fallback), wc-send-dialog (· ⟳ ✓ ✗) and wc-reconciliation-history (✓ ✗). Nothing in apps/app draws one; the only other hits are the docs that describe the gap and the preview harness's own ✓, which the font does have.
- Seven new icons on WcIconBase: wc-icon-status-{draft,sent,partial,paid,overdue,void} and wc-icon-dot. The send trace reuses wc-icon-check/close/refresh rather than adding a second copy of the same geometry, and wc-icon-dot serves both a step not started and a status the six words do not cover.
- Every status icon is drawn to one optical width — a 7-unit radius plus the base stroke — so a column of chips does not jitter; outline versus fill carries the distinction so colour stays a third cue.
- Sizing is --nc-icon-size: 1em set by the consuming component, so each mark tracks the font size of the text beside it; colour is WcIconBase's inherited currentColor, so the per-status rules that were already there keep working untouched.
- All decorative: no `label`, so WcIconBase renders role="presentation" aria-hidden="true". The word beside each one already names the state (the status word, the sr-only step state, Reconciled/Discrepancy).
- wc-send-dialog's step rows move from align-items: baseline to center — an inline-flex icon with no text content baselines on its bottom edge.
- Added packages/ui/src/__tests__/mono-glyph-coverage.test.ts: sweeps packages/{ui,theme}/src and apps/app/src for the eight characters and names the icon to use instead, with a guard-the-guard case and a check that every icon it names is registered. Test files are exempt — the tests for these components have to name the characters to say what is no longer rendered.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Replaced every character glyph the app's primary typeface cannot draw with a `wc-icon-*` SVG on the existing `WcIconBase`.

IBM Plex Mono has no glyph for `✗ ⟳ ◑ ● ◆ ▲ ⊘ ◻` — a property of the complete upstream release, not of the committed subset — so each of them came from whatever fallback face the browser found, putting two typefaces on a single line.

Changes:
- `@nigel/ui` gains seven icons: `wc-icon-status-{draft,sent,partial,paid,overdue,void}` and `wc-icon-dot`. Shape carries the distinction (outline versus fill, silhouette), all drawn to one optical width so a column of chips does not jitter.
- `wc-invoice-status` renders the status icon in place of the character, with `wc-icon-dot` for a status outside the six the data layer derives. `wc-send-dialog`'s step trace and `wc-reconciliation-history`'s result column reuse `wc-icon-check`/`wc-icon-close`/`wc-icon-refresh`/`wc-icon-dot` rather than adding second copies of the same geometry.
- Each mark is `--nc-icon-size: 1em` and inherits `currentColor`, so it tracks the size of the mono text beside it and every status keeps exactly the colour it had. All are decorative — the word beside each one (the status, the step's `sr-only` state, `Reconciled`/`Discrepancy`) is what announces.
- `wc-send-dialog`'s step rows move from `align-items: baseline` to `center`: an inline-flex icon with no text content baselines on its bottom edge.
- New guard, `packages/ui/src/__tests__/mono-glyph-coverage.test.ts`: sweeps `packages/{ui,theme}/src` and `apps/app/src` for the eight characters and names the icon that replaces each. A character typed back in renders plausibly on the author's machine and breaks no other test.
- Previews: the icon gallery gains an inline-with-text state, the status chip gains a running-text state, and the descriptions say icon rather than glyph. `web/README.md`, `subset-fonts.mjs` and `CLAUDE.md` now describe what draws the marks instead of describing a gap to be fixed.

Tests: `npm test` 1936 passed across 109 files (theme 188, ui 1088, app 760), no unhandled errors; `npm run build`, `npm run lint`, `npm run typecheck` all clean. `describePreviewA11y` reports zero violations for every state of the three components and the icon gallery.
<!-- SECTION:FINAL_SUMMARY:END -->
