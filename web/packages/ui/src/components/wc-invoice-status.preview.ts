import { html } from 'lit';
import './wc-invoice-status.js';
import { INVOICE_STATUS_WORDS } from './wc-invoice-status.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-invoice-status',
  title: 'Invoice status',
  group: 'Invoicing',
  description:
    'The six derived statuses as an icon and a word. Colour is a third cue, never the only one. The icons are SVGs at 1em: IBM Plex Mono has a glyph for none of the six shapes.',
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
      name: 'in-a-line-of-text',
      render: () => html`
        <p style="max-width:52ch;">
          Invoice #1248 is
          <wc-invoice-status status="overdue"></wc-invoice-status>
          and #1249 is
          <wc-invoice-status status="paid"></wc-invoice-status>
          — the marks are sized against the text they sit in.
        </p>
      `,
    },
  ],
};

export default preview;
