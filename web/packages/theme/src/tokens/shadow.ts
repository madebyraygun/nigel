import { css } from 'lit';

/**
 * Tuned for the pale surfaces: low alpha, tinted toward the charcoal text.
 *
 * `--nc-shadow-floating` is what the inset sidebar and status panels cast,
 * and `--wa-shadow-lg` is the modal lift — the palette, a dialog, the save
 * bar. Both are redefined for dark mode, where a tinted low-alpha shadow is
 * invisible against the dark canvas and only a deep black reads.
 */
export const shadowCss = css`
  :root {
    --wa-shadow-sm: 0 1px 2px rgb(43 43 51 / 6%);
    --wa-shadow-md: 0 4px 12px rgb(43 43 51 / 10%);
    --wa-shadow-lg: 0 16px 40px rgb(43 43 51 / 18%), 0 2px 6px rgb(43 43 51 / 6%);
    --nc-shadow-floating: 0 1px 2px rgb(43 43 51 / 6%), 0 8px 24px rgb(43 43 51 / 6%);
  }
`;

/**
 * The dark-mode shadows, composed into `colorDarkCss` so they switch under
 * the same selectors as the colours.
 */
export const shadowDarkTokens = css`
  --wa-shadow-lg: 0 20px 50px rgb(0 0 0 / 55%), 0 0 0 1px rgb(255 255 255 / 4%);
  --nc-shadow-floating: 0 8px 24px rgb(0 0 0 / 35%);
`;
