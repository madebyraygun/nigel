import { describe, it, expect, afterEach } from 'vitest';
import './wc-invoice-form.js';
import {
  EMPTY_INVOICE_FORM,
  addDays,
  dueTermFor,
  invoiceFormItems,
  netDueDate,
  prefilledTerms,
  validateInvoiceForm,
  withDueTerm,
  withIssueDate,
  type InvoiceClientOption,
  type InvoiceFormValue,
  type NcInvoiceFormChangeDetail,
  type WcInvoiceForm,
} from './wc-invoice-form.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-invoice-form.preview.js';

const CLIENTS: InvoiceClientOption[] = [
  { id: 1, name: 'Acme Co', email: 'ap@acme.test' },
  { id: 2, name: 'Globex', email: null },
];

const valid: InvoiceFormValue = {
  clientId: '1',
  issueDate: '2026-08-07',
  dueDate: '2026-09-06',
  dueTerm: 'net30',
  currency: 'USD',
  notes: '',
  terms: 'Net 30',
  items: [{ description: 'Consulting', quantity: '10', unitAmount: '150' }],
};

async function mount(props: Partial<WcInvoiceForm> = {}): Promise<WcInvoiceForm> {
  const el = document.createElement('wc-invoice-form');
  Object.assign(el, { value: valid, clients: CLIENTS }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('invoiceFormItems', () => {
  it('drops the rows nobody typed into', () => {
    const items = invoiceFormItems({
      ...valid,
      items: [
        valid.items[0],
        { description: '', quantity: '1', unitAmount: '' },
        { description: 'Hosting', quantity: '1', unitAmount: '350' },
      ],
    });
    expect(items.map((item) => item.description)).toEqual(['Consulting', 'Hosting']);
  });
});

describe('validateInvoiceForm', () => {
  it('accepts a well-formed invoice', () => {
    expect(validateInvoiceForm(valid)).toEqual({});
  });

  it('requires a client', () => {
    expect(validateInvoiceForm({ ...valid, clientId: '' }).clientId).toBe(
      'Choose a client',
    );
  });

  it('holds dates to the API shape, not the CLI one', () => {
    // `validate_date` accepts `2026-4-1`; the route does not, and a form that
    // accepted it would send a request it knows will be a 400.
    expect(validateInvoiceForm({ ...valid, issueDate: '2026-8-7' }).issueDate).toBe(
      'Issue date must be YYYY-MM-DD',
    );
    expect(validateInvoiceForm({ ...valid, issueDate: '' }).issueDate).toContain(
      'required',
    );
  });

  it('treats an empty due date as no due date rather than an error', () => {
    expect(validateInvoiceForm({ ...valid, dueDate: '' }).dueDate).toBeUndefined();
    expect(validateInvoiceForm({ ...valid, dueDate: '2026-9-6' }).dueDate).toBe(
      'Due date must be YYYY-MM-DD',
    );
  });

  it('requires a three-letter currency code', () => {
    expect(validateInvoiceForm({ ...valid, currency: 'DOLLARS' }).currency).toContain(
      'three-letter',
    );
  });

  it('refuses an invoice with no lines at all', () => {
    expect(validateInvoiceForm({ ...valid, items: [] }).items).toContain(
      'at least one line item',
    );
    expect(
      validateInvoiceForm({
        ...valid,
        items: [{ description: '', quantity: '1', unitAmount: '' }],
      }).items,
    ).toContain('at least one line item');
  });

  it('reports per-row problems in row order', () => {
    const errors = validateInvoiceForm({
      ...valid,
      items: [
        { description: '', quantity: 'lots', unitAmount: '150' },
        valid.items[0],
      ],
    });
    expect(errors.itemErrors?.[0]).toEqual({
      description: 'Description is required',
      quantity: 'Quantity must be a number',
    });
    expect(errors.itemErrors?.[1]).toEqual({});
  });

  it('refuses a line that overflows and a total of zero', () => {
    // `validate_items` checks the arithmetic's result, not its inputs: two
    // huge finite figures multiply to infinity, which serde renders as null.
    expect(
      validateInvoiceForm({
        ...valid,
        items: [{ description: 'Big', quantity: '1e308', unitAmount: '1e308' }],
      }).itemErrors?.[0].unitAmount,
    ).toContain('too large');

    expect(
      validateInvoiceForm({
        ...valid,
        items: [{ description: 'Free', quantity: '0', unitAmount: '150' }],
      }).items,
    ).toContain('more than zero');
  });
});

describe('due dates counted from the issue date', () => {
  it('pads every computed date, across a month and a year boundary', () => {
    expect(netDueDate('2026-02-27', 'net7')).toBe('2026-03-06');
    expect(netDueDate('2026-12-20', 'net14')).toBe('2027-01-03');
    expect(netDueDate('2028-02-01', 'net30')).toBe('2028-03-02');
    expect(addDays('2026-08-07', 30)).toBe('2026-09-06');
  });

  it('counts in whole days regardless of the reader’s daylight saving', () => {
    // Local-time arithmetic across a spring-forward lands an hour short and
    // rounds down to the previous day; UTC cannot.
    expect(netDueDate('2026-03-05', 'net7')).toBe('2026-03-12');
    expect(netDueDate('2026-10-25', 'net7')).toBe('2026-11-01');
  });

  it('says nothing rather than guessing when the issue date is not a day', () => {
    expect(netDueDate('', 'net30')).toBe('');
    expect(netDueDate('2026-8-7', 'net30')).toBe('');
    expect(netDueDate('2026-02-30', 'net30')).toBe('');
    expect(addDays('9999-12-31', 30)).toBe('');
  });

  it('moves a preset-derived due date when the issue date moves', () => {
    const moved = withIssueDate(valid, '2026-08-10');
    expect(moved).toMatchObject({ issueDate: '2026-08-10', dueDate: '2026-09-09' });
  });

  it('leaves a hand-picked due date, and an absent one, where they are', () => {
    expect(
      withIssueDate({ ...valid, dueTerm: 'custom' }, '2026-08-10').dueDate,
    ).toBe('2026-09-06');
    expect(
      withIssueDate({ ...valid, dueTerm: 'none', dueDate: '' }, '2026-08-10').dueDate,
    ).toBe('');
  });

  it('clears a preset due date the issue date can no longer support', () => {
    expect(withIssueDate(valid, '').dueDate).toBe('');
  });

  it('computes, keeps or clears the date as the choice demands', () => {
    const base: InvoiceFormValue = { ...valid, dueTerm: 'none', dueDate: '', terms: '' };

    expect(withDueTerm(base, 'net7').dueDate).toBe('2026-08-14');
    // Custom starts from whatever is showing, so reaching for the calendar
    // after a preset does not blank the field first.
    expect(withDueTerm(withDueTerm(base, 'net7'), 'custom').dueDate).toBe('2026-08-14');
    expect(withDueTerm(valid, 'none').dueDate).toBe('');
  });

  it('reads an existing invoice’s dates back as the choice that made them', () => {
    expect(dueTermFor('2026-08-07', '')).toBe('none');
    expect(dueTermFor('2026-08-07', '2026-08-14')).toBe('net7');
    expect(dueTermFor('2026-08-07', '2026-08-21')).toBe('net14');
    expect(dueTermFor('2026-08-07', '2026-09-06')).toBe('net30');
    expect(dueTermFor('2026-08-07', '2026-08-31')).toBe('custom');
    expect(dueTermFor('', '2026-08-31')).toBe('custom');
  });
});

describe('the terms a net preset writes', () => {
  it('fills an empty terms field so the page prints what the form chose', () => {
    expect(withDueTerm({ ...valid, terms: '' }, 'net30').terms).toBe('Net 30');
  });

  it('leaves a sentence somebody typed', () => {
    const typed = 'Payable on receipt; late after 15 days.';
    expect(withDueTerm({ ...valid, terms: typed }, 'net7').terms).toBe(typed);
    expect(prefilledTerms(typed, 'none')).toBe(typed);
  });

  it('rewrites a label it wrote itself, and clears it for a date or none', () => {
    expect(withDueTerm(valid, 'net7').terms).toBe('Net 7');
    expect(withDueTerm(valid, 'none').terms).toBe('');
    expect(withDueTerm(valid, 'custom').terms).toBe('');
  });
});

describe('wc-invoice-form', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('collects every field the create route takes', async () => {
    const el = await mount({ value: { ...EMPTY_INVOICE_FORM, dueTerm: 'custom' } });
    for (const hook of [
      '[data-client]',
      '[data-issue]',
      '[data-due-term]',
      '[data-due]',
      '[data-currency]',
      '[data-items]',
      '[data-notes]',
      '[data-terms]',
    ]) {
      expect(el.shadowRoot?.querySelector(hook), hook).toBeTruthy();
    }
  });

  it('emits the whole value on every edit', async () => {
    const el = await mount({ value: { ...valid, dueTerm: 'custom' } });
    const seen: InvoiceFormValue[] = [];
    el.addEventListener('nc-invoice-form-change', (event) =>
      seen.push((event as CustomEvent<NcInvoiceFormChangeDetail>).detail.value),
    );

    const input = el.shadowRoot?.querySelector<HTMLInputElement>('[data-due]');
    input!.value = '2026-10-01';
    input!.dispatchEvent(new Event('input'));

    expect(seen).toEqual([{ ...valid, dueTerm: 'custom', dueDate: '2026-10-01' }]);
  });

  it('uses date inputs, which jsdom and every browser but Safari implement', async () => {
    const el = await mount({ value: { ...valid, dueTerm: 'custom' } });
    expect(el.shadowRoot?.querySelector('[data-issue]')?.getAttribute('type')).toBe(
      'date',
    );
    expect(el.shadowRoot?.querySelector('[data-due]')?.getAttribute('type')).toBe('date');
  });

  it('takes a date typed into a control a browser degraded to text', async () => {
    // Safari has no date picker: the control is a text box, and what it
    // reports is whatever was typed. The shape check is what refuses the rest.
    const el = await mount({ value: { ...valid, dueTerm: 'custom' } });
    const seen: InvoiceFormValue[] = [];
    el.addEventListener('nc-invoice-form-change', (event) =>
      seen.push((event as CustomEvent<NcInvoiceFormChangeDetail>).detail.value),
    );

    const issue = el.shadowRoot?.querySelector<HTMLInputElement>('[data-issue]');
    issue!.value = '2026-09-01';
    issue!.dispatchEvent(new Event('input'));
    expect(seen.at(-1)?.issueDate).toBe('2026-09-01');
    expect(validateInvoiceForm(seen.at(-1)!).issueDate).toBeUndefined();

    issue!.value = '2026-9-1';
    issue!.dispatchEvent(new Event('input'));
    expect(seen.at(-1)?.issueDate).toBe('2026-9-1');
    expect(validateInvoiceForm(seen.at(-1)!).issueDate).toBe(
      'Issue date must be YYYY-MM-DD',
    );
  });

  it('moves a preset due date when the issue date is edited', async () => {
    const el = await mount();
    const seen: InvoiceFormValue[] = [];
    el.addEventListener('nc-invoice-form-change', (event) =>
      seen.push((event as CustomEvent<NcInvoiceFormChangeDetail>).detail.value),
    );

    const issue = el.shadowRoot?.querySelector<HTMLInputElement>('[data-issue]');
    issue!.value = '2026-08-10';
    issue!.dispatchEvent(new Event('input'));

    expect(seen.at(-1)).toMatchObject({
      issueDate: '2026-08-10',
      dueDate: '2026-09-09',
    });
  });

  it('offers the four terms and a custom date, and shows the picker for that one', async () => {
    const el = await mount({ value: { ...valid, dueTerm: 'none', dueDate: '' } });
    const options = [
      ...(el.shadowRoot?.querySelectorAll('[data-due-term] wa-option') ?? []),
    ];
    expect(options.map((option) => option.getAttribute('value'))).toEqual([
      'none',
      'net7',
      'net14',
      'net30',
      'custom',
    ]);
    expect(el.shadowRoot?.querySelector('[data-due]')).toBeNull();

    const select = el.shadowRoot?.querySelector<HTMLSelectElement>('[data-due-term]');
    const seen: InvoiceFormValue[] = [];
    el.addEventListener('nc-invoice-form-change', (event) =>
      seen.push((event as CustomEvent<NcInvoiceFormChangeDetail>).detail.value),
    );
    select!.value = 'net14';
    select!.dispatchEvent(new Event('change'));

    expect(seen.at(-1)).toMatchObject({ dueTerm: 'net14', dueDate: '2026-08-21' });
  });

  it('says which day a preset lands on', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('[data-due-hint]')?.textContent).toContain(
      '2026-09-06',
    );

    const empty = await mount({ value: { ...valid, dueTerm: 'none', dueDate: '' } });
    expect(empty.shadowRoot?.querySelector('[data-due-hint]')?.textContent).toContain(
      'never goes overdue',
    );
  });

  it('carries a line-item edit up as the whole value', async () => {
    const el = await mount();
    const seen: InvoiceFormValue[] = [];
    el.addEventListener('nc-invoice-form-change', (event) =>
      seen.push((event as CustomEvent<NcInvoiceFormChangeDetail>).detail.value),
    );

    el.shadowRoot
      ?.querySelector('[data-items]')
      ?.shadowRoot?.querySelector<HTMLElement>('[data-add-row]')
      ?.click();

    expect(seen[0].items).toHaveLength(2);
  });

  it('warns when the chosen client has no email', async () => {
    const el = await mount({ value: { ...valid, clientId: '2' } });
    expect(el.shadowRoot?.querySelector('[data-no-email]')?.textContent).toContain(
      'cannot be sent',
    );
  });

  it('locks the client on an edit — an invoice stays with the client it was raised for', async () => {
    const el = await mount({ mode: 'edit' });
    expect(el.shadowRoot?.querySelector('[data-client]')?.hasAttribute('disabled')).toBe(
      true,
    );
  });

  it('renders the list-level refusal on the line items rather than beside a field', async () => {
    const el = await mount({
      value: { ...valid, items: [] },
      errors: { items: 'An invoice needs at least one line item.' },
    });
    expect(el.shadowRoot?.querySelector('[data-items]')?.getAttribute('list-error')).toBe(
      'An invoice needs at least one line item.',
    );
  });

  it('disables every control while a save is in flight', async () => {
    const el = await mount({ disabled: true, value: { ...valid, dueTerm: 'custom' } });
    const controls = [
      ...(el.shadowRoot?.querySelectorAll(
        '[data-client],[data-issue],[data-due-term],[data-due],[data-currency],[data-notes],[data-terms]',
      ) ?? []),
    ];
    expect(controls).toHaveLength(7);
    expect(controls.every((control) => control.hasAttribute('disabled'))).toBe(true);
  });
});

describePreviewA11y(preview);
