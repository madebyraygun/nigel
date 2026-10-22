import { css } from 'lit';

/**
 * - `xs` 4px: badges.
 * - `sm` 6px: chips, small tiles.
 * - `md` 8px: nav items, inputs, the sidebar hamburger.
 * - `--nc-radius-tile` 10px: the dropzone, icon tiles.
 * - `lg` 12px: cards, the palette, the status panel.
 * - `xl` 14px: the sidebar, mobile cards, setup cards.
 * - `2xl` 18px: the mobile drawer.
 * - `pill` 999px: pills, every button (icon buttons included), switches.
 */
export const radiusCss = css`
  :root {
    --wa-radius-xs: 4px;
    --wa-radius-sm: 6px;
    --wa-radius-md: 8px;
    --nc-radius-tile: 10px;
    --wa-radius-lg: 12px;
    --wa-radius-xl: 14px;
    --wa-radius-2xl: 18px;
    --wa-radius-pill: 999px;
  }
`;
