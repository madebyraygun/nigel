import { html } from 'lit';
import './wc-invoice-summary.js';
import './wc-action-pills.js';
import type { ActionPill } from './wc-action-pills.js';
import type { Preview } from '../../preview/types.js';

interface SummaryProps {
  number: number;
  status: string;
  clientName: string | null;
  clientEmail: string | null;
  total: number;
  balance: number | null;
  issueDate: string;
  dueDate: string | null;
  dueNote: string;
}

const base: SummaryProps = {
  number: 1255,
  status: 'sent',
  clientName: 'Acme Co',
  clientEmail: 'billing@acme.example',
  total: 1850,
  balance: 1850,
  issueDate: '2026-10-06',
  dueDate: '2026-11-05',
  dueNote: 'Due in 17 days',
};

const ACTIONS: ActionPill[] = [
  { key: 'send', label: 'Resend…', tone: 'primary' },
  { key: 'pay', label: 'Record payment…' },
  { key: 'edit', label: 'Edit', disabled: true },
  { key: 'duplicate', label: 'Duplicate' },
  { key: 'void', label: 'Void…', tone: 'danger' },
];

function summary(props: Partial<SummaryProps>, actions = false) {
  const value = { ...base, ...props };
  return html`
    <wc-invoice-summary
      .number=${value.number}
      status=${value.status}
      .clientName=${value.clientName}
      .clientEmail=${value.clientEmail}
      .total=${value.total}
      .balance=${value.balance}
      .issueDate=${value.issueDate}
      .dueDate=${value.dueDate}
      .dueNote=${value.dueNote}
    >
      ${actions
        ? html`<wc-action-pills slot="actions" align="end" .actions=${ACTIONS}></wc-action-pills>`
        : ''}
    </wc-invoice-summary>
  `;
}

const preview: Preview = {
  id: 'wc-invoice-summary',
  title: 'Invoice summary',
  group: 'Invoicing',
  description:
    'The detail head: the title with its status pill, the actions slotted beside it, and a fact strip of Client, Issued, Due and Balance. An overdue due date reads in the expense colour with how late it is.',
  layout: 'stack',
  states: [
    {
      name: 'partial-with-actions',
      render: () =>
        summary(
          {
            number: 1254,
            status: 'partial',
            total: 3200,
            balance: 1200,
            issueDate: '2026-09-20',
            dueDate: '2026-10-20',
            dueNote: 'Due tomorrow',
          },
          true,
        ),
    },
    {
      name: 'draft',
      render: () =>
        summary({ number: 1256, status: 'draft', clientName: 'Northwind Traders', clientEmail: null, dueDate: null, dueNote: '' }),
    },
    { name: 'sent', render: () => summary({}) },
    {
      name: 'overdue',
      render: () =>
        summary({
          number: 1253,
          status: 'overdue',
          clientName: 'Globex',
          clientEmail: 'ap@globex.example',
          total: 960,
          balance: 960,
          issueDate: '2026-08-31',
          dueDate: '2026-09-30',
          dueNote: '19 days overdue',
        }),
    },
    {
      name: 'paid',
      render: () => summary({ number: 1252, status: 'paid', total: 4000, balance: 0, dueNote: '' }),
    },
    {
      name: 'void',
      render: () =>
        summary({ number: 1250, status: 'void', total: 500, balance: null, dueDate: null, dueNote: '' }),
    },
    { name: 'orphaned-client', render: () => summary({ clientName: null, clientEmail: null }) },
  ],
};

export default preview;
