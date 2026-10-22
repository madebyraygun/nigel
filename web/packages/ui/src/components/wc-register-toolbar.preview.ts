import { html } from 'lit';
import './wc-register-toolbar.js';
import './wc-shortcut-help.js';
import { REGISTER_SHORTCUTS } from './wc-register-table.js';
import type { AccountOption } from './wc-register-toolbar.js';
import type { Preview } from '../../preview/types.js';

const accounts: AccountOption[] = [
  { id: 1, name: 'BofA Checking' },
  { id: 2, name: 'BofA Credit Card' },
  { id: 3, name: 'Line of Credit' },
];

/** What the register puts in the end slot. */
const keys = html`<wc-shortcut-help
  slot="end"
  label="Keys"
  heading="Register shortcuts"
  .shortcuts=${[...REGISTER_SHORTCUTS]}
></wc-shortcut-help>`;

const preview: Preview = {
  id: 'wc-register-toolbar',
  title: 'Register toolbar',
  group: 'Navigation',
  layout: 'stack',
  description:
    'Account filter, period stepper, All / Year / Month, incremental search and the row count, in one wrapping row of 32px controls; the end slot takes the screen\'s own controls (the register\'s Keys legend). The row count is a live region, so a search announces its result. Below 48rem the row wraps into three lines.',
  states: [
    {
      name: 'default',
      render: () => html`
        <wc-register-toolbar
          .accounts=${accounts}
          .totalCount=${480}
        >${keys}</wc-register-toolbar
        >
      `,
    },
    {
      name: 'filtered',
      render: () => html`
        <wc-register-toolbar
          .accounts=${accounts}
          account="BofA Checking"
          .period=${{ kind: 'month', year: 2025, month: 3 } as const}
          .totalCount=${41}
        >${keys}</wc-register-toolbar
        >
      `,
    },
    {
      name: 'searching',
      render: () => html`
        <wc-register-toolbar
          .accounts=${accounts}
          search="adobe"
          .matchCount=${7}
          .totalCount=${480}
        >${keys}</wc-register-toolbar
        >
      `,
    },
    {
      name: 'no-matches',
      render: () => html`
        <wc-register-toolbar
          .accounts=${accounts}
          search="zzzz"
          .matchCount=${0}
          .totalCount=${480}
        >${keys}</wc-register-toolbar
        >
      `,
    },
    {
      name: 'busy',
      render: () => html`
        <wc-register-toolbar
          busy
          .accounts=${accounts}
          .totalCount=${0}
        >${keys}</wc-register-toolbar
        >
      `,
    },
  ],
};

export default preview;
