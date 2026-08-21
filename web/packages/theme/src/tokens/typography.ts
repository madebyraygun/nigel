import { css } from 'lit';

export const typographyCss = css`
  :root {
    /* The machine's own face. A native app's chrome is drawn in it, and
       reading it here rather than bundling a proportional face is the whole
       point: system-ui is whatever the person already reads every menu and
       dialog in. The tail is for engines that do not answer system-ui —
       WebKitGTK and older WKWebView among them, which is both of the shells
       nigel ships in. The token is named sans because it means "the primary
       UI face", which is the name Web Awesome's own internals read. */
    --wa-font-family-sans: system-ui, -apple-system, BlinkMacSystemFont,
      'Segoe UI', Roboto, 'Helvetica Neue', Arial, 'Noto Sans', sans-serif;
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

    /* Plex Mono's two remaining jobs, each named for the job rather than for
       the family, so a component never has to know which face answers it and
       a later change has somewhere to read the intent off.

       Figures are columns whose digits have to land above each other — an
       amount, a count, a date in a table. tabular-nums gets a proportional
       face most of the way there, but only a mono keeps the separators and
       the currency symbol aligned too. */
    --nc-font-figures: var(--wa-font-family-mono);
    /* Money is the figure everything else in this app is about; it has its
       own name because wc-money is where a person's eye goes first. */
    --nc-font-money: var(--nc-font-figures);
    /* The wordmark and the company name: the terminal's own face, which is
       the character the brand is drawn in. */
    --nc-font-brand: var(--wa-font-family-mono);
  }
`;
