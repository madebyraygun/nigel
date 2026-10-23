import { describe, it, expect, afterEach, beforeAll, afterAll } from 'vitest';
import './wc-transaction-dialog.js';
import { CHOOSE_CATEGORY_MESSAGE, WcTransactionDialog } from './wc-transaction-dialog.js';
import type { CategoryOption } from './category-option.js';
import type { NcEditCommitDetail, RegisterTableRow } from './wc-register-table.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-transaction-dialog.preview.js';

const categories: CategoryOption[] = [
  { id: 3, name: 'Consulting income', categoryType: 'income' },
  { id: 12, name: 'Software / Subscriptions', categoryType: 'expense' },
  { id: 14, name: 'Meals', categoryType: 'expense' },
];

const row: RegisterTableRow = {
  id: 7,
  date: '2025-03-03',
  description: 'ACME SUPPLY CO',
  amount: -42.5,
  category: 'Software / Subscriptions',
  categoryId: 12,
  vendor: 'Acme',
  accountName: 'Business Checking',
  isFlagged: false,
};

async function mount(props: Partial<WcTransactionDialog> = {}): Promise<WcTransactionDialog> {
  const el = document.createElement('wc-transaction-dialog');
  Object.assign(el, { open: true, row, categories }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  await el.shadowRoot?.querySelector('wc-category-picker')?.updateComplete;
  return el;
}

function picker(el: WcTransactionDialog) {
  return el.shadowRoot!.querySelector('wc-category-picker')!;
}

function vendorInput(el: WcTransactionDialog) {
  return el.shadowRoot!.querySelector('wa-input') as HTMLElement & { value: string };
}

function commits(el: WcTransactionDialog): NcEditCommitDetail[] {
  const seen: NcEditCommitDetail[] = [];
  el.addEventListener('nc-edit-commit', (event) => seen.push(event.detail));
  return seen;
}

async function typeCategory(el: WcTransactionDialog, text: string): Promise<void> {
  const input = picker(el).shadowRoot!.querySelector('input')!;
  input.value = text;
  input.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
  await picker(el).updateComplete;
  await el.updateComplete;
}

function press(target: Element, key: string): void {
  target.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, composed: true }));
}

