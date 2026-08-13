import { html } from 'lit';
import './wc-document-frame.js';
import type { Preview } from '../../preview/types.js';

// A data: URL so the harness has something to frame without a server.
const page = `data:text/html;charset=utf-8,${encodeURIComponent(
  '<h1>Invoice #1251</h1><p>Acme Co — $1,850.00</p>',
)}`;

const document = '<h1>Invoice #1251</h1><p>Acme Co — $1,850.00</p>';

const preview: Preview = {
  id: 'wc-document-frame',
  title: 'Document frame',
  group: 'Invoicing',
  description:
    'One document in a sandboxed iframe, with a spinner until it loads. The sandbox omits allow-same-origin and allow-scripts, so the framed document cannot reach the app it is embedded in — which is the only real control over a srcdoc, since the route’s headers do not apply to one.',
  layout: 'stack',
  states: [
    {
      name: 'loading',
      // Nothing to load, so the spinner is what there is to look at.
      render: () => html`<wc-document-frame height="12rem"></wc-document-frame>`,
    },
    {
      name: 'loaded-srcdoc',
      render: () => html`
        <wc-document-frame
          height="12rem"
          label="Invoice preview"
          .srcdoc=${document}
        ></wc-document-frame>
      `,
    },
    {
      name: 'loaded-src',
      render: () => html`
        <wc-document-frame height="12rem" label="Invoice preview" .src=${page}></wc-document-frame>
      `,
    },
  ],
};

export default preview;
