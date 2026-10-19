import { html } from 'lit';
import './wc-recipient-editor.js';
import {
  defaultRecipients,
  validateRecipients,
  type RecipientContactOption,
  type RecipientEditorValue,
} from './wc-recipient-editor.js';
import type { Preview } from '../../preview/types.js';

const CONTACTS: RecipientContactOption[] = [
  { name: 'Pat Example', email: 'pat@cedar.test', isBilling: true },
  { name: 'Sam Example', email: 'sam@cedar.test', isBilling: false },
  { name: null, email: 'legal@cedar.test', isBilling: false },
];

const WITH_COLLABORATORS: RecipientEditorValue = {
  signer: { name: 'Pat Example', email: 'pat@cedar.test' },
  collaborators: [
    { name: 'Sam Example', email: 'sam@cedar.test' },
    { name: 'Harbor & Vale counsel', email: 'counsel@harborvale.test' },
  ],
};

const INVALID: RecipientEditorValue = {
  signer: { name: '', email: 'pat@cedar.test' },
  collaborators: [
    { name: 'Sam Example', email: 'sam.cedar.test' },
    { name: 'Pat again', email: 'PAT@cedar.test' },
  ],
};

const preview: Preview = {
  id: 'wc-recipient-editor',
  title: 'Recipient editor',
  group: 'Documents',
  description:
    'Who a document goes to: exactly one signer, defaulting to the billing contact, and any number of collaborators picked from the client’s contacts or typed in.',
  layout: 'stack',
  states: [
    {
      name: 'default',
      render: () => html`
        <wc-recipient-editor
          .value=${defaultRecipients(CONTACTS)}
          .contacts=${CONTACTS}
        ></wc-recipient-editor>
      `,
    },
    {
      name: 'with-collaborators',
      render: () => html`
        <wc-recipient-editor
          .value=${WITH_COLLABORATORS}
          .contacts=${CONTACTS}
        ></wc-recipient-editor>
      `,
    },
    {
      name: 'errors',
      render: () => html`
        <wc-recipient-editor
          .value=${INVALID}
          .contacts=${CONTACTS}
          .errors=${validateRecipients(INVALID)}
        ></wc-recipient-editor>
      `,
    },
    {
      name: 'disabled',
      render: () => html`
        <wc-recipient-editor
          disabled
          .value=${WITH_COLLABORATORS}
          .contacts=${CONTACTS}
        ></wc-recipient-editor>
      `,
    },
    {
      name: 'no-contacts',
      render: () => html`<wc-recipient-editor></wc-recipient-editor>`,
    },
  ],
};

export default preview;
