import { describe, it, expect, afterEach, vi } from 'vitest';
import './wc-breadcrumb.js';
import { WcBreadcrumb } from './wc-breadcrumb.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { describePrintHiding } from '../../preview/print-suite.js';
import preview from './wc-breadcrumb.preview.js';

async function mount(props: Partial<WcBreadcrumb> = {}): Promise<WcBreadcrumb> {
  const el = document.createElement('wc-breadcrumb');
  Object.assign(el, { bookName: 'Acme Consulting LLC', bookKey: 'acme' }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-breadcrumb', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('is a labelled breadcrumb landmark', async () => {
    const el = await mount({ trail: [{ label: 'Dashboard' }] });
    expect(el.shadowRoot?.querySelector('nav')?.getAttribute('aria-label')).toBe('Breadcrumb');
  });

  it('leads with the book: its monogram and its name', async () => {
    const el = await mount({ trail: [{ label: 'Dashboard' }] });
    const book = el.shadowRoot?.querySelector('.book');
    expect(book?.querySelector('wc-monogram')?.getAttribute('name')).toBe('Acme Consulting LLC');
    expect(book?.querySelector('wc-monogram')?.getAttribute('seed')).toBe('acme');
    expect(book?.querySelector('.book-name')?.textContent).toBe('Acme Consulting LLC');
  });

  it('asks for the palette scoped to books when the book is pressed', async () => {
    const el = await mount({ trail: [{ label: 'Dashboard' }] });
    const spy = vi.fn();
    document.body.addEventListener('nc-palette-open', spy);
    el.shadowRoot?.querySelector<HTMLButtonElement>('.book')?.click();
    document.body.removeEventListener('nc-palette-open', spy);
    expect(spy).toHaveBeenCalledOnce();
    expect(spy.mock.calls[0][0].detail).toEqual({ scope: 'books' });
  });

  it('marks the last level as the current page and does not make it a button', async () => {
    const el = await mount({ trail: [{ label: 'Dashboard', id: 'dashboard' }] });
    const current = el.shadowRoot?.querySelector('[aria-current="page"]');
    expect(current?.textContent?.trim()).toBe('Dashboard');
    expect(current?.tagName).toBe('SPAN');
  });

  it('turns an earlier level into a way back to its screen', async () => {
    const el = await mount({
      trail: [{ label: 'Invoices', id: 'invoices' }, { label: '#1254' }],
    });
    const spy = vi.fn();
    el.addEventListener('nc-navigate', spy);
    el.shadowRoot?.querySelector<HTMLButtonElement>('[data-level="invoices"]')?.click();
    expect(spy.mock.calls[0][0].detail).toEqual({ id: 'invoices' });
    expect(spy.mock.calls[0][0].composed).toBe(true);
  });

  it('separates the levels with a slash nobody hears', async () => {
    const el = await mount({
      trail: [{ label: 'Invoices', id: 'invoices' }, { label: '#1254' }],
    });
    const seps = [...(el.shadowRoot?.querySelectorAll('.sep') ?? [])];
    expect(seps).toHaveLength(2);
    for (const sep of seps) {
      expect(sep.textContent).toBe('/');
      expect(sep.getAttribute('aria-hidden')).toBe('true');
    }
  });

  it('sets the last level of a deeper trail in text colour', async () => {
    const el = await mount({
      trail: [{ label: 'Invoices', id: 'invoices' }, { label: '#1254' }],
    });
    expect(el.shadowRoot?.querySelector('[aria-current="page"]')?.classList).toContain('deep');
  });
});

describePreviewA11y(preview);

describePrintHiding(WcBreadcrumb, ':host');
