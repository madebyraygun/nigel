---
id: TASK-99
title: 'Web UI: Snake easter egg'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-12 22:51'
updated_date: '2026-08-14 05:27'
labels:
  - web
  - ui
  - enhancement
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TUI dashboard hides a Snake game behind the s key (cli/snake.rs, with the shared pastel gradient and particle effects from effects.rs). The web app should carry the same easter egg so the two front ends share their one secret. Needs a discreet trigger (a key on the dashboard when no input has focus, or a konami-style sequence — deliberately undocumented in the UI), a wc-snake component rendered as an overlay, arrow-key controls, score display, and an exit back to whatever screen was underneath. Visuals should read as the same game as the TUI: the brand gradient snake and the effects.rs-derived palette the theme already carries. It must not interfere with normal typing — the trigger only fires when focus is not in a form control.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A hidden trigger on the web dashboard opens Snake without being discoverable from the visible UI
- [x] #2 Arrow keys steer, the game scores, and Esc exits back to the underlying screen with prior focus restored
- [x] #3 The trigger never fires while focus is inside a form control or dialog
- [x] #4 The snake renders in the brand gradient palette shared with the TUI game
- [x] #5 Component-first: wc-snake ships with a preview and describePreviewA11y passes (reduced-motion respected)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. @nigel/theme: gradientColor(t) mirroring effects.rs gradient_color, plus the three mode-independent arcade tokens (board, ink, food) and contrast tests for them.
2. @nigel/ui: snake-engine.ts as a pure port of cli/snake.rs (board 40x20, wall/self/board-full game over, $1.00-$9.99 food, 150ms base tick -2ms per segment floored at 50ms), with a parity test that reads the Rust constants.
3. wc-snake.ts: fixed overlay, arrow keys, score, R restart, Esc exit event, gradient body, particles off under reduced motion; wc-snake.preview.ts states + describePreviewA11y.
4. @nigel/app: snake-trigger.ts pure guards (composed path + deep active element; form controls and dialogs block), wired into nigel-app as a window keydown listener on the dashboard route only, with focus captured and restored.
5. Docs: CLAUDE.md and web/README.md; verify with npm run build/test/lint/typecheck.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Ported the rules rather than reimplementing them: `snake-engine.ts` is `cli/snake.rs` function for function, and `snake-parity.test.ts` reads the Rust source for the board, the tick curve, the food cent range and the full-board win, so the two cannot drift silently. The Rust `mod tests` cases are ported case for case as well.
- The trigger is its own pure module. `s` bound at the window is a bug in every context but one, so the refusals live in `snake-trigger.ts` with 30 tests against hand-built events, checking the composed path *and* the focus chain — the path is where the event came from, the chain catches a dialog holding the screen with nothing focusable in it. `deepActiveElement` walks shadow roots, since `document.activeElement` answers `nigel-app` for a caret in the register editor.
- Theme gained `gradientColor` (the `effects::gradient_color` interpolation, truncating channels as `as u8` does) and three mode-independent `--nc-color-arcade-*` tokens. The board keeps a dark ground in light mode because the pastels are invisible on a light surface; the palette stops, the ink and the food are all held to a threshold on it in `contrast.test.ts`.
- Reduced motion stops the specks and the gradient cycling, in the property and again in a media query, and never the snake. The four preview states are paused so axe runs over a board that is not moving under it.
- Verified from web/: npm ci, npm run build, npm test (10+63+40 files, 208+1109+799 tests, no Unhandled Errors block), npm run lint, npm run typecheck — all exit 0.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The web dashboard now hides Snake behind the same `s` the TUI hides it behind, playing the same game rather than a lookalike.

**The game.** `@nigel/ui` gains `snake-engine.ts`, a pure port of `src/cli/snake.rs` — 40×20, three segments heading right, food worth $1.00–$9.99, a 150 ms tick shedding 2 ms per segment down to a 50 ms floor, and the three endings (wall, itself, a full board, which is the win). `snake-parity.test.ts` reads the Rust source and fails if any of those numbers drift, the way `palette-parity.test.ts` pins the colours; the Rust suite's own cases are ported case for case. `wc-snake` owns the clock and the pixels: absolutely positioned cells rather than a canvas, so a segment's colour is asserted rather than eyeballed, and only the snake, the food and twenty specks are ever in the DOM.

**The trigger.** `apps/app/src/snake-trigger.ts` is a pure module because `s` is a letter before it is a shortcut. It fires for a bare unmodified `s` that is not a repeat, not mid-composition and not already handled, and never while a form control or a dialog is in the event's composed path or the focus chain — two sources, since the path is where the event came from and the chain catches a dialog holding the screen with nothing focusable inside it. `deepActiveElement` follows shadow roots down, because `document.activeElement` answers `nigel-app` for a caret in the register's inline editor. `nigel-app` binds it at the window (the game outlives the screen under it), only on the dashboard route and only once the app is past booting and unlocked, marks the shell `inert`, and on `nc-snake-exit` takes the overlay down and returns focus to the element it captured.

**The look.** `@nigel/theme` gains `gradientColor`, the browser's `effects::gradient_color`, and the three mode-independent `--nc-color-arcade-*` tokens: the board keeps a dark ground in light mode because the pastels the snake is drawn in are invisible on a light one, and `contrast.test.ts` holds every palette stop, the ink and the food to a threshold on it. Reduced motion stops the drifting specks and the gradient cycling along the snake — in the property and again in a media query — and does not stop the snake, which would not leave a game.

**Tests.** `wc-snake.preview.ts` declares four states (new game, in play, game over, reduced motion), all paused so axe runs over a still board; `describePreviewA11y` passes on all four. 60 new tests across the three packages.

Verified from `web/`: `npm ci`, `npm run build`, `npm test` (208 + 1109 + 799 passing, no Unhandled Errors), `npm run lint`, `npm run typecheck` — all clean. Docs: CLAUDE.md architecture and project structure, and a "The easter egg" section in `web/README.md`.
<!-- SECTION:FINAL_SUMMARY:END -->
