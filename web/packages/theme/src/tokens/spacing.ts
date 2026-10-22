import { css } from 'lit';

export const spacingCss = css`
  :root {
    --wa-space-xs: 4px;
    --wa-space-s: 8px;
    --wa-space-m: 12px;
    --wa-space-l: 16px;
    --wa-space-xl: 24px;
    --wa-space-2xl: 32px;

    --nc-icon-size: 20px;

    /* The standard button (controlsCss): 32px by default, 38px at
       size=l, 28px at size=s, with an icon-only button's glyph at
       17px inside its circle. */
    --nc-button-height: 32px;
    --nc-button-height-lg: 38px;
    --nc-button-height-sm: 28px;
    --nc-button-padding-inline: 14px;
    --nc-button-padding-inline-sm: 10px;
    --nc-button-icon-size: 17px;

    /* The floating sidebar panel, expanded and collapsed to its icon rail. */
    --nc-sidebar-width: 224px;
    --nc-sidebar-collapsed-width: 58px;
    /* The top bar above the content, and the sidebar header beside it. */
    --nc-header-height: 50px;
    /* How far a floating panel sits in from the window's edges. */
    --nc-window-inset: 8px;
  }
`;
