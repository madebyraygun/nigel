import { html } from 'lit';
import './wc-invoice-status.js';
import { INVOICE_STATUS_WORDS } from './wc-invoice-status.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-invoice-status',
  title: 'Invoice status',
  group: 'Invoicing',
  description:
    'The six derived statuses as a pill: a 1px border, a 6px dot and the word, all in the status colour. The word carries the meaning; colour is never the only cue. `size="l"` is the 24px pill beside a page title.',
  states: [
    ...INVOICE_STATUS_WORDS.map((status) => ({
      name: status,
      render: () => html`<wc-invoice-status status=${status}></wc-invoice-status>`,
    })),
    {
      name: 'unknown',
      render: () => html`<wc-invoice-status status="imported"></wc-invoice-status>`,
    },
    {
      name: 'row',
      render: () => html`
        <div style="display:flex;gap:8px;flex-wrap:wrap;">
          ${INVOICE_STATUS_WORDS.map(
            (status) => html`<wc-invoice-status status=${status}></wc-invoice-status>`,
          )}
        </div>
      `,
    },
    {
      name: 'large',
      render: () => html`
        <div style="display:flex;gap:8px;flex-wrap:wrap;">
          ${INVOICE_STATUS_WORDS.map(
            (status) =>
              html`<wc-invoice-status status=${status} size="l"></wc-invoice-status>`,
          )}
        </div>
      `,
    },
    {
      name: 'in-a-line-of-text',
      render: () => html`
        <p style="max-width:52ch;">
          Invoice #1248 is
          <wc-invoice-status status="overdue"></wc-invoice-status>
          and #1249 is
          <wc-invoice-status status="paid"></wc-invoice-status>
          — the pill sits on the text's centre line.
        </p>
      `,
    },
  ],
};

export default preview;
