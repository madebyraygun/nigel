import { css, unsafeCSS } from 'lit';

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
export const NIGEL_PALETTE = [
  '#ffb3ba', // soft pink
  '#ffc8a2', // peach
  '#ffe0a3', // pastel yellow
  '#c9ffcb', // mint
  '#bae1ff', // pastel cyan
  '#c4b7ff', // lavender
  '#ffb3de', // soft magenta
] as const;

/**
 * The same seven hues, taken to a lightness that survives a light surface.
 *
 * The pastels are the brand, and on a near-white header a pastel wordmark is
 * very nearly invisible. These keep each stop's hue and trade its lightness
 * for saturation, so the ramp still reads as the rainbow rather than as seven
 * arbitrary colours — every stop clears 4.5:1 on the sidebar, which
 * `contrast.test.ts` holds it to.
 *
 * `NIGEL_PALETTE` itself is untouched, which matters: `palette-parity.test.ts`
 * pins it to `GRADIENT` in `src/effects.rs` so the TUI and the browser cannot
 * drift, and this is a second, browser-only ramp rather than an edit to the
 * shared one. Dark mode keeps the pastels — see `--nc-grad-brand-text` in
 * `color.ts`.
 */
export const NIGEL_PALETTE_INK = [
  '#c22e3b', // pink   -> deep rose
  '#9b5524', // peach  -> burnt orange
  '#84621f', // yellow -> ochre
  '#1c781f', // mint   -> leaf green
  '#266ba1', // cyan   -> steel blue
  '#6951d6', // lavender -> violet
  '#ba2c7c', // magenta -> fuchsia
] as const;

// unsafeCSS is required to interpolate anything that is not itself a
// CSSResult. The value is a join of the frozen hex literals above — no input
// reaches it — so there is nothing here for a stylesheet injection to ride in on.
const ramp = unsafeCSS(NIGEL_PALETTE.join(', '));
const inkRamp = unsafeCSS(NIGEL_PALETTE_INK.join(', '));

export const gradientCss = css`
  :root {
    --nc-grad-brand: linear-gradient(90deg, ${ramp});
    --nc-grad-brand-soft: linear-gradient(
      90deg,
      rgb(255 179 186 / 18%),
      rgb(255 200 162 / 18%),
      rgb(255 224 163 / 18%),
      rgb(201 255 203 / 18%),
      rgb(186 225 255 / 18%),
      rgb(196 183 255 / 18%),
      rgb(255 179 222 / 18%)
    );
    --nc-grad-brand-hover: linear-gradient(90deg, ${ramp});

    /* Gradient text on a light surface — the wordmark. Overridden back to the
       pastel ramp in dark mode, where the pastels are the legible choice. */
    --nc-grad-brand-text: linear-gradient(90deg, ${inkRamp});

    /* Text drawn on the gradient itself. Deliberately not mode-dependent and
       deliberately not --wa-color-text: the ramp above is the same pastel in
       light and dark, so a foreground that flips with the mode is unreadable
       in one of them. Held against every stop by contrast.test.ts. */
    --nc-color-on-gradient: #2b2b33;
  }
`;