describe('wc-transaction-dialog', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('shows the date, description, amount and account as read-only text', async () => {
    const el = await mount();
    const summary = el.shadowRoot!.querySelector('dl.summary')!;
    const text = summary.textContent ?? '';
    expect(text).toContain('2025-03-03');
    expect(text).toContain('ACME SUPPLY CO');
    expect(text).toContain('Business Checking');
    expect(summary.querySelector('wc-money')?.amount).toBe(-42.5);
    expect(summary.querySelector('input, wa-input')).toBeNull();
  });

  it('leaves the account out when the register is filtered to one', async () => {
    const el = await mount({ showAccount: false });
    expect(el.shadowRoot!.querySelector('dl.summary')?.textContent).not.toContain('Account');
  });

  it('starts the fields from the row', async () => {
    const el = await mount();
    expect(picker(el).queryText).toBe('Software / Subscriptions');
    expect(vendorInput(el).value).toBe('Acme');
  });

  it('saves the row unchanged as the same ids, trimmed vendor', async () => {
    const el = await mount();
    const seen = commits(el);
    el.shadowRoot!.querySelector<HTMLElement>('[data-save]')!.click();
    expect(seen).toEqual([{ id: 7, categoryId: 12, vendor: 'Acme' }]);
  });

  it('sends a chosen category and an edited vendor', async () => {
    const el = await mount();
    const seen = commits(el);
    picker(el).dispatchEvent(
      new CustomEvent('nc-category-change', {
        detail: { categoryId: 14, name: 'Meals' },
        bubbles: true,
        composed: true,
      }),
    );
    const vendor = vendorInput(el);
    vendor.value = '  Globex  ';
    vendor.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
    await el.updateComplete;

    el.shadowRoot!.querySelector<HTMLElement>('[data-save]')!.click();
    expect(seen).toEqual([{ id: 7, categoryId: 14, vendor: 'Globex' }]);
  });

  it('sends an emptied vendor as null', async () => {
    const el = await mount();
    const seen = commits(el);
    const vendor = vendorInput(el);
    vendor.value = '   ';
    vendor.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
    await el.updateComplete;
    el.shadowRoot!.querySelector<HTMLElement>('[data-save]')!.click();
    expect(seen[0]?.vendor).toBeNull();
  });

  it('refuses a category box that names no category', async () => {
    const el = await mount();
    const seen = commits(el);
    await typeCategory(el, 'Softwar');

    el.shadowRoot!.querySelector<HTMLElement>('[data-save]')!.click();
    await el.updateComplete;

    expect(seen).toEqual([]);
    expect(el.shadowRoot!.querySelector('.error[role="alert"]')?.textContent).toContain(
      CHOOSE_CATEGORY_MESSAGE,
    );
    expect(picker(el).invalid).toBe(true);
  });

  it('lets an uncategorized row save with the category left empty', async () => {
    const el = await mount({ row: { ...row, category: null, categoryId: null } });
    const seen = commits(el);
    el.shadowRoot!.querySelector<HTMLElement>('[data-save]')!.click();
    expect(seen).toEqual([{ id: 7, categoryId: null, vendor: 'Acme' }]);
  });

  it('saves on Enter from the vendor field', async () => {
    const el = await mount();
    const seen = commits(el);
    press(vendorInput(el), 'Enter');
    expect(seen).toHaveLength(1);
  });

  it('leaves Enter to the category list while it is open', async () => {
    const el = await mount();
    const seen = commits(el);
    await typeCategory(el, 'Meal');
    press(picker(el).shadowRoot!.querySelector('input')!, 'Enter');
    await el.updateComplete;
    expect(seen).toEqual([]);
    expect(picker(el).value).toBe(14);
  });

  it('cancels from the Cancel button and from the dialog dismissing itself', async () => {
    const el = await mount();
    const cancelled: number[] = [];
    el.addEventListener('nc-edit-cancel', (event) => cancelled.push(event.detail.id));

    el.shadowRoot!.querySelector<HTMLElement>('[data-cancel]')!.click();
    el.shadowRoot!.querySelector('wa-dialog')!.dispatchEvent(new CustomEvent('wa-hide'));
    expect(cancelled).toEqual([7, 7]);
  });

  it('will not close or save twice while a save is in flight', async () => {
    const el = await mount({ busy: true });
    const seen = commits(el);
    let cancelled = 0;
    el.addEventListener('nc-edit-cancel', () => (cancelled += 1));

    const save = el.shadowRoot!.querySelector<HTMLElement>('[data-save]')!;
    expect(save.hasAttribute('disabled')).toBe(true);
    expect(save.textContent?.trim()).toBe('Saving…');
    save.click();

    const hide = new CustomEvent('wa-hide', { cancelable: true });
    el.shadowRoot!.querySelector('wa-dialog')!.dispatchEvent(hide);
    expect(hide.defaultPrevented).toBe(true);
    expect(seen).toEqual([]);
    expect(cancelled).toBe(0);
  });

  it("shows the server's refusal in an alert", async () => {
    const el = await mount({ error: 'Category 99 does not exist.' });
    expect(el.shadowRoot!.querySelector('.error[role="alert"]')?.textContent).toContain(
      'does not exist',
    );
  });

  it('starts again from the row when reopened after a cancel', async () => {
    const el = await mount();
    await typeCategory(el, 'Softwar');
    el.shadowRoot!.querySelector<HTMLElement>('[data-save]')!.click();

    el.open = false;
    await el.updateComplete;
    el.open = true;
    await el.updateComplete;
    await picker(el).updateComplete;

    expect(picker(el).queryText).toBe('Software / Subscriptions');
    expect(el.validationError).toBe('');
  });
});

describePreviewA11y(preview);

describe('in dark mode', () => {
  beforeAll(() => document.documentElement.classList.add('dark-mode'));
  afterAll(() => document.documentElement.classList.remove('dark-mode'));
  describePreviewA11y(preview);
});
