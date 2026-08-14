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

/**
 * The plain ramp, drawn once across whatever it fills.
 *
 * This is what the wordmark wants and what `--nc-grad-brand` is deliberately
 * not: a periodic image sized for the drift renders at six-thirteenths scale
 * and tiled in anything that does not also set `--nc-grad-brand-size`, and
 * gradient text sets neither.
 */
export const brandRamp = css`linear-gradient(90deg, ${ramp})`;

/**
 * The brand ramp as a *periodic* image, so the button's hover drift can loop.
 *
 * Counted in the ramp's own step — seven stops across the element is six gaps,
 * so a step is a sixth of the element's width. One period is the ramp plus a
 * seventh step wrapping magenta back to pink, and the image is a period plus
 * the ramp: thirteen steps. That is what `--nc-grad-brand-size` says, and it
 * is the whole trick — `background-position: 100%` offsets by image minus
 * element, which is thirteen steps minus six, exactly one period, so the loop
 * has no seam and no jump wherever the pointer leaves.
 *
 * At rest the element shows steps 0-6, which is pink at the left edge and
 * magenta at the right: the same seven stops in the same places the plain
 * `linear-gradient` put them, so a button that never moves looks unchanged.
 * The wrap segment lives off the right-hand edge until the drift brings it in.
 *
 * Anything reading `--nc-grad-brand` therefore has to set
 * `--nc-grad-brand-size` as its `background-size`, or it gets the ramp at
 * six-thirteenths scale, tiled. The two are declared together below for that
 * reason.
 */
const GAPS = NIGEL_PALETTE.length - 1;
const PERIOD_STEPS = NIGEL_PALETTE.length;
const IMAGE_STEPS = GAPS + PERIOD_STEPS;
const at = (step: number): string => `${((step / IMAGE_STEPS) * 100).toFixed(4)}%`;
const cycleStops = [
  ...NIGEL_PALETTE.map((c, i) => `${c} ${at(i)}`),
  `${NIGEL_PALETTE[0]} ${at(PERIOD_STEPS)}`,
].join(', ');
const cycle = unsafeCSS(`repeating-linear-gradient(90deg, ${cycleStops})`);
const cycleSize = unsafeCSS(`${((IMAGE_STEPS / GAPS) * 100).toFixed(4)}% 100%`);

/**
 * The drift itself: one period per iteration, which `--nc-grad-brand-size`
 * makes exactly one `background-position: 100%`.
 *
 * It is defined here and composed into both `nigelTheme` and `controlsCss`.
 * `@keyframes` are name-defining and scoped to a tree, and the element that
 * animates is `wa-button`'s `base` part — inside its shadow root, which is
 * neither the tree the rule is written in nor the document. Browsers do not
 * agree on how far that lookup walks, so the name is put within reach both
 * ways rather than relying on one of them.
 */
export const brandCycleKeyframes = css`
  @keyframes nc-brand-cycle {
    from {
      background-position: 0% 50%;
    }
    to {
      background-position: 100% 50%;
    }
  }
`;

export const gradientCss = css`
  :root {
    --nc-grad-brand: ${cycle};
    --nc-grad-brand-size: ${cycleSize};
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
    --nc-grad-brand-hover: ${cycle};

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
