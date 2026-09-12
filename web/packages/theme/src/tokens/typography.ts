import { css } from 'lit';

export const typographyCss = css`
  :root {
    /* The machine's own face: system-ui is whatever the person already reads
       every menu and dialog in, which is what a native app's chrome is drawn
       in. The tail is for engines that do not answer system-ui — WebKitGTK
       and older WKWebView among them, which is both of the shells nigel
       ships in.

       "sans" here names the role — the primary UI face — not a family, and
       it is the name every component reads. Web Awesome's own compiled
       styles read --wa-font-family-body/-heading/-longform/-code instead;
       tokens/wa-contract.ts is where those four are answered. */
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

    /* Three jobs the bundled face answers, each named for the job rather
       than the family, so a component asks for what it needs and a change of
       face has somewhere to read the intent off.

       Figures are columns whose digits have to land above each other — an
       amount, a count, a date in a table. tabular-nums gets a proportional
       face most of the way there, but only a mono keeps the separators and
       the currency symbol aligned too. Money has its own name over the top
       of it because wc-money is where a reader's eye goes first. Brand is
       the wordmark and the company name, in the character the CLI draws
       itself in. Code-shaped strings — a pattern, an account code, a
       filename — read --wa-font-family-mono directly. */
    --nc-font-figures: var(--wa-font-family-mono);
    --nc-font-money: var(--nc-font-figures);
    --nc-font-brand: var(--wa-font-family-mono);
  }
`;
