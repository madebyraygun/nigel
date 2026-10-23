import { describe, it, expect, afterEach } from 'vitest';
import './wc-aging-bars.js';
import {
  agingBucketLabel,
  agingShares,
  WcAgingBars,
  type AgingBucketView,
} from './wc-aging-bars.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-aging-bars.preview.js';

const BUCKETS: AgingBucketView[] = [
  { label: 'current', count: 2, total: 4200 },
  { label: '1-30', count: 1, total: 1850 },
  { label: '31-60', count: 0, total: 0 },
  { label: '61-90', count: 1, total: 960 },
  { label: '90+', count: 0, total: 0 },
];

async function mount(props: Partial<WcAgingBars> = {}): Promise<WcAgingBars> {
  const el = document.createElement('wc-aging-bars');
  Object.assign(el, { buckets: BUCKETS, total: 7010, asOf: '2026-08-07' }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('agingShares', () => {
  it('splits the bar by each bucket’s share of the whole', () => {
    const shares = agingShares(BUCKETS);
    expect(shares.map((share) => Math.round(share * 1000))).toEqual([599, 264, 0, 137, 0]);
    expect(shares.reduce((a, b) => a + b, 0)).toBeCloseTo(1, 10);
  });

  it('draws nothing when every bucket is zero', () => {
    expect(agingShares(BUCKETS.map((b) => ({ ...b, total: 0 })))).toEqual([0, 0, 0, 0, 0]);
  });
});

describe('agingBucketLabel', () => {
  it('names the five buckets the way the legend prints them', () => {
    expect(BUCKETS.map((bucket) => agingBucketLabel(bucket.label))).toEqual([
      'Current',
      '1–30 days',
      '31–60 days',
      '61–90 days',
      'Over 90',
    ]);
  });

  it('prints a bucket it does not know as written', () => {
    expect(agingBucketLabel('120+')).toBe('120+');
  });
});

describe('wc-aging-bars', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('prints the outstanding total and every bucket, in that order', async () => {
    const el = await mount();
    const amounts = [...(el.shadowRoot?.querySelectorAll('wc-money') ?? [])].map(
      (money) => (money as HTMLElement & { amount: number }).amount,
    );
    expect(amounts).toEqual([7010, 4200, 1850, 0, 960, 0]);
  });

  it('counts the open invoices across the buckets and says what day it is about', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('[data-meta]')?.textContent).toBe(
      'across 4 open invoices · as of Aug 7, 2026',
    );
  });

  it('draws a segment per non-empty bucket, hidden from assistive tech', async () => {
    const el = await mount();
    const bar = el.shadowRoot?.querySelector('.bar');
    expect(bar?.getAttribute('aria-hidden')).toBe('true');
    expect(
      [...(bar?.querySelectorAll('.segment') ?? [])].map((s) => s.getAttribute('data-bucket')),
    ).toEqual(['0', '1', '3']);
  });

  it('paints the five buckets from the theme’s aging fills', () => {
    const css = [WcAgingBars.styles].flat().map(String).join('\n');
    for (const n of [1, 2, 3, 4, 5]) expect(css).toContain(`var(--nc-color-aging-${n})`);
  });

  it('mutes an empty bucket’s amount', async () => {
    const el = await mount();
    const zero = el.shadowRoot?.querySelector('[data-legend="31-60"] dd');
    expect(zero?.hasAttribute('data-zero')).toBe(true);
    const full = el.shadowRoot?.querySelector('[data-legend="current"] dd');
    expect(full?.hasAttribute('data-zero')).toBe(false);
  });

  it('links to the full report only when given somewhere to go', async () => {
    const bare = await mount();
    expect(bare.shadowRoot?.querySelector('a')).toBeNull();

    const linked = await mount({ href: '#/reports?report=aging' });
    expect(linked.shadowRoot?.querySelector('a')?.getAttribute('href')).toBe(
      '#/reports?report=aging',
    );
  });

  it('says so rather than drawing an empty card when there are no buckets', async () => {
    const el = await mount({ buckets: [] });
    expect(el.shadowRoot?.querySelector('.empty')?.textContent).toContain(
      'No open receivables',
    );
  });
});

describePreviewA11y(preview);
