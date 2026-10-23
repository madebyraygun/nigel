import { html } from 'lit';
import { ref } from 'lit/directives/ref.js';
import './wc-transaction-dialog.js';
import type { WcTransactionDialog } from './wc-transaction-dialog.js';
import type { CategoryOption } from './category-option.js';
import type { RegisterTableRow } from './wc-register-table.js';
import type { Preview } from '../../preview/types.js';

const categories: CategoryOption[] = [
  { id: 3, name: 'Consulting income', categoryType: 'income' },
  { id: 12, name: 'Software / Subscriptions', categoryType: 'expense' },
  { id: 14, name: 'Meals', categoryType: 'expense' },
  { id: 21, name: 'Bank fees', categoryType: 'expense' },
];

const categorized: RegisterTableRow = {
  id: 101,
  date: '2025-03-03',
  description: 'CEDAR SYSTEMS CLOUD HOSTING',
  amount: -54.99,
  category: 'Software / Subscriptions',
  categoryId: 12,
  vendor: 'Cedar Systems',
  accountName: 'Business Checking',
  isFlagged: false,
};

const uncategorized: RegisterTableRow = {
  id: 103,
  date: '2025-03-07',
  description:
    'POS PURCHASE JUNIPER LABS CAFE — CARD 0000, A DESCRIPTION LONG ENOUGH TO WRAP ACROSS LINES',
  amount: -18.5,
  category: null,
  categoryId: null,
  vendor: null,
  accountName: 'Business Credit Card',
  isFlagged: true,
};

/** A phone's dialog: the panel a 390px viewport leaves after the dialog's margins. */
const PHONE = '--nc-transaction-dialog-width: 358px';

/** The box after a Save that failed the check: text typed past the chosen category. */
const typedPastChoice = ref((el) => {
  if (!el) return;
  const dialog = el as WcTransactionDialog;
  void dialog.updateComplete.then(async () => {
    const picker = dialog.shadowRoot?.querySelector('wc-category-picker');
    if (!picker) return;
    await picker.updateComplete;
    picker.value = null;
    picker.queryText = 'Softwar';
  });
});

const preview: Preview = {
  id: 'wc-transaction-dialog',
  title: 'Transaction dialog',
  group: 'Overlays',
  description:
    'Editing a register row: the date, description, amount and account read-only for context, then the category and vendor. Enter saves, Esc cancels. Check light and dark with the harness toggle.',
  layout: 'stack',
  states: [
    {
      name: 'closed',
      render: () => html`<wc-transaction-dialog .row=${categorized}></wc-transaction-dialog>`,
    },
    {
      name: 'desktop',
      render: () => html`
        <wc-transaction-dialog
          open
          .row=${categorized}
          .categories=${categories}
        ></wc-transaction-dialog>
      `,
    },
    {
      name: 'phone',
      render: () => html`
        <wc-transaction-dialog
          open
          style=${PHONE}
          .row=${uncategorized}
          .categories=${categories}
        ></wc-transaction-dialog>
      `,
    },
    {
      name: 'single-account',
      render: () => html`
        <wc-transaction-dialog
          open
          .showAccount=${false}
          .row=${categorized}
          .categories=${categories}
        ></wc-transaction-dialog>
      `,
    },
    {
      name: 'validation-error',
      render: () => html`
        <wc-transaction-dialog
          open
          validation-error="Choose a category from the list."
          .row=${categorized}
          .categories=${categories}
          ${typedPastChoice}
        ></wc-transaction-dialog>
      `,
    },
    {
      name: 'validation-error-phone',
      render: () => html`
        <wc-transaction-dialog
          open
          style=${PHONE}
          validation-error="Choose a category from the list."
          .row=${categorized}
          .categories=${categories}
          ${typedPastChoice}
        ></wc-transaction-dialog>
      `,
    },
    {
      name: 'server-error',
      render: () => html`
        <wc-transaction-dialog
          open
          error="Category 99 does not exist."
          .row=${categorized}
          .categories=${categories}
        ></wc-transaction-dialog>
      `,
    },
    {
      name: 'saving',
      render: () => html`
        <wc-transaction-dialog
          open
          busy
          .row=${categorized}
          .categories=${categories}
        ></wc-transaction-dialog>
      `,
    },
  ],
};

export default preview;
