import { describe, it, expect, afterEach } from 'vitest';
import './wc-invoice-summary.js';
import type { WcInvoiceSummary } from './wc-invoice-summary.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-invoice-summary.preview.js';

async function mount(props: Partial<WcInvoiceSummary> = {}): Promise<WcInvoiceSummary> {
  const el = document.createElement('wc-invoice-summary');
  Object.assign(
    el,
    {
      number: 1250,
      status: 'partial',
      clientName: 'Acme Co',
      total: 3200,
      balance: 1200,
      issueDate: '2026-02-20',
      dueDate: '2026-03-20',
    },
    props,
  );
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function amountOf(el: WcInvoiceSummary, hook: string): number | undefined {
  const money = el.shadowRoot?.querySelector(hook) as
    | (HTMLElement & { amount: number })
    | null;
  return money?.amount;
}

describe('wc-invoice-summary', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('leads with the number, the status and the client', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('h1')?.textContent).toBe('Invoice #1250');
    expect(el.shadowRoot?.querySelector('wc-invoice-status')?.getAttribute('status')).toBe(
      'partial',
    );
    expect(el.shadowRoot?.querySelector('[data-client]')?.textContent?.trim()).toBe('Acme Co');
  });

  it('prints the client’s email under its name, and the dates as words', async () => {
    const el = await mount({ clientEmail: 'billing@acme.test' });
    expect(el.shadowRoot?.querySelector('[data-client-email]')?.textContent).toBe(
      'billing@acme.test',
    );
    expect(el.shadowRoot?.querySelector('[data-issued]')?.textContent?.trim()).toBe(
      'Feb 20, 2026',
    );
    expect(el.shadowRoot?.querySelector('[data-due]')?.textContent?.trim()).toBe(
      'Mar 20, 2026',
    );
  });

  it('marks an overdue due date and prints the note it is given', async () => {
    const el = await mount({ status: 'overdue', dueNote: '19 days overdue' });
    expect(el.shadowRoot?.querySelector('[data-due]')?.hasAttribute('data-overdue')).toBe(true);
    expect(el.shadowRoot?.querySelector('[data-due-note]')?.textContent).toBe(
      '19 days overdue',
    );

    const current = await mount({ dueNote: '' });
    expect(current.shadowRoot?.querySelector('[data-due]')?.hasAttribute('data-overdue')).toBe(
      false,
    );
    expect(current.shadowRoot?.querySelector('[data-due-note]')).toBeNull();
  });

  it('puts slotted actions in the title row', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('.title slot[name="actions"]')).toBeTruthy();
  });

  it('shows the total and what is still outstanding, not one of them', async () => {
    const el = await mount();
    expect(amountOf(el, '[data-total]')).toBe(3200);
    expect(amountOf(el, '[data-balance]')).toBe(1200);
  });

  it('says an invoice with no due date is due on receipt', async () => {
    const el = await mount({ dueDate: null });
    expect(el.shadowRoot?.querySelector('[data-due]')?.textContent?.trim()).toBe('On receipt');
  });

  it('renders an em dash for an invoice whose client row is gone', async () => {
    const el = await mount({ clientName: null });
    expect(el.shadowRoot?.querySelector('[data-client]')?.textContent?.trim()).toBe('—');
  });

  it('passes the currency down to the figures', async () => {
    const el = await mount({ currency: 'EUR' });
    const money = el.shadowRoot?.querySelector('[data-total]') as HTMLElement & {
      currency: string;
    };
    expect(money.currency).toBe('EUR');
  });

  it('renders a null balance as an em dash, not as a figure', async () => {
    // A void invoice owes nothing and never will; its total under
    // "Balance" would report a receivable the aging report excludes.
    const el = await mount({ status: 'void', total: 500, balance: null });
    const balance = el.shadowRoot?.querySelector('[data-balance]');
    expect(balance?.textContent?.trim()).toBe('\u2014');
    expect(balance?.tagName.toLowerCase()).not.toBe('wc-money');
  });
});

describePreviewA11y(preview);
