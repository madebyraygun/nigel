import { css } from 'lit';

/**
 * The width below which a list table stacks its rows into cards.
 *
 * Read from the table's own frame, not the viewport, so a narrow window, the
 * sidebar and a split view all count. Stated here for tests; the container
 * query below spells the same number, since a custom property cannot stand in
 * a container condition.
 */
export const STACKED_TABLE_MAX_WIDTH = 720;

/**
 * The phone layout every list table shares: below 720px of its frame's width
 * each row becomes a card, so no list scrolls sideways.
 *
 * A component opts in with three things, and keeps its own desktop table:
 *
 * - a `stack` class on the element wrapping the table, which becomes the
 *   query container;
 * - on each cell, `data-stack="title"` for the cell that names the row (the
 *   card's first line), `data-stack="last"` for what closes the card (a status
 *   pill, the row's buttons) and `data-stack="hide"` for what only the
 *   desktop row needs (a chevron);
 * - `data-label` on every other cell, its column heading, which the card shows
 *   beside the value.
 *
 * The header row stays for assistive technology and takes no room. Setting a
 * table's elements to `display: block` takes the table semantics away in some
 * engines, so a component using this states `role="table"`, `rowgroup`, `row`,
 * `columnheader` and `cell` on its elements.
 *
 * Adopt it after the component's own sheet: its rules are written one class
 * more specific than a table's usual cell rules, and later in the cascade.
 */
export const stackedTableCss = css`
  .stack {
    container-type: inline-size;
  }

  @container (max-width: 720px) {
    .stack table,
    .stack tbody,
    .stack tbody tr,
    .stack tbody td {
      display: block;
    }

    .stack table {
      min-width: 0;
      table-layout: auto;
    }

    .stack colgroup {
      display: none;
    }

    .stack thead {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    .stack tbody tr {
      display: grid;
      gap: 4px;
      padding: 12px 16px;
      border-bottom: 1px solid var(--wa-color-border-soft);
    }

    .stack tbody tr:last-child {
      border-bottom: 0;
    }

    .stack tbody tr:hover {
      background: var(--wa-color-surface-alt);
    }

    .stack tbody tr:hover td {
      background: none;
    }

    .stack tbody td {
      height: auto;
      min-width: 0;
      padding: 0;
      border-bottom: 0;
      text-align: start;
      white-space: normal;
      overflow: visible;
      overflow-wrap: anywhere;
    }

    .stack tbody td[data-stack='title'] {
      font-weight: var(--wa-font-weight-medium);
    }

    .stack tbody td[data-stack='last'] {
      order: 1;
      padding-top: 6px;
    }

    .stack tbody td[data-stack='hide'] {
      display: none;
    }

    .stack tbody td[data-label] {
      display: grid;
      grid-template-columns: minmax(0, 9rem) minmax(0, 1fr);
      gap: 12px;
      align-items: baseline;
      font-size: var(--nc-font-size-control);
    }

    .stack tbody td[data-label]::before {
      content: attr(data-label);
      color: var(--wa-color-muted);
      font-size: var(--nc-font-size-meta);
      font-weight: var(--nc-font-weight-table-heading);
    }

    /* A value set to fill its column on the desktop row (an amount aligned
       to the end) reads from the start of its line here. */
    .stack tbody td[data-label] > * {
      justify-self: start;
      width: auto;
      text-align: start;
    }
  }
`;
