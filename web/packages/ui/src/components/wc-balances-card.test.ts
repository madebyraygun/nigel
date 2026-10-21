import { describe, it, expect, afterEach } from 'vitest';
import './wc-balances-card.js';
import type { WcBalancesCard } from './wc-balances-card.js';
import type { WcMoney } from './wc-money.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-balances-card.preview.js';

async function mount(props: Partial<WcBalancesCard> = {}): Promise<WcBalancesCard> {
  const el = document.createElement('wc-balances-card');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

const all = (el: WcBalancesCard, selector: string) => [
  ...(el.shadowRoot?.querySelectorAll<HTMLElement>(selector) ?? []),
];

const items = [
  { name: 'Harbor Checking', accountType: 'checking', balance: 100 },
  { name: 'Harbor Credit Card', accountType: 'credit_card', balance: -25 },
];

describe('wc-balances-card', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('lists each account with its readable type', async () => {
    const el = await mount({ items });
    expect(all(el, 'tbody th').map((th) => th.textContent?.replace(/\s+/g, ' ').trim())).toEqual([
      'Harbor Checking checking',
      'Harbor Credit Card credit card',
    ]);
  });

  it('hands balances to wc-money and puts the total after them', async () => {
    const el = await mount({ items, total: 75 });
    const amounts = all(el, 'wc-money').map((m) => (m as WcMoney).amount);
    expect(amounts).toEqual([100, -25, 75]);
    expect(el.shadowRoot?.querySelector('tfoot th')?.textContent).toBe('Total');
  });

  it('draws no total row when none is given', async () => {
    const el = await mount({ items });
    expect(el.shadowRoot?.querySelector('tfoot')).toBeNull();
  });

  it('says so when there are no accounts', async () => {
    const el = await mount({ items: [] });
    expect(el.shadowRoot?.querySelector('.empty')?.textContent).toBe('No accounts yet.');
  });

  it('shows the error and retries', async () => {
    const el = await mount({ items, error: 'boom' });
    expect(el.shadowRoot?.querySelector('table')).toBeNull();
    let fired = 0;
    el.addEventListener('nc-retry', () => (fired += 1));
    el.shadowRoot?.querySelector<HTMLButtonElement>('.retry')?.click();
    expect(fired).toBe(1);
  });

  it('shows a spinner while loading', async () => {
    const el = await mount({ items, loading: true });
    expect(el.shadowRoot?.querySelector('wc-spinner')).not.toBeNull();
  });
});

describePreviewA11y(preview);
