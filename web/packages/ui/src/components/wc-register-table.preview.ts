import { html } from 'lit';
import './wc-register-table.js';
import type { RegisterTableRow } from './wc-register-table.js';
import type { Preview } from '../../preview/types.js';

const rows: RegisterTableRow[] = [
  {
    id: 101,
    date: '2025-03-03',
    description: 'ADOBE CREATIVE CLOUD SUBSCRIPTION',
    amount: -54.99,
    category: 'Software / Subscriptions',
    categoryId: 12,
    vendor: 'Adobe',
    accountName: 'BofA Credit Card',
    isFlagged: false,
  },
  {
    id: 102,
    date: '2025-03-05',
    description: 'CLIENT PAYMENT — NORTHWIND',
    amount: 8400,
    category: 'Consulting income',
    categoryId: 3,
    vendor: 'Northwind',
    accountName: 'BofA Checking',
    isFlagged: false,
  },
  {
    id: 103,
    date: '2025-03-07',
    description: 'SQ *BLUE BOTTLE COFFEE',
    amount: -18.5,
    category: null,
    categoryId: null,
    vendor: null,
    accountName: 'BofA Credit Card',
    isFlagged: true,
  },
  {
    id: 104,
    date: '2025-03-11',
    description:
      'WIRE TRANSFER FEE — INTERNATIONAL OUTGOING, REFERENCE 88213-A, DESCRIPTION LONG ENOUGH TO WRAP ACROSS LINES',
    amount: -45,
    category: 'Bank fees',
    categoryId: 21,
    vendor: null,
    accountName: 'BofA Checking',
    isFlagged: false,
  },
];

/**
 * The widest row the register can realistically be asked to draw: a 60-column
 * bank description, a long vendor, the longest name in the stock chart of
 * accounts (`crates/nigel-core/src/db.rs`), and a six-figure amount, all on one line with the
 * account column showing.
 *
 * IBM Plex Mono advances every glyph at 0.6em — 8.4px at the 14px base — so
 * that description alone wants about 500px before padding. This is where a
 * monospace UI overflows first, which is why the state is declared
 * permanently rather than checked once. The row clips rather than wraps: one
 * line per transaction is what lets the window place rows by arithmetic, and
 * the full text stays in the DOM and on the cell's title.
 */
const widestRow: RegisterTableRow[] = [
  {
    id: 900,
    date: '2025-11-28',
    description: 'ACH DEBIT NORTHWIND TRADING COMPANY INTL SETTLEMENT 00421',
    amount: -128450.75,
    category: 'Software & Subscriptions',
    categoryId: 12,
    vendor: 'Northwind Trading Company',
    accountName: 'BofA Line of Credit',
    isFlagged: true,
  },
];

/** More rows than any sensible window shows, so the scroller has work to do. */
const longRegister: RegisterTableRow[] = Array.from({ length: 32 }, (_, index) => {
  const base = rows[index % rows.length] as RegisterTableRow;
  return { ...base, id: 1000 + index };
});

/**
 * Past the windowing threshold, so this state shows what a real unfiltered
 * register looks like: a scrollbar the length of 1,872 rows over a few dozen
 * of them in the DOM.
 */
const wholeYear: RegisterTableRow[] = Array.from({ length: 1872 }, (_, index) => {
  const base = rows[index % rows.length] as RegisterTableRow;
  return { ...base, id: 2000 + index };
});

/** Wider than the Amount column, so the wide layout clips it and its title has it. */
const sevenFigures: RegisterTableRow = {
  id: 105,
  date: '2025-03-14',
  description: 'WIRE IN — CEDAR SYSTEMS ACQUISITION',
  amount: 1234567.89,
  category: 'Consulting income',
  categoryId: 3,
  vendor: 'Cedar Systems',
  accountName: 'BofA Checking',
  isFlagged: false,
};

