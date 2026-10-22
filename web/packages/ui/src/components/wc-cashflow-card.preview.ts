import { html } from 'lit';
import './wc-cashflow-card.js';
import type { BarBucket } from './wc-bar-chart.js';
import type { Preview } from '../../preview/types.js';

const MONTHS = ['Nov', 'Dec', 'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct'];

const year: BarBucket[] = MONTHS.map((label, i) => ({
  label,
  income: 12000 + Math.round(Math.sin(i / 2) * 5200) + i * 240,
  expense: 8200 + Math.round(Math.cos(i / 3) * 3100) + i * 130,
}));

const preview: Preview = {
  id: 'wc-cashflow-card',
  title: 'Cash flow card',
  group: 'Data',
  description:
    'Twelve months of paired income and expense bars with rounded tops, under a title, the period covered and a legend. Figures are repeated in a table only assistive tech reads.',
  layout: 'stack',
  states: [
    {
      name: 'twelve months',
      render: () =>
        html`<wc-cashflow-card .buckets=${year} range="2025 – 26"></wc-cashflow-card>`,
    },
    {
      name: 'short history',
      render: () =>
        html`<wc-cashflow-card
          .buckets=${year.slice(0, 3)}
          range="2026"
        ></wc-cashflow-card>`,
    },
    {
      name: 'lopsided',
      render: () =>
        html`<wc-cashflow-card
          .buckets=${[
            { label: 'Jan', income: 240000, expense: 1200 },
            { label: 'Feb', income: 900, expense: 800 },
            { label: 'Mar', income: 1500, expense: 96000 },
          ]}
          range="2026"
        ></wc-cashflow-card>`,
    },
    {
      name: 'all zero',
      render: () =>
        html`<wc-cashflow-card
          .buckets=${MONTHS.slice(0, 6).map((label) => ({
            label,
            income: 0,
            expense: 0,
          }))}
          range="2026"
        ></wc-cashflow-card>`,
    },
    {
      name: 'empty',
      render: () =>
        html`<wc-cashflow-card
          ><p slot="empty" style="margin:0;color:var(--wa-color-muted);">
            No cash flow yet.
          </p></wc-cashflow-card
        >`,
    },
    {
      name: 'loading',
      render: () => html`<wc-cashflow-card loading></wc-cashflow-card>`,
    },
    {
      name: 'error',
      render: () =>
        html`<wc-cashflow-card
          error="Could not reach the nigel server."
        ></wc-cashflow-card>`,
    },
  ],
};

export default preview;
