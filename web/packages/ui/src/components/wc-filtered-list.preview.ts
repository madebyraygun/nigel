import { html, type TemplateResult } from 'lit';
import { unsafeHTML } from 'lit/directives/unsafe-html.js';
import { controlsCss } from '@nigel/theme';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/switch/switch.js';
import './wc-filtered-list.js';
import './wc-invoice-table.js';
import type { InvoiceTableRow } from './wc-invoice-table.js';
import type { Preview } from '../../preview/types.js';

/**
 * The sheet a screen adopts, injected so the select's inline label shows
 * here as it does on a screen: the harness's own shadow root adopts none.
 */
const SHEET = `<style>${controlsCss.cssText}</style>`;

const rows: InvoiceTableRow[] = [
  {
    number: 1251,
    href: '#/invoices?number=1251',
    status: 'draft',
    clientName: 'Cedar Systems',
    total: 4190,
    balance: 4190,
    dueDate: '2026-11-19',
  },
  {
    number: 1250,
    href: '#/invoices?number=1250',
    status: 'sent',
    clientName: 'Juniper Labs',
    total: 5200,
    balance: 5200,
    dueDate: '2026-11-09',
  },
];

const statusLinks = (): TemplateResult => html`
  <span class="label" slot="filters">Status</span>
  <a slot="filters" href="#all" aria-current="true">All</a>
  <a slot="filters" href="#draft">Draft</a>
  <a slot="filters" href="#open">Open</a>
`;

const preview: Preview = {
  id: 'wc-filtered-list',
  title: 'Filtered List',
  group: 'Data',
  description:
    'A list with its filter row: everything slotted into filters on one wrapping, vertically centred row — a label before its chips, a select labelled inline — and --nc-toolbar-gap down to the table. wc-manager-layout spaces its toolbar the same way.',
  layout: 'stack',
  states: [
    {
      name: 'chips',
      render: () => html`${unsafeHTML(SHEET)}
        <wc-filtered-list label="Invoice filters">
          ${statusLinks()}
          <wc-invoice-table .rows=${rows}></wc-invoice-table>
        </wc-filtered-list>`,
    },
    {
      name: 'chips and a select',
      render: () => html`${unsafeHTML(SHEET)}
        <wc-filtered-list label="Document filters">
          ${statusLinks()}
          <wa-select slot="filters" size="s" label="Client" value="">
            <wa-option value="">All clients</wa-option>
            <wa-option value="1">Acme Co</wa-option>
          </wa-select>
          <wc-invoice-table .rows=${rows}></wc-invoice-table>
        </wc-filtered-list>`,
    },
    {
      name: 'narrow',
      render: () => html`${unsafeHTML(SHEET)}
        <div style="max-width: 22rem">
          <wc-filtered-list label="Document filters">
            ${statusLinks()}
            <wa-select slot="filters" size="s" label="Client" value="">
              <wa-option value="">All clients</wa-option>
            </wa-select>
            <wc-invoice-table .rows=${rows}></wc-invoice-table>
          </wc-filtered-list>
        </div>`,
    },
    {
      name: 'no filters',
      render: () => html`<wc-filtered-list>
        <wc-invoice-table .rows=${rows}></wc-invoice-table>
      </wc-filtered-list>`,
    },
  ],
};

export default preview;