const preview = {
  id: 'wc-register-table',
  title: 'Register table',
  group: 'Data',
  layout: 'stack',
  description:
    'The transaction register. Selection follows focus through a roving tabindex; a row renders no wa-* components, because an unfiltered register is thousands of rows.',
  states: [
    {
      name: 'default',
      render: () => html`
        <wc-register-table
          .rows=${rows}
          .selectedId=${102}
          .total=${8281.51}
          footer-label="All accounts"
          footer-note="4 of 480 rows"
        ></wc-register-table>
      `,
    },
    {
      // A phone's card: below 860px every row is two lines — flag, date,
      // description and amount, then category · vendor · account — at one
      // height, with no header row and no sideways scroll. The seven-figure
      // amount is whole on its line.
      name: 'stacked',
      render: () => html`
        <div style="inline-size: 22rem">
          <wc-register-table
            .rows=${[...rows, sevenFigures]}
            .total=${8281.51}
            footer-label="All accounts"
          ></wc-register-table>
        </div>
      `,
    },
    {
      name: 'stacked-selected',
      render: () => html`
        <div style="inline-size: 22rem">
          <wc-register-table
            .rows=${rows}
            .selectedId=${102}
            .total=${8281.51}
            footer-label="BofA Checking"
          ></wc-register-table>
        </div>
      `,
    },
    {
      // Flagged and uncategorised: the stripe runs down both lines, and an
      // empty category reads "Uncategorized" in the flag colour.
      name: 'stacked-flagged',
      render: () => html`
        <div style="inline-size: 22rem">
          <wc-register-table
            .rows=${rows.filter((row) => row.isFlagged || row.categoryId === null)}
            .total=${-18.5}
            footer-label="All accounts"
          ></wc-register-table>
        </div>
      `,
    },
    {
      name: 'longest-realistic-row',
      render: () => html`
        <wc-register-table
          .rows=${widestRow}
          .selectedId=${900}
          .total=${-128450.75}
          footer-note="1 of 1 row"
        ></wc-register-table>
      `,
    },
    {
      name: 'filling-its-parent',
      render: () => html`
        <div style="display: flex; flex-direction: column; height: 18rem">
          <wc-register-table
            fill
            .rows=${longRegister}
            .selectedId=${1000}
            .total=${8281.51}
            footer-label="BofA Checking"
            footer-note="32 of 1,872 rows"
          ></wc-register-table>
        </div>
      `,
    },
    {
      name: 'windowed',
      render: () => html`
        <div style="display: flex; flex-direction: column; height: 22rem">
          <wc-register-table
            fill
            .rows=${wholeYear}
            .selectedId=${2000}
            .total=${-12480.19}
            footer-note="1,872 rows"
          ></wc-register-table>
        </div>
      `,
    },
    {
      name: 'dense',
      render: () => html`
        <wc-register-table dense .rows=${rows}></wc-register-table>
      `,
    },
    {
      name: 'empty',
      render: () => html`
        <wc-register-table
          .rows=${[]}
          empty-message="No transactions match this search."
        ></wc-register-table>
      `,
    },
    {
      // The register screen is a flex column filling the content area, and the
      // table takes what the toolbar left; with no rows that height is what its
      // empty state centres in. The dashed box stands in for the area.
      name: 'empty-filling-a-screen',
      render: () => html`
        <div
          style="display:flex; flex-direction:column; block-size:22rem; border:1px dashed var(--wa-color-border);"
        >
          <wc-register-table
            .rows=${[]}
            empty-message="No transactions in this period."
          ></wc-register-table>
        </div>
      `,
    },
    {
      name: 'flagged',
      render: () => html`
        <wc-register-table
          .rows=${rows.filter((row) => row.isFlagged)}
        ></wc-register-table>
      `,
    },
    {
      name: 'saving',
      render: () => html`
        <wc-register-table
          .rows=${rows}
          .selectedId=${101}
          .busyId=${101}
        ></wc-register-table>
      `,
    },
    {
      name: 'no-account-column',
      render: () => html`
        <wc-register-table
          .rows=${rows}
          .showAccount=${false}
          .total=${-120.99}
        ></wc-register-table>
      `,
    },
    {
      name: 'readonly',
      render: () => html`
        <wc-register-table
          readonly
          .rows=${rows}
          .total=${-120.99}
          footer-note="Read-only — editing lives in the register"
        ></wc-register-table>
      `,
    },
  ],
} satisfies Preview;

export default preview;
