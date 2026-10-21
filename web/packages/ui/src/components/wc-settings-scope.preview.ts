import { html } from 'lit';
import './wc-settings-scope.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-settings-scope',
  title: 'Settings Scope',
  group: 'Layout',
  description:
    'Opens a group of settings and says what it applies to: the open book, or this device.',
  layout: 'stack',
  states: [
    {
      name: 'this book',
      render: () =>
        html`<wc-settings-scope
          kind="book"
          label="This book"
          heading="Acme Consulting LLC"
          seed="~/Documents/Nigel/acme-consulting"
        ></wc-settings-scope>`,
    },
    {
      name: 'this device',
      render: () =>
        html`<wc-settings-scope
          kind="device"
          label="This device"
          heading="Applies to every set of books"
        ></wc-settings-scope>`,
    },
    {
      name: 'long name',
      render: () =>
        html`<div style="width:280px;">
          <wc-settings-scope
            kind="book"
            label="This book"
            heading="Harbor & Vale Architecture and Interiors Partnership"
            seed="harbor"
          ></wc-settings-scope>
        </div>`,
    },
  ],
};

export default preview;
