import { css } from 'lit';

export const typographyCss = css`
  :root {
    --wa-font-family-sans: 'IBM Plex Mono', ui-monospace, SFMono-Regular,
      'SF Mono', Menlo, Consolas, monospace;
    --wa-font-family-mono: 'IBM Plex Mono', ui-monospace, SFMono-Regular,
      'SF Mono', Menlo, Consolas, monospace;
    --wa-font-size-s: 13px;
    --wa-font-size-base: 14px;
    --wa-font-size-lg: 16px;
    --wa-font-size-xl: 20px;
    --wa-font-size-2xl: 26px;
    --wa-font-weight-normal: 400;
    --wa-font-weight-medium: 500;
    --wa-font-weight-bold: 600;
    --wa-line-height: 1.5;

    /* Now the same stack as everything else, and kept anyway: it names an
       intent — figures that have to align in a column — which a UI that ever
       moves back to a proportional face would need again. */
    --nc-font-money: var(--wa-font-family-mono);
  }
`;
