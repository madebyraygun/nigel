import { html } from 'lit';
import './wc-stat-strip.js';
import type { StatStripCell } from './wc-stat-strip.js';
import type { Preview } from '../../preview/types.js';

const three: StatStripCell[] = [
  { id: 'income', label: 'Income, year to date', amount: 184320.5, hint: '2026' },
  { id: 'expenses', label: 'Expenses, year to date', amount: -96540.18, hint: '2026' },
  { id: 'net', label: 'Net', amount: 87780.32, hint: 'Cash basis' },
];

const preview: Preview = {
  id: 'wc-stat-strip',
  title: 'Stat strip',
  group: 'Data',
  description:
    'Headline figures in one card, divided by hairlines. Cells flow in a single row, so a fourth is one more entry. Each cell has its own loading and error state.',
  layout: 'stack',
  states: [
    {
      name: 'three cells',
      render: () => html`<wc-stat-strip .cells=${three}></wc-stat-strip>`,
    },
    {
      name: 'net negative',
      render: () =>
        html`<wc-stat-strip
          .cells=${[
            three[0]!,
            three[1]!,
            { id: 'net', label: 'Net', amount: -12480.07, hint: 'Cash basis' },
          ]}
        ></wc-stat-strip>`,
    },
    {
      name: 'four cells',
      render: () =>
        html`<wc-stat-strip
          .cells=${[
            ...three,
            { id: 'ar', label: 'Outstanding', amount: 6200, hint: '3 invoices' },
          ]}
        ></wc-stat-strip>`,
    },
    {
      name: 'about 50rem wide, three cells',
      render: () =>
        html`<div style="width:50rem;max-width:100%;">
          <wc-stat-strip .cells=${three}></wc-stat-strip>
        </div>`,
    },
    {
      name: 'about 50rem wide, four cells',
      render: () =>
        html`<div style="width:50rem;max-width:100%;">
          <wc-stat-strip
            .cells=${[
              ...three,
              { id: 'ar', label: 'Outstanding', amount: 6200, hint: '3 invoices' },
            ]}
          ></wc-stat-strip>
        </div>`,
    },
    {
      name: 'zero',
      render: () =>
        html`<wc-stat-strip
          .cells=${three.map((c) => ({ ...c, amount: 0 }))}
        ></wc-stat-strip>`,
    },
    {
      name: 'loading',
      render: () =>
        html`<wc-stat-strip
          .cells=${three.map((c) => ({ ...c, loading: true }))}
        ></wc-stat-strip>`,
    },
    {
      name: 'one cell failed',
      render: () =>
        html`<wc-stat-strip
          .cells=${[
            three[0]!,
            { ...three[1]!, error: 'Could not reach the nigel server.' },
            three[2]!,
          ]}
        ></wc-stat-strip>`,
    },
  ],
};

export default preview;
