import { describe, it, expect, afterEach } from 'vitest';
import './wc-invoice-table.js';
import type { InvoiceTableRow, WcInvoiceTable } from './wc-invoice-table.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-invoice-table.preview.js';

const ROWS: InvoiceTableRow[] = [
  {
    number: 1250,
    status: 'partial',
    clientName: 'Acme Co',
    total: 3200,
    balance: 1200,
    issueDate: '2026-07-21',
    dueDate: '2026-08-20',
    href: '#/invoices/1250',
  },
  {
    number: 1247,
    status: 'void',
    clientName: null,
    total: 500,
    balance: null,
    dueDate: null,
  },
];

async function mount(props: Partial<WcInvoiceTable> = {}): Promise<WcInvoiceTable> {
  const el = document.createElement('wc-invoice-table');
  Object.assign(el, { rows: ROWS }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-invoice-table', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders one row per invoice, keyed by number', async () => {
    const el = await mount();
    const numbers = [...(el.shadowRoot?.querySelectorAll('tr[data-row]') ?? [])].map(
      (tr) => tr.getAttribute('data-row'),
    );
    expect(numbers).toEqual(['1250', '1247']);
  });

  it('shows an em dash for a void balance rather than $0.00', async () => {
    // A void invoice owes nothing and never will; `$0.00` reads as settled,
    // which is a different thing. The `wc-import-history` null precedent.
    const el = await mount();
    const cells = [...(el.shadowRoot?.querySelectorAll('[data-balance]') ?? [])];
    expect(cells[0].querySelector('wc-money')).toBeTruthy();
    expect(cells[1].querySelector('wc-money')).toBeNull();
    expect(cells[1].textContent?.trim()).toBe('—');
  });

  it('shows an em dash for a missing client and says a dateless invoice is due on receipt', async () => {
    const el = await mount();
    const row = el.shadowRoot?.querySelector('tr[data-row="1247"]');
    const cells = [...(row?.querySelectorAll('td') ?? [])].map((td) =>
      td.textContent?.trim(),
    );
    expect(cells[1]).toBe('—');
    expect(cells[3]).toBe('On receipt');
  });

  it('prints the number, the client and both dates in the handoff’s order', async () => {
    const el = await mount();
    const row = el.shadowRoot?.querySelector('tr[data-row="1250"]');
    const cells = [...(row?.querySelectorAll('td') ?? [])].map((td) =>
      td.textContent?.trim(),
    );
    expect(cells.slice(0, 4)).toEqual(['#1250', 'Acme Co', 'Jul 21, 2026', 'Aug 20, 2026']);
    const headers = [...(el.shadowRoot?.querySelectorAll('th') ?? [])].map((th) =>
      th.textContent?.trim(),
    );
    expect(headers).toEqual([
      'Number',
      'Client',
      'Issued',
      'Due',
      'Status',
      'Total',
      'Balance',
      'Open',
    ]);
  });

  it('marks an overdue invoice’s due date and strikes a void invoice’s total', async () => {
    const el = await mount({
      rows: [{ ...ROWS[0], status: 'overdue' }, ROWS[1]],
    });
    expect(
      el.shadowRoot?.querySelector('tr[data-row="1250"] [data-due]')?.hasAttribute('data-overdue'),
    ).toBe(true);
    expect(el.shadowRoot?.querySelector('tr[data-row="1247"] [data-void]')).toBeTruthy();
    expect(el.shadowRoot?.querySelector('tr[data-row="1250"] [data-void]')).toBeNull();
  });

  it('follows the row’s link when the row itself is clicked', async () => {
    const el = await mount();
    const link = el.shadowRoot?.querySelector<HTMLAnchorElement>('tr[data-row="1250"] a');
    let followed = 0;
    link?.addEventListener('click', (event) => {
      event.preventDefault();
      followed += 1;
    });
    el.shadowRoot
      ?.querySelector<HTMLElement>('tr[data-row="1250"] [data-due]')
      ?.dispatchEvent(new MouseEvent('click', { bubbles: true, composed: true }));
    expect(followed).toBe(1);
  });

  it('links a row only when given an address', async () => {
    const el = await mount();
    const links = [...(el.shadowRoot?.querySelectorAll('a') ?? [])].map((a) =>
      a.getAttribute('href'),
    );
    expect(links).toEqual(['#/invoices/1250']);
  });

  it('carries the status word into a status chip', async () => {
    const el = await mount();
    const chips = [...(el.shadowRoot?.querySelectorAll('wc-invoice-status') ?? [])].map(
      (chip) => chip.getAttribute('status'),
    );
    expect(chips).toEqual(['partial', 'void']);
  });

  it('says it is loading before it says it is empty', async () => {
    const el = await mount({ rows: [], loading: true });
    expect(el.shadowRoot?.querySelector('wc-spinner')).toBeTruthy();
    expect(el.shadowRoot?.querySelector('[data-empty]')).toBeNull();
  });

  it('renders the empty message it was given', async () => {
    const el = await mount({ rows: [], emptyMessage: 'No invoices yet.' });
    expect(el.shadowRoot?.querySelector('[data-empty]')?.textContent).toContain(
      'No invoices yet.',
    );
  });

  it('renders each row in its own currency', async () => {
    // A list spans clients and need not be in one currency, so the symbol is
    // the row's, not the table's.
    const el = await mount({
      rows: [
        { ...ROWS[0], currency: 'EUR' },
        { ...ROWS[0], number: 1251, currency: 'USD' },
      ],
    });
    const currencies = [...(el.shadowRoot?.querySelectorAll('wc-money') ?? [])].map(
      (money) => (money as HTMLElement & { currency: string }).currency,
    );
    // total + balance for each of the two rows.
    expect(currencies).toEqual(['EUR', 'EUR', 'USD', 'USD']);
  });
});

describePreviewA11y(preview);
