import { html } from 'lit';
import './wc-client-form.js';
import { EMPTY_CLIENT_FORM } from './wc-client-form.js';
import type { ClientFormValue } from './wc-client-form.js';
import type { Preview } from '../../preview/types.js';

const one: ClientFormValue = {
  name: 'Acme Co',
  contacts: [{ email: 'ap@acme.test', name: 'Ada Payne', title: 'AP Manager' }],
  billingIndex: 0,
  billingAddress: '1 Main St, Springfield',
  notes: 'Net 30, PO required',
};

const several: ClientFormValue = {
  ...one,
  contacts: [
    { email: 'ap@acme.test', name: 'Ada Payne', title: 'AP Manager' },
    { email: 'dana@acme.test', name: 'Dana Chen', title: 'Design Lead' },
    { email: 'sam@acme.test', name: '', title: '' },
  ],
};

const preview: Preview = {
  id: 'wc-client-form',
  title: 'Client form',
  group: 'Invoicing',
  description:
    'Name, contacts, billing address and notes. Exactly one contact is the billing recipient — the invoice\'s To — and the rest are copied. An address is not shape-checked, because `nigel client add` does not check it either.',
  layout: 'stack',
  states: [
    {
      name: 'add — no contacts yet',
      render: () => html`<wc-client-form .value=${EMPTY_CLIENT_FORM}></wc-client-form>`,
    },
    {
      name: 'one contact',
      render: () => html`<wc-client-form .value=${one}></wc-client-form>`,
    },
    {
      name: 'several contacts',
      render: () => html`<wc-client-form .value=${several}></wc-client-form>`,
    },
    {
      name: 'a long name',
      render: () => html`
        <wc-client-form
          .value=${{
            ...one,
            contacts: [
              {
                email: 'accounts.payable.department@northwind-traders.example',
                name: 'Alexandra Beauregard-Fitzwilliam',
                title: 'Deputy Head of Accounts Payable',
              },
            ],
          }}
        ></wc-client-form>
      `,
    },
    {
      name: 'a refused row',
      render: () => html`
        <wc-client-form
          .value=${{ ...one, contacts: [...one.contacts, { email: '', name: '', title: '' }] }}
          .errors=${{ contacts: { 1: 'An email address is required' } }}
        ></wc-client-form>
      `,
    },
    {
      name: 'name required',
      render: () => html`
        <wc-client-form
          .value=${{ ...one, name: '' }}
          .errors=${{ name: 'Name is required' }}
        ></wc-client-form>
      `,
    },
    {
      name: 'saving',
      render: () => html`<wc-client-form .value=${several} disabled></wc-client-form>`,
    },
  ],
};

export default preview;
