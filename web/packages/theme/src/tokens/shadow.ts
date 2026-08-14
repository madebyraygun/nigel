import { css, unsafeCSS } from 'lit';
import { NIGEL_PALETTE, NIGEL_PALETTE_INK } from './gradient.js';

const lavender = unsafeCSS(NIGEL_PALETTE[5]);
const magenta = unsafeCSS(NIGEL_PALETTE[6]);
const violetInk = unsafeCSS(NIGEL_PALETTE_INK[5]);
const fuchsiaInk = unsafeCSS(NIGEL_PALETTE_INK[6]);

/**
 * The hover and focus-visible halo: a close lavender glow with a wider, fainter
 * magenta one under it. `--nc-glow-brand` sits behind a button filled with the
 * gradient, `--nc-glow-neutral` behind one that carries no fill.
 *
 * Two ramps for `--nc-grad-brand-text`'s reason. A pastel halo on a near-white
 * surface is invisible, so light mode mixes the ink ramp; on a dark surface the
 * ink hues read as a smudge, so dark mode mixes the pastels. The alphas differ
 * with them — a glow has to lift off the surface it is drawn on.
 */
const glowLightTokens = css`
  --nc-glow-brand: 0 2px 10px color-mix(in srgb, ${violetInk} 32%, transparent),
    0 6px 20px color-mix(in srgb, ${fuchsiaInk} 16%, transparent);
  --nc-glow-neutral: 0 2px 10px color-mix(in srgb, ${violetInk} 20%, transparent);
`;

export const glowDarkTokens = css`
  --nc-glow-brand: 0 2px 10px color-mix(in srgb, ${lavender} 36%, transparent),
    0 6px 20px color-mix(in srgb, ${magenta} 20%, transparent);
  --nc-glow-neutral: 0 2px 10px color-mix(in srgb, ${lavender} 26%, transparent);
`;

/** Tuned for the pale surfaces: low alpha, tinted toward the lavender brand. */
export const shadowCss = css`
  :root {
    --wa-shadow-sm: 0 1px 2px rgb(43 43 51 / 6%);
    --wa-shadow-md: 0 4px 12px rgb(43 43 51 / 10%);
    --wa-shadow-lg: 0 12px 32px rgb(43 43 51 / 14%);

    ${glowLightTokens}

    /* A semantic button is already drawn in its own colour, and that colour
       carries a dark override, so one declaration reading the token serves
       both modes. The ramp above is not a token and needs two. */
    --nc-glow-danger: 0 2px 10px color-mix(in srgb, var(--wa-color-danger) 30%, transparent);
    --nc-glow-success: 0 2px 10px color-mix(in srgb, var(--wa-color-success) 30%, transparent);
    --nc-glow-warning: 0 2px 10px color-mix(in srgb, var(--wa-color-warning) 30%, transparent);
  }
`;
