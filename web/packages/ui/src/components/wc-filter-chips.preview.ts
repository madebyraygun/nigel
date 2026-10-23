import { html } from 'lit';
import './wc-filter-chips.js';
import type { FilterChip } from './wc-filter-chips.js';
import type { Preview } from '../../preview/types.js';

const STATUS: FilterChip[] = [
  { value: 'all', label: 'All', count: 7, href: '#/invoices' },
  { value: 'draft', label: 'Draft', count: 1, href: '#/invoices?status=draft' },
  { value: 'sent', label: 'Sent', count: 1, href: '#/invoices?status=sent' },
  { value: 'partial', label: 'Partial', count: 1, href: '#/invoices?status=partial' },
  { value: 'overdue', label: 'Overdue', count: 1, href: '#/invoices?status=overdue' },
  { value: 'paid', label: 'Paid', count: 2, href: '#/invoices?status=paid' },
  { value: 'void', label: 'Void', count: 1, href: '#/invoices?status=void' },
];

const preview: Preview = {
  id: 'wc-filter-chips',
  title: 'Filter chips',
  group: 'Controls',
  description:
    'A labelled row of filter links, each with an optional count. The chip in effect has a brand border and the highlight fill, and says so with aria-current.',
  layout: 'stack',
  states: [
    {
      name: 'all',
      render: () =>
        html`<wc-filter-chips label="Status" active="all" .chips=${STATUS}></wc-filter-chips>`,
    },
    {
      name: 'one-active',
      render: () =>
        html`<wc-filter-chips label="Status" active="overdue" .chips=${STATUS}></wc-filter-chips>`,
    },
    {
      name: 'with-a-clear-chip',
      render: () =>
        html`<wc-filter-chips
          label="Status"
          active="all"
          .chips=${[
            ...STATUS,
            { value: 'clear-client', label: 'Clear client filter', href: '#/invoices' },
          ]}
        ></wc-filter-chips>`,
    },
    {
      name: 'unlabelled',
      render: () =>
        html`<wc-filter-chips active="paid" .chips=${STATUS.slice(0, 3)}></wc-filter-chips>`,
    },
  ],
};

export default preview;
