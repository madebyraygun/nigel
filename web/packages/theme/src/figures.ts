import { css } from 'lit';

/**
 * The figures treatment, adopted by any component that renders a column of
 * digits. The face and the tabular numerals travel together — a cell that
 * gets one without the other aligns on some engines and not others — so the
 * pair is declared once here and a component marks the cell (never the
 * heading above it) with `class="figure"`. A native input inside a marked
 * cell takes the same face explicitly, because form controls do not inherit
 * a family on their own.
 */
export const figuresCss = css`
  .figure {
    font-family: var(--nc-font-figures);
    font-variant-numeric: tabular-nums;
  }

  .figure input {
    font-family: inherit;
    font-variant-numeric: inherit;
  }
`;
