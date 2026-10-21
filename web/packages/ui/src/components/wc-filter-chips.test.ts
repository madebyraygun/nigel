import { describe, it, expect, afterEach } from 'vitest';
import './wc-filter-chips.js';
import type { FilterChip, WcFilterChips } from './wc-filter-chips.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-filter-chips.preview.js';

const CHIPS: FilterChip[] = [
  { value: 'all', label: 'All', count: 3, href: '#/invoices' },
  { value: 'paid', label: 'Paid', count: 2, href: '#/invoices?status=paid' },
  { value: 'clear', label: 'Clear client filter', href: '#/invoices' },
];

async function mount(active = 'paid'): Promise<WcFilterChips> {
  const el = document.createElement('wc-filter-chips');
  el.label = 'Status';
  el.chips = CHIPS;
  el.active = active;
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-filter-chips', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders each chip as a link with its count', async () => {
    const el = await mount();
    const links = [...(el.shadowRoot?.querySelectorAll('a') ?? [])];
    expect(links.map((a) => a.getAttribute('href'))).toEqual([
      '#/invoices',
      '#/invoices?status=paid',
      '#/invoices',
    ]);
    expect(links.map((a) => a.querySelector('.count')?.textContent ?? null)).toEqual([
      '3',
      '2',
      null,
    ]);
  });

  it('marks only the chip in effect as current', async () => {
    const el = await mount();
    const current = [...(el.shadowRoot?.querySelectorAll('[aria-current="true"]') ?? [])];
    expect(current.map((a) => a.getAttribute('data-filter'))).toEqual(['paid']);
  });

  it('names the group with its label', async () => {
    const el = await mount();
    const group = el.shadowRoot?.querySelector('[role="group"]');
    expect(group?.getAttribute('aria-labelledby')).toBe('label');
    expect(el.shadowRoot?.getElementById('label')?.textContent).toBe('Status');
  });
});

describePreviewA11y(preview);
