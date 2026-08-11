/**
 * Nigel's pastel rainbow, in the order `GRADIENT` declares it in
 * `src/effects.rs`. The TUI splash, goodbye, onboarding, and snake screens
 * interpolate along these stops; the web UI reuses them so the two front ends
 * are recognisably the same product.
 *
 * These are decorative values. Every solid interactive or semantic color in
 * `color.ts` is a darkened derivation that clears WCAG AA — a pastel on white
 * does not.
 */
export declare const NIGEL_PALETTE: readonly ["#ffb3ba", "#ffc8a2", "#ffe0a3", "#c9ffcb", "#bae1ff", "#c4b7ff", "#ffb3de"];
export declare const gradientCss: import("lit").CSSResult;
//# sourceMappingURL=gradient.d.ts.map