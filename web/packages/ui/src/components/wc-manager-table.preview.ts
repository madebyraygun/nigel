import { html } from 'lit';
import './wc-manager-table.js';
import './wc-panel.js';
import type { Preview } from '../../preview/types.js';
import type { ManagerAction, ManagerColumn, ManagerRow } from './wc-manager-table.js';

const accountColumns: ManagerColumn[] = [
  { key: 'name', label: 'Name' },
  { key: 'type', label: 'Type' },
  { key: 'institution', label: 'Institution' },
  { key: 'lastFour', label: 'Last four' },
];

const accountRows: ManagerRow[] = [
  {
    id: 1,
    label: 'BofA Checking',
    cells: ['BofA Checking', 'Checking', 'Bank of America', '4821'],
  },
  {
    id: 2,
    label: 'Gusto Payroll',
    cells: ['Gusto Payroll', 'Payroll', null, null],
  },
];

const ruleColumns: ManagerColumn[] = [
  { key: 'pattern', label: 'Pattern', mono: true },
  { key: 'matchType', label: 'Match' },
  { key: 'category', label: 'Category' },
  { key: 'vendor', label: 'Vendor' },
  { key: 'priority', label: 'Priority', align: 'end' },
  { key: 'hits', label: 'Hits', align: 'end' },
];

const ruleRows: ManagerRow[] = [
  {
    id: 7,
    label: 'ADOBE',
    cells: ['ADOBE', 'Contains', 'Software / Subscriptions', 'Adobe', 10, 42],
  },
  {
    id: 8,
    label: 'SQ *',
    cells: ['SQ *', 'Starts with', 'Meals / Entertainment', null, 0, 3],
  },
];

const editDelete: ManagerAction[] = [
  { name: 'edit', label: 'Edit', icon: 'wc-icon-edit' },
  { name: 'delete', label: 'Delete', icon: 'wc-icon-trash', variant: 'danger' },
];

const preview: Preview = {
  id: 'wc-manager-table',
  title: 'Manager table',
  group: 'Data',
  description:
    'The editable list every manager screen is built on. Row actions carry the row name in their labels, so a column of Delete buttons is still navigable by ear.',
  layout: 'stack',
  states: [
    {
      name: 'default',
      render: () => html`
        <wc-manager-table
          caption="Accounts"
          .columns=${accountColumns}
          .rows=${accountRows}
          .actions=${editDelete}
        ></wc-manager-table>
      `,
    },
    {
      name: 'with-null-cells',
      render: () => html`
        <wc-manager-table
          caption="Accounts with missing details"
          .columns=${accountColumns}
          .rows=${[accountRows[1]]}
          .actions=${editDelete}
        ></wc-manager-table>
      `,
    },
    {
      name: 'numeric-columns',
      render: () => html`
        <wc-manager-table
          caption="Categorization rules"
          .columns=${ruleColumns}
          .rows=${ruleRows}
          .actions=${editDelete}
        ></wc-manager-table>
      `,
    },
    {
      name: 'busy-row',
      render: () => html`
        <wc-manager-table
          caption="Accounts"
          .columns=${accountColumns}
          .rows=${accountRows}
          .actions=${editDelete}
          .busyId=${1}
        ></wc-manager-table>
      `,
    },
    {
      name: 'single-action',
      render: () => html`
        <wc-manager-table
          caption="Categorization rules"
          .columns=${ruleColumns}
          .rows=${ruleRows}
          .actions=${[editDelete[1]]}
        ></wc-manager-table>
      `,
    },
    {
      name: 'no-actions',
      render: () => html`
        <wc-manager-table
          caption="Accounts, read only"
          .columns=${accountColumns}
          .rows=${accountRows}
        ></wc-manager-table>
      `,
    },
    {
      // A phone's width: each row is a card with its column headings beside
      // the values and its buttons last, and nothing scrolls sideways. The
      // box stands in for the content area of a 390px window.
      name: 'stacked-at-phone-width',
      render: () => html`
        <div style="inline-size: 22rem">
          <wc-manager-table
            caption="Categorization rules"
            .columns=${ruleColumns}
            .rows=${ruleRows}
            .actions=${editDelete}
          ></wc-manager-table>
        </div>
      `,
    },
    {
      // Per-row actions and a badge, as the clients list has them, stacked.
      name: 'stacked-with-row-actions',
      render: () => html`
        <div style="inline-size: 22rem">
          <wc-manager-table
            caption="Clients"
            .columns=${[
              { key: 'name', label: 'Name' },
              { key: 'email', label: 'Email' },
              { key: 'billingAddress', label: 'Billing address' },
            ]}
            .rows=${[
              {
                id: 1,
                label: 'Cedar Systems',
                cells: ['Cedar Systems', 'ap@cedarsystems.test', '88 Cedar Way, Bend OR 97701'],
                actions: [
                  { name: 'invoices', label: 'Invoices' },
                  { name: 'edit', label: 'Edit', icon: 'wc-icon-edit' },
                  { name: 'archive', label: 'Archive' },
                  { name: 'delete', label: 'Delete', icon: 'wc-icon-trash', variant: 'danger' },
                ],
              },
              {
                id: 2,
                label: 'Globex',
                badge: 'Archived',
                cells: ['Globex', null, '1 Globex Plaza'],
                actions: [
                  { name: 'invoices', label: 'Invoices' },
                  { name: 'unarchive', label: 'Unarchive' },
                ],
              },
            ] satisfies ManagerRow[]}
          ></wc-manager-table>
        </div>
      `,
    },
    {
      // Inside a card of its own, the table drops its card so the two do not
      // nest: Documents' inactive kinds sit in a wc-panel.
      name: 'plain-inside-a-panel',
      render: () => html`
        <wc-panel heading="Inactive kinds">
          <wc-manager-table
            plain
            caption="Inactive document kinds"
            .columns=${[{ key: 'name', label: 'Name' }]}
            .rows=${[
              { id: 1, label: 'Statement of work', cells: ['Statement of work'] },
              { id: 2, label: 'Change order', cells: ['Change order'] },
            ]}
            .actions=${[{ name: 'reactivate', label: 'Reactivate' }]}
          ></wc-manager-table>
        </wc-panel>
      `,
    },
  ],
};

export default preview;
