import { html } from 'lit';
import './wc-document-table.js';
import type { DocumentTableRow } from './wc-document-table.js';
import type { Preview } from '../../preview/types.js';

/** One document per status. */
const rows: DocumentTableRow[] = [
  {
    id: 21,
    title: 'Website proposal',
    kind: 'proposal',
    status: 'draft',
    clientName: 'Juniper Labs',
    version: 1,
    updatedAt: '2026-09-28',
    href: '#/documents?id=21',
  },
  {
    id: 20,
    title: 'Master services agreement',
    kind: 'contract',
    status: 'sent',
    clientName: 'Cedar Systems',
    version: 2,
    updatedAt: '2026-09-25',
    href: '#/documents?id=20',
  },
  {
    id: 19,
    title: 'Retainer terms',
    kind: 'contract',
    status: 'changes_requested',
    clientName: 'Harbor & Vale',
    version: 3,
    updatedAt: '2026-09-22',
    href: '#/documents?id=19',
  },
  {
    id: 18,
    title: 'Brand refresh proposal',
    kind: 'proposal',
    status: 'accepted',
    clientName: 'Cedar Systems',
    version: 1,
    updatedAt: '2026-09-18',
    href: '#/documents?id=18',
  },
  {
    id: 17,
    title: 'Data audit proposal',
    kind: 'proposal',
    status: 'declined',
    clientName: 'Juniper Labs',
    version: 2,
    updatedAt: '2026-09-10',
    href: '#/documents?id=17',
  },
  {
    id: 16,
    title: 'Statement of work',
    kind: 'contract',
    status: 'executed',
    clientName: 'Harbor & Vale',
    version: 1,
    updatedAt: '2026-09-03',
    href: '#/documents?id=16',
  },
  {
    id: 15,
    title: 'Support agreement',
    kind: 'contract',
    status: 'withdrawn',
    clientName: 'Cedar Systems',
    version: 1,
    updatedAt: '2026-08-29',
    href: '#/documents?id=15',
  },
];

const preview: Preview = {
  id: 'wc-document-table',
  title: 'Document table',
  group: 'Documents',
  description:
    'Title, status, kind, client, version and last update. A document whose client is gone shows an em dash.',
  layout: 'stack',
  states: [
    { name: 'list', render: () => html`<wc-document-table .rows=${rows}></wc-document-table>` },
    {
      name: 'orphaned-client',
      render: () => html`
        <wc-document-table .rows=${[{ ...rows[1], clientName: null }]}></wc-document-table>
      `,
    },
    { name: 'loading', render: () => html`<wc-document-table loading></wc-document-table>` },
    {
      name: 'empty',
      render: () => html`
        <wc-document-table
          .rows=${[]}
          empty-message="No documents yet — New document."
        ></wc-document-table>
      `,
    },
    {
      name: 'unlinked',
      render: () => html`
        <wc-document-table
          .rows=${rows.map((row) => ({ ...row, href: undefined }))}
        ></wc-document-table>
      `,
    },
  ],
};

export default preview;
