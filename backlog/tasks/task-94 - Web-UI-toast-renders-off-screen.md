---
id: TASK-94
title: 'Web UI: toast renders off-screen'
status: Done
assignee:
  - '@claude'
created_date: '2026-08-12 17:51'
updated_date: '2026-08-14 18:02'
labels:
  - web
  - ui
  - bug
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Toasts render partially outside the viewport — screenshot shows a success toast ('…recorded 0.') clipped at the top-left corner over the sidebar brand, with most of the message cut off. Toasts should render inside the viewport in a consistent corner with sane stacking, and never clip their message.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Toasts render fully inside the viewport in a consistent position on every screen
- [x] #2 Long toast messages wrap rather than clip
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reproduce the placement from the stylesheet: .region set left: 50% then inset-inline: auto later in the same rule, unanchoring it
2. Re-anchor the region to a corner with both inline insets, no translate
3. Stack up to three toasts with per-toast timers; wrap long messages against the region width
4. Preview states for single, stacked and long-message; axe over every state
5. Tests assert resolved geometry (preview/css-geometry.ts) rather than declarations
6. npm ci / build / test / lint / typecheck from web/
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Root cause: `.region` set `left: 50%` and then `inset-inline: auto` later in the same rule (a popover reset). The shorthand won on cascade order, leaving both inline insets auto, so the fixed region fell back to its static position — the flex container's top-left corner in `wc-app-shell` — and `translateX(-50%)` pulled it half off-screen over the sidebar.
- Fix: bottom-right corner, both inline insets set, no translate. Toast max-width `min(360px, 100%)` where 100% is the region (viewport minus gutters), so long messages wrap.
- Stacking: up to 3 toasts, newest at the bottom, per-toast timers, oldest dropped past the cap.
- Two bugs found while writing it: a backtick inside the css`` template truncated `static styles` to NaN, and a private `remove(id)` shadowed `HTMLElement.remove()`.
- Tests assert geometry via `preview/css-geometry.ts` (stylesheet -> viewport pixels), not declarations. Also verified in headless Chromium inside the real `wc-app-shell` at 1280x800 and 390x700.

Review round (9 findings) — all addressed on feat/toast-position:
- A duration:0 toast with no action now carries a close button; `dismiss(id?)` closes one or all and `show()` answers the id.
- Top-layer promotion is keyed to arrival alone: no re-promotion on expiry (survivors stay where they are), and the doc comment states the trade.
- Live-region semantics split: region stays role=status/polite with no aria-atomic; a danger toast carries its own role=alert.
- `fixedBox` clamps a both-inset box to max-width and anchors it at the start inset, matching the over-constrained rule.
- Rule scanning behind `resolvedDeclarations`/`customProperties` extracted to one generator; the placement test reads its viewport from the shared table.
- Preview gains a `never-expires` state; other states seed a long duration rather than zero so they are not all close-buttoned.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Toasts render fully inside the viewport, in a consistent bottom-right corner, and long messages wrap.

Root cause: `wc-toast`'s `.region` declared `left: 50%` and then, further down the same rule, `inset-inline: auto` as part of a UA popover reset. The shorthand won on cascade order, so the fixed region had no resolved inline anchor and fell back to its static position — for a fixed box inside `wc-app-shell`'s flex container, the container's top-left corner — where `transform: translateX(-50%)` pulled it half off-screen over the sidebar brand.

Changes:
- The region pins to the bottom-right corner with both inline insets set and no translate, so it spans the viewport minus its gutters. The other two corners are occupied (sidebar left, header top).
- A toast's `max-width: min(360px, 100%)` is now viewport-relative, so a long message wraps inside the chip; `.message` carries `overflow-wrap: anywhere`.
- Up to `MAX_VISIBLE_TOASTS` (3) share the column, newest at the bottom, each with its own timer; the oldest is dropped past the cap. `.initial` seeds one toast or a whole stack.
- New `preview/css-geometry.ts`: resolves a stylesheet's cascade, shorthand expansion, `var()` fallbacks and `calc()`/`min()`/`max()` into viewport pixels, so placement is asserted as boxes rather than as declarations (jsdom has no layout engine).
- Preview gains a `stacked` state and a longer `long-message`; `describePreviewA11y` covers all six with zero violations.

Tests: web `npm test` 2051 passed (188 theme / 1103 ui / 760 app), no unhandled errors; `npm run build`, `npm run lint`, `npm run typecheck` clean. Additionally verified in headless Chromium inside the real `wc-app-shell`: at 1280x800 the three-toast stack measures left 904-1264, top 489-705 against a 1280x713 viewport, and at 390x700 the long message clamps to 358px and wraps.
<!-- SECTION:FINAL_SUMMARY:END -->
