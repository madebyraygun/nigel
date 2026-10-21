import { html } from 'lit';
import './wc-balances-card.js';
import type { BalanceRow } from './wc-balance-list.js';
import type { Preview } from '../../preview/types.js';

const rows: BalanceRow[] = [
  { name: 'Harbor Checking', accountType: 'checking', balance: 79101.64 },
  { name: 'Harbor Credit Card', accountType: 'credit_card', balance: -1284.37 },
];

const preview: Preview = {
  id: 'wc-balances-card',
  title: 'Balances card',
  group: 'Data',
  description:
    'Account balances in a titled card, each coloured by sign, with a Total row under a heavier rule.',
  states: [
    {
      name: 'with total',
      render: () => html`<wc-balances-card .items=${rows} .total=${77817.27}></wc-balances-card>`,
    },
    {
      name: 'negative total',
      render: () =>
        html`<wc-balances-card
          .items=${[
            { name: 'Cedar Line of Credit', accountType: 'line_of_credit', balance: -9400 },
            rows[1]!,
          ]}
          .total=${-10684.37}
        ></wc-balances-card>`,
    },
    {
      name: 'no total',
      render: () => html`<wc-balances-card .items=${rows}></wc-balances-card>`,
    },
    {
      name: 'long account name',
      render: () =>
        html`<wc-balances-card
          .items=${[
            {
              name: 'Juniper Labs Operating Account With A Very Long Descriptive Name',
              accountType: 'checking',
              balance: 1250,
            },
          ]}
          .total=${1250}
        ></wc-balances-card>`,
    },
    {
      name: 'empty',
      render: () => html`<wc-balances-card></wc-balances-card>`,
    },
    {
      name: 'loading',
      render: () => html`<wc-balances-card loading></wc-balances-card>`,
    },
    {
      name: 'error',
      render: () =>
        html`<wc-balances-card
          error="Could not reach the nigel server."
        ></wc-balances-card>`,
    },
  ],
};

export default preview;
