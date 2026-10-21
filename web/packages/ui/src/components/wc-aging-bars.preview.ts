import { html } from 'lit';
import './wc-aging-bars.js';
import type { AgingBucketView } from './wc-aging-bars.js';
import type { Preview } from '../../preview/types.js';

const LABELS = ['current', '1-30', '31-60', '61-90', '90+'];

function buckets(totals: number[], labels = LABELS): AgingBucketView[] {
  return labels.map((label, index) => ({
    label,
    count: totals[index] === 0 ? 0 : 1,
    total: totals[index] ?? 0,
  }));
}

const full = buckets([3050, 960, 420, 280, 150]);
const demo = buckets([3050, 960, 0, 0, 0]);

const preview: Preview = {
  id: 'wc-aging-bars',
  title: 'Outstanding card',
  group: 'Invoicing',
  description:
    'What is owed, across how many open invoices, and how old it is: a stacked bar over the five aging buckets and a legend under it. The bar is decoration; every figure is in the legend.',
  layout: 'stack',
  states: [
    {
      name: 'default',
      render: () => html`
        <wc-aging-bars
          .buckets=${demo}
          .total=${4010}
          as-of="2026-10-19"
          href="#/reports?report=aging"
        ></wc-aging-bars>
      `,
    },
    {
      name: 'all-buckets',
      render: () => html`
        <wc-aging-bars
          .buckets=${full}
          .total=${4860}
          as-of="2026-08-07"
          href="#/reports?report=aging"
        ></wc-aging-bars>
      `,
    },
    {
      name: 'one-bucket',
      render: () => html`
        <wc-aging-bars
          .buckets=${buckets([0, 0, 960, 0, 0])}
          .total=${960}
          as-of="2026-08-07"
        ></wc-aging-bars>
      `,
    },
    {
      name: 'all-zero',
      render: () => html`
        <wc-aging-bars
          .buckets=${buckets([0, 0, 0, 0, 0])}
          .total=${0}
          as-of="2026-08-07"
        ></wc-aging-bars>
      `,
    },
    {
      name: 'long-labels',
      render: () => html`
        <wc-aging-bars
          .buckets=${buckets(
            [4200, 1850, 0, 960, 0],
            [
              'Not yet due',
              '1-30 days late',
              '31-60 days late',
              '61-90 days late',
              'More than 90 days late',
            ],
          )}
          .total=${7010}
          as-of="2026-08-07"
        ></wc-aging-bars>
      `,
    },
    {
      name: 'no-buckets',
      render: () => html`<wc-aging-bars .buckets=${[]}></wc-aging-bars>`,
    },
  ],
};

export default preview;
