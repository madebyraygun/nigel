import { html } from 'lit';
import './wc-breadcrumb.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-breadcrumb',
  title: 'Breadcrumb',
  group: 'Layout',
  description:
    'Where the user is: the open book (monogram, name, chevron — opens the palette), then the screen, then any level below it.',
  states: [
    {
      name: 'screen',
      render: () =>
        html`<wc-breadcrumb
          book-name="Acme Consulting LLC"
          book-key="acme"
          .trail=${[{ label: 'Dashboard' }]}
        ></wc-breadcrumb>`,
    },
    {
      name: 'invoice detail',
      render: () =>
        html`<wc-breadcrumb
          book-name="Acme Consulting LLC"
          book-key="acme"
          .trail=${[{ label: 'Invoices', id: 'invoices' }, { label: '#1254' }]}
        ></wc-breadcrumb>`,
    },
    {
      name: 'another book',
      render: () =>
        html`<wc-breadcrumb
          book-name="Juniper Labs"
          book-key="juniper"
          .trail=${[{ label: 'Register' }]}
        ></wc-breadcrumb>`,
    },
    {
      name: 'long name in a narrow box',
      render: () =>
        html`<div style="width:260px;">
          <wc-breadcrumb
            book-name="Harbor & Vale Architecture and Interiors Partnership"
            book-key="harbor"
            .trail=${[{ label: 'Reconcile' }]}
          ></wc-breadcrumb>
        </div>`,
    },
  ],
};

export default preview;
