import { css, unsafeCSS } from 'lit';

/**
 * The standard switch's knob and the check mark it shows when on.
 *
 * The knob is white in both modes — the design's 16px white knob — so the
 * check drawn on it does not flip with the mode either: one brand ink, held
 * to 3:1 against the knob by `contrast.test.ts`. The mode-dependent half of
 * the switch, its off edge, is `--nc-color-control-border` in `color.ts`.
 */
export const SWITCH_KNOB = '#ffffff';
export const SWITCH_CHECK = '#5a3fd6';

/**
 * The check from the icon set (`wc-icon-check` in `@nigel/ui`, which a parity
 * test there holds this to): the same 24-unit path with round caps and
 * joins, stroked at 3 rather than 2 because it renders at 10px.
 */
export const SWITCH_CHECK_PATH = 'M4 12l5 5L20 6';

const checkSvg =
  `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' ` +
  `stroke='${SWITCH_CHECK}' stroke-width='3' stroke-linecap='round' ` +
  `stroke-linejoin='round'><path d='${SWITCH_CHECK_PATH}'/></svg>`;

// unsafeCSS is required to interpolate anything that is not itself a
// CSSResult. Every piece is a constant in this module — no input reaches it.
const checkImage = unsafeCSS(`url("data:image/svg+xml,${encodeURIComponent(checkSvg)}")`);

export const switchCss = css`
  :root {
    --nc-switch-width: 38px;
    --nc-switch-height: 22px;
    --nc-switch-knob-size: 16px;
    --nc-color-switch-knob: ${unsafeCSS(SWITCH_KNOB)};
    --nc-color-switch-check: ${unsafeCSS(SWITCH_CHECK)};
    --nc-switch-check-image: ${checkImage};
  }
`;
