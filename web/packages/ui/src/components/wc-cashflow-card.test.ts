import { describe, it, expect, afterEach } from 'vitest';
import './wc-cashflow-card.js';
import { WcCashflowCard as Ctor, type WcCashflowCard } from './wc-cashflow-card.js';
import type { BarBucket } from './wc-bar-chart.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { styleText } from '../../preview/controls-suite.js';
import preview from './wc-cashflow-card.preview.js';

const buckets: BarBucket[] = [
  { label: 'Jan', income: 1000, expense: 500 },
  { label: 'Feb', income: 2000, expense: 250 },
];

async function mount(props: Partial<WcCashflowCard> = {}): Promise<WcCashflowCard> {
  const el = document.createElement('wc-cashflow-card');
  Object.assign(el, { locale: 'en-US', ...props });
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

const all = (el: WcCashflowCard, selector: string) => [
  ...(el.shadowRoot?.querySelectorAll<HTMLElement>(selector) ?? []),
];

describe('wc-cashflow-card', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('shows the title, the range and a two-key legend', async () => {
    const el = await mount({ buckets, range: '2025 – 26' });
    expect(el.shadowRoot?.querySelector('.title')?.textContent).toBe('Cash flow');
    expect(el.shadowRoot?.querySelector('.range')?.textContent).toBe('2025 – 26');
    expect(all(el, '.legend .key').map((k) => k.textContent?.trim())).toEqual([
      'Income',
      'Expenses',
    ]);
  });

  it('draws a pair of bars per bucket scaled to the tallest figure', async () => {
    const el = await mount({ buckets });
    expect(all(el, '.bar.income').map((b) => b.style.height)).toEqual(['50%', '100%']);
    expect(all(el, '.bar.expense').map((b) => b.style.height)).toEqual(['25%', '12.5%']);
  });

  it('labels each month under the baseline', async () => {
    const el = await mount({ buckets });
    expect(all(el, '.tick').map((t) => t.textContent)).toEqual(['Jan', 'Feb']);
  });

  it('names itself for assistive tech and repeats the figures in a table', async () => {
    const el = await mount({ buckets, range: '2026' });
    const label = el.shadowRoot?.querySelector('[role="img"]')?.getAttribute('aria-label');
    expect(label).toContain('$3,000.00');
    expect(label).toContain('$750.00');
    expect(all(el, 'table.sr-only tbody tr')[0]?.textContent).toContain('$1,000.00');
  });

  it('offers the empty slot rather than an empty plot', async () => {
    const el = await mount({ buckets: [] });
    expect(el.shadowRoot?.querySelector('.plot')).toBeNull();
    expect(el.shadowRoot?.querySelector('slot[name="empty"]')).not.toBeNull();
  });

  it('shows a spinner while loading', async () => {
    const el = await mount({ buckets, loading: true });
    expect(el.shadowRoot?.querySelector('wc-spinner')).not.toBeNull();
    expect(el.shadowRoot?.querySelector('.plot')).toBeNull();
  });

  it('shows the error instead of a stale chart and retries', async () => {
    const el = await mount({ buckets, error: 'boom' });
    expect(el.shadowRoot?.querySelector('.plot')).toBeNull();
    let fired = 0;
    el.addEventListener('nc-retry', () => (fired += 1));
    el.shadowRoot?.querySelector<HTMLButtonElement>('.retry')?.click();
    expect(fired).toBe(1);
  });

  it('draws bars with fully rounded tops, 38% wide, 3px apart', () => {
    const text = styleText(Ctor);
    expect(text).toMatch(/\.bar\s*{[^}]*width:\s*38%/);
    expect(text).toMatch(/\.bar\s*{[^}]*border-radius:\s*999px 999px 0 0/);
    expect(text).toMatch(/\.bucket\s*{[^}]*gap:\s*3px/);
    expect(text).toMatch(/\.plot\s*{[^}]*height:\s*220px/);
  });

  it('scrolls inside the card rather than widening the page', () => {
    expect(styleText(Ctor)).toMatch(/\.chart\s*{[^}]*overflow-x:\s*auto/);
  });
});

describePreviewA11y(preview);
