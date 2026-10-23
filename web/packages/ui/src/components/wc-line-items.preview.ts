import { html } from 'lit';
import './wc-line-items.js';
import { lineItemsSubtotal, type LineItemValue } from './wc-line-items.js';
import type { Preview } from '../../preview/types.js';

const one: LineItemValue[] = [
  { description: 'Consulting — August', quantity: '10', unitAmount: '150' },
];

const many: LineItemValue[] = [
  { description: 'Consulting — August', quantity: '10', unitAmount: '150' },
  { description: 'Hosting', quantity: '1', unitAmount: '350' },
  { description: 'Out-of-hours support', quantity: '2.5', unitAmount: '220' },
];

/** A short line and one long enough to wrap to three lines at a dialog's width. */
const wrapping: LineItemValue[] = [
  { description: 'Hosting', quantity: '1', unitAmount: '350' },
  {
    description:
      'Design system build, phase 2: component library, tokens and documentation site, including accessibility review of every component state, two rounds of stakeholder revisions, and handover workshops for the in-house team',
    quantity: '1',
    unitAmount: '3200',
  },
];

const preview: Preview = {
  id: 'wc-line-items',
  title: 'Line items',
  group: 'Invoicing',
  description:
    'The repeatable rows an invoice is built from, with a live subtotal. Reordering is up/down buttons — a drag handle has no keyboard equivalent that passes axe without building these anyway.',
  layout: 'stack',
  states: [
    { name: 'one-row', render: () => html`<wc-line-items .items=${one}></wc-line-items>` },
    { name: 'many-rows', render: () => html`<wc-line-items .items=${many}></wc-line-items>` },
    {
      name: 'empty',
      render: () => html`
        <wc-line-items
          .items=${[]}
          list-error="An invoice needs at least one line."
        ></wc-line-items>
      `,
    },
    {
      name: 'editor-short-and-long-descriptions',
      render: () => html`<wc-line-items .items=${wrapping}></wc-line-items>`,
    },
    {
      name: 'readonly-short-and-long-descriptions',
      render: () => html`
        <wc-line-items readonly .items=${wrapping} .total=${lineItemsSubtotal(wrapping)}></wc-line-items>
      `,
    },
    {
      name: 'unit-prefix-in-field',
      render: () => html`
        <wc-line-items
          .items=${[
            { description: 'Platform build', quantity: '1', unitAmount: '12500.00' },
            { description: 'Support hours', quantity: '12.5', unitAmount: '165' },
          ]}
        ></wc-line-items>
      `,
    },
    {
      name: 'readonly',
      render: () => html`
        <wc-line-items readonly .items=${many} .total=${2400}></wc-line-items>
      `,
    },
    {
      name: 'readonly-with-tax',
      render: () => html`
        <wc-line-items readonly .items=${many} .total=${lineItemsSubtotal(many) + 198}></wc-line-items>
      `,
    },
    {
      name: 'field-errors',
      render: () => html`
        <wc-line-items
          .items=${[
            { description: '', quantity: 'lots', unitAmount: '' },
            ...one,
          ]}
          .errors=${[
            {
              description: 'Description is required',
              quantity: 'Quantity must be a number',
              unitAmount: 'Unit amount is required',
            },
          ]}
        ></wc-line-items>
      `,
    },
    {
      name: 'saving',
      render: () => html`<wc-line-items .items=${many} disabled></wc-line-items>`,
    },
  ],
};

export default preview;
