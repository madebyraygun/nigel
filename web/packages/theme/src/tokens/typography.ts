import { css } from 'lit';

/**
 * The `--wa-font-size-*` scale is what Web Awesome and the older components
 * read. The `--nc-font-size-*` roles are the app-refresh type scale, named
 * for what they set rather than by size, so a screen asks for "a card title"
 * and the scale can move underneath it.
 */
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

    /* Page h1, 500 weight, drawn with --nc-letter-spacing-tight. */
    --nc-font-size-display: 28px;
    /* Setup headings: the arrival step and the steps after it. */
    --nc-font-size-heading: 26px;
    --nc-font-size-heading-sm: 24px;
    /* Secondary figures: the outstanding total, an invoice's amount. */
    --nc-font-size-figure: 22px;
    /* Settings section titles, 600. */
    --nc-font-size-section: 16px;
    /* Card titles, 600. */
    --nc-font-size-card-title: 15px;
    /* Nav items and body copy. */
    --nc-font-size-body: 13.5px;
    /* Buttons, fields, table rows. */
    --nc-font-size-control: 13px;
    /* Meta lines, captions under a figure, column headers. */
    --nc-font-size-meta: 12px;
    /* The status line and keycaps. */
    --nc-font-size-caption: 11.5px;
    /* Uppercase group labels, 500, drawn with --nc-letter-spacing-label. */
    --nc-font-size-label: 10.5px;

    /* Every table's column and row headings, in every table. */
    --nc-font-weight-table-heading: 700;

    --nc-letter-spacing-tight: -0.02em;
    --nc-letter-spacing-label: 0.08em;

    /* Now the same stack as everything else, and kept anyway: it names an
       intent — figures that have to align in a column — which a UI that ever
       moves back to a proportional face would need again. */
    --nc-font-money: var(--wa-font-family-mono);

    /* Every figure in the app sets on tabular numerals. Inherited, so it
       reaches into every shadow root from here. */
    font-variant-numeric: tabular-nums;
  }
`;
