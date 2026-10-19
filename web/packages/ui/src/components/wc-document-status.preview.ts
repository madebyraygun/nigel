import { html } from 'lit';
import './wc-document-status.js';
import { DOCUMENT_STATUS_WORDS } from './wc-document-status.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-document-status',
  title: 'Document status',
  group: 'Documents',
  description:
    'The seven document statuses as an icon and a word. Colour is a third cue, never the only one.',
  states: [
    ...DOCUMENT_STATUS_WORDS.map((status) => ({
      name: status,
      render: () => html`<wc-document-status status=${status}></wc-document-status>`,
    })),
    {
      name: 'unknown',
      render: () => html`<wc-document-status status="imported"></wc-document-status>`,
    },
  ],
};

export default preview;
