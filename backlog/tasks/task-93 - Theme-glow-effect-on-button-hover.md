---
id: TASK-93
title: 'Theme: glow effect on button hover'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-12 17:51'
updated_date: '2026-08-14 04:57'
labels:
  - web
  - ui
  - theme
  - enhancement
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Buttons currently change little on hover. Add a subtle glow (soft box-shadow in the button's accent color, e.g. the brand gradient hues) on hover/focus-visible for primary and secondary buttons. Must respect prefers-reduced-motion for any transition, pass the contrast suites, and hold up in both light and dark modes.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Primary and secondary buttons show a visible glow on hover and focus-visible in both modes
- [x] #2 Transitions respect prefers-reduced-motion and all contrast/a11y suites pass
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add --nc-glow-brand / --nc-glow-neutral tokens to tokens/gradient.ts, derived from NIGEL_PALETTE_INK (light) and NIGEL_PALETTE (dark) so the glow is the brand ramp by construction
2. Interpolate the dark block into darkTokens in tokens/color.ts, beside the --nc-grad-brand-text override
3. Add hover/focus-visible box-shadow rules for brand/primary and neutral (secondary) buttons to controls.ts; no transition declaration — wa-button's own .button already transitions box-shadow over --wa-transition-fast, which wa-contract points at --nc-duration-fast (0ms under prefers-reduced-motion)
4. Tests: token presence and mode-specific ramp in nigel-theme.test.ts, rule/selector coverage in controls.test.ts
5. Verify: npm run build, npm test, npm run lint, npm run typecheck
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Tokens: `--nc-glow-brand` / `--nc-glow-neutral` in tokens/gradient.ts, mixed with color-mix from stops of the ramps themselves (light: NIGEL_PALETTE_INK violet #6951d6 + fuchsia #ba2c7c; dark: NIGEL_PALETTE lavender #c4b7ff + magenta #ffb3de). Two layers — a close 10px halo and a wider 20px one — at 32%/16% light, 36%/20% dark; the neutral token is the single close layer at 20%/26%.
- The dark values ride `darkTokens` in tokens/color.ts, beside the `--nc-grad-brand-text` override that established the ink-vs-pastel split.
- controls.ts adds two rules: brand/primary and neutral (excluding `appearance=plain` and disabled), each on `:hover` and `:focus-visible`, with focus-visible stated on the host *and* on the part because wa-button sets delegatesFocus.
- No transition is declared: wa-button already transitions box-shadow on its base part over `--wa-transition-fast`, which wa-contract.ts points at `--nc-duration-fast` and motion.ts zeroes under prefers-reduced-motion. An outer-tree transition would replace that list rather than extend it.
- The preview harness cannot force :hover/:focus-visible (no state forcing, and previews render in preview-app's shadow root, which does not adopt controlsCss — a bare wa-button there would show no part styling at all). Buttons stay previewable only through the wc-* components that host them; the glow itself needs a real pointer or keyboard in a browser.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Buttons now lift on hover and focus-visible with a soft glow in the brand ramp's own hues, in both colour modes.

Changes:
- `tokens/gradient.ts` gains `--nc-glow-brand` and `--nc-glow-neutral`, mixed from stops of the ramps already declared there — the ink violet/fuchsia in light mode, the pastel lavender/magenta in dark. Two box-shadow layers (a close halo, a wider fainter one) for the brand token, one for the neutral. Deriving from the arrays rather than restating hexes keeps the glow pinned to the palette.
- `tokens/color.ts` interpolates the dark block into `darkTokens`, beside the `--nc-grad-brand-text` override whose split it follows: a pastel glow is invisible on a near-white surface, the ink hues are a smudge on a dark one.
- `controls.ts` draws the glow for the brand/primary variants and for the neutral (secondary) ones, skipping disabled buttons and `appearance="plain"` row actions. `:focus-visible` is written on the host and on the part, since wa-button sets `delegatesFocus`.

Reduced motion: no transition is declared. wa-button's base part already transitions box-shadow over `--wa-transition-fast`, which `wa-contract.ts` points at `--nc-duration-fast` and `motion.ts` zeroes under `prefers-reduced-motion`; an outer-tree transition would replace that whole list, dropping the background and colour transitions with it.

Tests: `nigel-theme.test.ts` pins the tokens, the light/dark ramp split, the three declarations (one light, two dark blocks), the reduced-motion duration chain, and a 40% ceiling on every mix so the glow stays subtle; `controls.test.ts` pins the selectors, the hover/focus-visible coverage and the exclusions. Full run green: theme 203, ui 1058, app 760; lint and typecheck clean.

What a reviewer has to judge in a browser: how the glow actually reads. The preview harness cannot force :hover or :focus-visible, and previews render in preview-app's shadow root, which does not adopt controlsCss — so no preview can show it. `npm run preview` and hovering a button inside wc-send-dialog / wc-confirm / wc-invoice-form, in both modes, is the check.
<!-- SECTION:FINAL_SUMMARY:END -->
