import { describe, it, expect, afterEach } from 'vitest';
import './wc-stat-strip.js';
import type { StatStripCell, WcStatStrip } from './wc-stat-strip.js';
import { WcStatStrip as Ctor } from './wc-stat-strip.js';
import type { WcMoney } from './wc-money.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { styleText } from '../../preview/controls-suite.js';
import { controlsCss } from '@nigel/theme';
import preview from './wc-stat-strip.preview.js';

const cells: StatStripCell[] = [
  { id: 'income', label: 'Income, year to date', amount: 100, hint: '2026' },
  { id: 'expenses', label: 'Expenses, year to date', amount: -40 },
  { id: 'net', label: 'Net', amount: 60, hint: 'Cash basis' },
];

async function mount(props: Partial<WcStatStrip> = {}): Promise<WcStatStrip> {
  const el = document.createElement('wc-stat-strip');
  Object.assign(el, { cells, ...props });
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

const all = (el: WcStatStrip, selector: string) => [
  ...(el.shadowRoot?.querySelectorAll<HTMLElement>(selector) ?? []),
];

describe('wc-stat-strip', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('draws one cell per entry with its label and hint', async () => {
    const el = await mount();
    expect(all(el, '.cell').length).toBe(3);
    expect(all(el, '.label').map((n) => n.textContent)).toEqual([
      'Income, year to date',
      'Expenses, year to date',
      'Net',
    ]);
    expect(all(el, '.hint').map((n) => n.textContent)).toEqual(['2026', 'Cash basis']);
  });

  it('hands each amount to wc-money so the sign colours it', async () => {
    const el = await mount();
    const amounts = all(el, 'wc-money').map((n) => (n as WcMoney).amount);
    expect(amounts).toEqual([100, -40, 60]);
  });

  it('takes a fourth cell without any other change', async () => {
    const el = await mount({
      cells: [...cells, { id: 'ar', label: 'Outstanding', amount: 5 }],
    });
    expect(all(el, '.cell').length).toBe(4);
  });

  it('shows a failed cell as an error and leaves its neighbours', async () => {
    const el = await mount({
      cells: [cells[0]!, { ...cells[1]!, error: 'boom' }, cells[2]!],
    });
    expect(all(el, '.error').length).toBe(1);
    expect(all(el, 'wc-money').length).toBe(2);
  });

  it('names the cell whose retry was pressed', async () => {
    const el = await mount({ cells: [{ ...cells[1]!, error: 'boom' }] });
    let id = '';
    el.addEventListener('nc-retry', (e) => (id = (e as CustomEvent).detail.id));
    el.shadowRoot?.querySelector<HTMLButtonElement>('.retry')?.click();
    expect(id).toBe('expenses');
  });

  it('shows a spinner for a loading cell', async () => {
    const el = await mount({ cells: [{ ...cells[0]!, loading: true }] });
    expect(el.shadowRoot?.querySelector('wc-spinner')).not.toBeNull();
    expect(el.shadowRoot?.querySelector('wc-money')).toBeNull();
  });

  it('names each retry button for its cell', async () => {
    const el = await mount({ cells: [{ ...cells[1]!, error: 'boom' }] });
    // wa-button does not forward a host aria-label to its inner button, so
    // the cell's name is in the content, where the inner button's name comes from.
    const retry = el.shadowRoot?.querySelector('.retry');
    expect(retry?.hasAttribute('aria-label')).toBe(false);
    expect(retry?.textContent?.replace(/\s+/g, ' ').trim()).toBe('Retry Expenses, year to date');
    expect(retry?.querySelector('.sr-only')?.textContent?.replace(/\s+/g, ' ').trim()).toBe('Expenses, year to date');
  });

  it('divides cells with a left hairline that becomes a top one when the strip is narrow', () => {
    const text = styleText(Ctor);
    expect(text).toMatch(/\.cell \+ \.cell\s*{[^}]*border-left:\s*1px solid/);
    expect(text).toMatch(/@container \(max-width: 46rem\)[\s\S]*border-top:\s*1px solid/);
  });

  it('wraps on its own width rather than the viewport', () => {
    // Its own rules only: the adopted controlsCss carries forced-colors and
    // print queries, which are not layout.
    const text = styleText(Ctor).replace(controlsCss.cssText, '');
    expect(text).toMatch(/:host\s*{[^}]*container-type:\s*inline-size/);
    expect(text).not.toMatch(/@media/);
  });

  it('wraps four cells earlier than three', async () => {
    const el = await mount({
      cells: [...cells, { id: 'ar', label: 'Outstanding', amount: 5 }],
    });
    expect(el.shadowRoot?.querySelector('.cells')?.getAttribute('data-count')).toBe('4');
    expect(styleText(Ctor)).toMatch(/@container \(max-width: 62rem\)/);
  });
});

describePreviewA11y(preview);
