---
id: TASK-93
title: 'Theme: animated gradient and edge on button hover'
status: Done
assignee:
  - '@claude'
created_date: '2026-08-12 17:51'
updated_date: '2026-08-14 19:35'
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
Buttons currently change little on hover. The brand button should animate its own gradient on hover/focus-visible — the same seven palette colours, scrolled rather than recoloured — and every button should fade a 1px solid edge in its variant colour in and out. Must respect prefers-reduced-motion, pass the contrast suites, and hold up in both light and dark modes.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Transitions respect prefers-reduced-motion and all contrast/a11y suites pass
- [x] #2 The brand button's gradient drifts on hover and focus-visible in both modes — the same palette, scrolled, looping with no seam
- [x] #3 Every button fades a 1px edge in its own variant colour in and out; plain, disabled and loading buttons are excluded
- [x] #4 A button at rest looks exactly as it did before the change
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

Review round on PR #9 (7 findings):
- Semantic variants glow in their own hue. wc-confirm renders its destructive action as variant="danger", so a glow limited to brand/neutral lit the Cancel button and left the primary dark — an inverted affordance. `--nc-glow-danger/-success/-warning` mix from `--wa-color-danger` and friends, which carry dark overrides already, so one declaration serves both modes.
- `:not([loading])` joins the exclusions: wa-button's handleClick calls preventDefault + stopImmediatePropagation while loading, so a busy button was drawing the strongest click invitation in the theme and ignoring the click.
- The dead `variant='primary'` selectors are gone from the whole sheet — Web Awesome has no such variant and nothing in packages/ or apps/ ever set it.
- The part-level `::part(base):focus-visible` duplicate is gone; delegatesFocus makes the host match, and the test now asserts the behaviour rather than the duplication.
- The anti-regression transition comment and the palette-derivation comment are gone; the rationale lives in the guard test and the CLAUDE.md bullet.
- One shared rule now applies the glow, with the variant hue named one line each, so the plain/disabled/loading exclusions cannot drift apart between variants.
- The glow family moved to tokens/shadow.ts, which is where box-shadow tokens live; gradient.ts is back to being only the ramps.

