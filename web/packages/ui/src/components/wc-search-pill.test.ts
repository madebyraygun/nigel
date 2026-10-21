import { describe, it, expect, afterEach, vi } from 'vitest';
import './wc-search-pill.js';
import { WcSearchPill } from './wc-search-pill.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { describePrintHiding } from '../../preview/print-suite.js';
import preview from './wc-search-pill.preview.js';

async function mount(props: Partial<WcSearchPill> = {}): Promise<WcSearchPill> {
  const el = document.createElement('wc-search-pill');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-search-pill', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('asks for the whole palette', async () => {
    const el = await mount();
    const spy = vi.fn();
    document.body.addEventListener('nc-palette-open', spy);
    el.shadowRoot?.querySelector('button')?.click();
    document.body.removeEventListener('nc-palette-open', spy);
    expect(spy).toHaveBeenCalledOnce();
    expect(spy.mock.calls[0][0].detail).toEqual({ scope: 'all' });
  });

  it('shows the shortcut it is given on the keycap', async () => {
    const el = await mount({ shortcut: 'Ctrl K', keyShortcuts: 'Control+K' });
    expect(el.shadowRoot?.querySelector('kbd')?.textContent).toBe('Ctrl K');
    expect(el.shadowRoot?.querySelector('button')?.getAttribute('aria-keyshortcuts')).toBe(
      'Control+K',
    );
  });

  it('keeps its name when it folds to the magnifier', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('button')?.getAttribute('aria-label')).toBe('Search');
  });
});

describePreviewA11y(preview);

describePrintHiding(WcSearchPill, ':host');
