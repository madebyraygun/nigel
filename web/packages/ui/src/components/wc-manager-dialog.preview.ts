import { html } from 'lit';
import './wc-manager-dialog.js';
import './wc-account-form.js';
import './wc-client-form.js';
import './wc-category-form.js';
import './wc-rule-form.js';
import './wc-rule-test-preview.js';
import './wc-manager-table.js';
import type { Preview } from '../../preview/types.js';
import type { ManagerAction, ManagerColumn, ManagerRow } from './wc-manager-table.js';
import type { ClientFormValue } from './wc-client-form.js';
import type { CategoryOption } from './category-option.js';

const ruleCategories: CategoryOption[] = [
  { id: 3, name: 'Consulting income', categoryType: 'income' },
  { id: 12, name: 'Software / Subscriptions', categoryType: 'expense' },
];

/** A client with three contacts: the widest form any manager dialog holds. */
const severalContacts: ClientFormValue = {
  name: 'Juniper Labs',
  contacts: [
    { email: 'accounts-payable@juniperlabs.test', name: 'Avery Juniper', title: 'Controller' },
    { email: 'ops@juniperlabs.test', name: 'Rowan Field', title: 'Operations lead' },
    { email: 'cfo@juniperlabs.test', name: 'Sam Okafor', title: 'Chief financial officer' },
  ],
  billingIndex: 0,
  billingAddress: '45 Juniper Rd, Eugene OR 97401',
  notes: '',
};

const value = {
  name: 'Chase Business',
  accountType: 'checking',
  class: 'asset',
  institution: 'Chase',
  lastFour: '9921',
};

const clientColumns: ManagerColumn[] = [
  { key: 'name', label: 'Name' },
  { key: 'email', label: 'Email' },
  { key: 'billingAddress', label: 'Billing address' },
];

const clientRows: ManagerRow[] = [
  {
    id: 1,
    label: 'Acme Co',
    cells: ['Acme Co', 'ap@acme.test', '1 Main St, Springfield'],
  },
  {
    id: 2,
    label: 'Blackwood Partners',
    cells: ['Blackwood Partners', 'billing@blackwood.test', '400 Harbor Way, Oakland'],
  },
  { id: 3, label: 'Corvid Labs', cells: ['Corvid Labs', null, '77 Rookery Ln, Portland'] },
  {
    id: 4,
    label: 'Delta Provisioning',
    cells: ['Delta Provisioning', 'invoices@delta.test', '9 Front St, Seattle'],
  },
  {
    id: 5,
    label: 'Eastgate Holdings',
    cells: ['Eastgate Holdings', 'ap@eastgate.test', '1200 Bell Ave, Denver'],
  },
  {
    id: 6,
    label: 'Foxglove Studio',
    cells: ['Foxglove Studio', 'hello@foxglove.test', '3 Kiln Court, Austin'],
  },
];

const editDelete: ManagerAction[] = [
  { name: 'edit', label: 'Edit', icon: 'wc-icon-edit' },
  { name: 'delete', label: 'Delete', icon: 'wc-icon-trash', variant: 'danger' },
];

const preview: Preview = {
  id: 'wc-manager-dialog',
  title: 'Manager dialog',
  group: 'Overlays',
  description:
    'The frame around a manager form. A save that comes back 409 renders here, beside the field that caused it, rather than in a toast that leaves before it is read.',
  layout: 'stack',
  states: [
    {
      name: 'closed',
      render: () => html`<wc-manager-dialog heading="Add account"></wc-manager-dialog>`,
    },
    {
      name: 'default',
      render: () => html`
        <wc-manager-dialog open heading="Add account" confirm-label="Add">
          <wc-account-form .value=${value}></wc-account-form>
        </wc-manager-dialog>
      `,
    },
    {
      name: 'edit',
      render: () => html`
        <wc-manager-dialog open heading="Rename account">
          <wc-account-form mode="edit" .value=${value}></wc-account-form>
        </wc-manager-dialog>
      `,
    },
    {
      name: 'with-error',
      render: () => html`
        <wc-manager-dialog
          open
          heading="Add account"
          confirm-label="Add"
          error="An account named “Chase Business” already exists."
        >
          <wc-account-form .value=${value}></wc-account-form>
        </wc-manager-dialog>
      `,
    },
    {
      name: 'busy',
      render: () => html`
        <wc-manager-dialog open busy heading="Add account" confirm-label="Add">
          <wc-account-form .value=${value} disabled></wc-account-form>
        </wc-manager-dialog>
      `,
    },
    {
      name: 'rule-form',
      render: () => html`
        <wc-manager-dialog open heading="Add rule" confirm-label="Add rule">
          <wc-rule-form
            .value=${{
              pattern: 'GITHUB',
              matchType: 'contains',
              categoryId: 12,
              vendor: 'GitHub',
              priority: 10,
            }}
            .categories=${ruleCategories}
          >
            <wc-rule-test-preview slot="test"></wc-rule-test-preview>
          </wc-rule-form>
        </wc-manager-dialog>
      `,
    },
    {
      name: 'category-form',
      render: () => html`
        <wc-manager-dialog open heading="Add category" confirm-label="Add category">
          <wc-category-form
            .value=${{
              name: 'Software / Subscriptions',
              categoryType: 'expense',
              class: 'expense',
              taxLine: 'Other expenses',
              formLine: '1120S-19',
            }}
          ></wc-category-form>
        </wc-manager-dialog>
      `,
    },
    {
      // The widest form a manager dialog holds. Each contact's fields sit side
      // by side while they fit and stack when they do not; nothing scrolls
      // sideways.
      name: 'client-with-several-contacts',
      render: () => html`
        <wc-manager-dialog open heading="Edit client">
          <wc-client-form .value=${severalContacts}></wc-client-form>
        </wc-manager-dialog>
      `,
    },
    {
      name: 'over-a-populated-list',
      // The bug as reported: the clients table reading straight through the
      // dialog's header, body and footer. The panel has to be opaque here.
      render: () => html`
        <wc-manager-table
          .columns=${clientColumns}
          .rows=${clientRows}
          .actions=${editDelete}
        ></wc-manager-table>
        <wc-manager-dialog open heading="Edit client">
          <wc-client-form
            .value=${{
              name: 'Acme Co',
              contacts: [{ email: 'ap@acme.test', name: 'Ada Payne', title: 'AP Manager' }],
              billingIndex: 0,
              billingAddress: '1 Main St, Springfield',
              notes: 'Net 30, PO required',
            } satisfies ClientFormValue}
          ></wc-client-form>
        </wc-manager-dialog>
      `,
    },
  ],
};

export default preview;