Theme gap found while testing PR #6, fixed here (same variant surface):
- wa-button[variant="danger"] rendered as the base button. WA colours a variant in two hops — variants.styles points --wa-color-fill-loud at --wa-color-danger-fill-loud, the component reads the generic one — and the theme defined only the neutral and brand families, so the declaration was discarded and every delete dialog's primary (wc-confirm) plus wc-password-form's remove submit fell back to the neutral grey.
- wa-contract.ts now defines the danger/success/warning families, mixed from each variant's own colour (10%/16% washes, solid loud fill, --wa-color-on-brand as the label). Brand decided deliberately: its loud fill stays the solid colour for outlined/plain, and the gradient part rule covers the default appearance.
- The guard missed it because it walked <name>.styles.js and counted only bare var(--x): the families appear there as var(--x, fallback), and variants.styles hangs off the component class. A second check walks the component module and demands the leaf families, with the intermediate names still deliberately undemanded.
- __tests__/token-resolution.ts resolves var() chains and color-mix so the suites assert what a button paints rather than what the sheet says; contrast.test.ts holds every on/fill pairing to AA and every loud border to 3:1. That caught --wa-color-neutral-border-loud at 1.23:1 (the outlined button's only edge) — it now names --wa-color-muted.
- button-variants.preview.ts shows the four variants across accent/outlined/filled/plain, with axe coverage from its own test. Brand is left out: a preview host does not adopt controlsCss.

Redesign after review of PR #9 (the glow was not what was wanted):
- The glow is gone entirely — --nc-glow-* tokens deleted from tokens/shadow.ts and the dark block in tokens/color.ts. Nothing reads them now.
- Brand hover/focus is the ramp scrolling: --nc-grad-brand is a repeating-linear-gradient and --nc-grad-brand-size (216.6667%) is the size that makes one background-position: 100% shift exactly one period, so nc-brand-cycle (2.4s linear infinite) loops with no seam and no jump wherever the pointer leaves. Same seven colours — nothing hue-rotated.
- The size is derived, not tuned: counted in the ramp own step (seven stops across the element is six gaps), one period is the ramp plus a seventh step wrapping magenta back to pink, and the image is thirteen steps. That puts the ramp exactly across the element at rest, so a button that never moves is pixel-identical to before. nigel-theme.test.ts asserts the derivation.
- Anything reading --nc-grad-brand without that size gets the ramp at six-thirteenths scale, tiled. The dark-mode wordmark used to name it and now names the new plain brandRamp export instead.
- The keyframes live in tokens/gradient.ts and are composed into both nigelTheme and controlsCss: keyframe names are tree-scoped and the animated element is inside wa-buttons shadow root, which is neither the tree the rule is written in nor the document.
- Every button fades a 1px edge in its own colour (--nc-hover-border, one line per variant, applied by a single rule with the same plain/disabled/loading exclusions the glow had). Neutral names --wa-color-text: an outlined button already sits on its own border tokens, so hovering to them is no change at all.
- The 500ms fade forced the transition list to be restated. WA transitions six properties on the base part over --wa-transition-fast and an outer-tree transition replaces that list rather than extending it — so all six are named again with border-color on --nc-duration-slow. controls.test.ts pins which properties this is answerable for; anything dropped from it stops transitioning on every button, silently.
- prefers-reduced-motion stops the drift in controlsCss and zeroes --nc-duration-slow in motion.ts. The edge still draws, instantly, so hover and focus keep an indication when the motion goes.
- button-hover.preview.ts is the harness for it, and it injects controlsCss into the state rather than copying the rules, so what is hovered is what ships. That is the gap button-variants.preview.ts documents: a preview host adopts no controlsCss.
- npm test (2102 across the three packages), lint and typecheck all pass.

- The edge is --wa-border-width-m (2px), not 1px, and drawn in two halves so hovering shifts nothing: the --wa-border-width-s border WA already reserved space for, plus the remainder as an inset box-shadow. An inset shadow is clipped to the padding edge, so it lands flush inside the border and the two read as one solid edge; box-shadow never participates in layout, and the rule sets no border-width at all (controls.test.ts fails if it ever does).
- The remainder is calc(--wa-border-width-m - --wa-form-control-border-width) rather than a literal 1px, so a change to either token stays an -m edge.
- box-shadow moved to --nc-duration-slow in the restated transition list beside border-color: the two halves of one edge have to fade together, or the second pixel arrives 380ms after the first.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Buttons now answer a hover by moving their own gradient and drawing their own edge, in both colour modes.

The brand button scrolls the palette rather than lighting up behind it: --nc-grad-brand became a repeating-linear-gradient paired with --nc-grad-brand-size, the size at which one background-position: 100% is exactly one period, so nc-brand-cycle (2.4s linear infinite) loops seamlessly and a pointer leaving mid-cycle causes no jump. The size is derived from the ramp own step count, which also makes a button at rest pixel-identical to before.

Every button — brand, neutral, danger, success, warning — fades a 1px edge in its own colour in and out over 500ms, applied by one rule carrying the same plain/disabled/loading exclusions. Giving one property a longer duration meant restating wa-buttons whole base-part transition, since an outer-tree transition replaces that list instead of extending it; a guard test names the properties this is now answerable for.

The outer glow is removed: --nc-glow-* is deleted from shadow.ts and colors dark block, and no rule reads it.

User impact: a clearer, quieter hover affordance that reads the same on a filled brand button and an outlined Cancel beside it. Under prefers-reduced-motion the drift stops and the edge draws instantly, so the affordance survives without the motion.

Tests: npm test (269 theme, 1073 ui, 760 app), npm run lint, npm run typecheck — all green. The button-hover preview injects the shipped controlsCss rather than a copy, so hovering the harness exercises what a screen ships.

Follow-up: live browser verification of the shipped rules is still owed — the mechanism was confirmed running in Chrome during exploration, but the devtools connection dropped before the final sheet could be re-checked.
<!-- SECTION:FINAL_SUMMARY:END -->
