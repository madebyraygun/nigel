import { describe, it, expect, afterEach, vi } from 'vitest';
import './wc-nav-sidebar.js';
import { WcNavSidebar } from './wc-nav-sidebar.js';
import { describePrintHiding } from '../../preview/print-suite.js';
import { NAV_ITEMS, NAV_ITEMS_WITH_DISABLED } from './__mocks__/nav.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { styleText } from '../../preview/controls-suite.js';
import preview from './wc-nav-sidebar.preview.js';

async function mount(props: Partial<WcNavSidebar> = {}): Promise<WcNavSidebar> {
  const el = document.createElement('wc-nav-sidebar');
  Object.assign(el, { items: NAV_ITEMS, active: 'dashboard' }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function button(el: WcNavSidebar, id: string): HTMLButtonElement | null {
  return el.shadowRoot?.querySelector(`[data-nav="${id}"]`) ?? null;
}

describe('wc-nav-sidebar', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders one button per item', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelectorAll('[data-nav]').length).toBe(NAV_ITEMS.length);
  });

  it('labels the navigation landmark', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('nav')?.getAttribute('aria-label')).toBe('Primary');
  });

  it('marks the active item as the current page', async () => {
    const el = await mount({ active: 'register' });
    expect(button(el, 'register')?.getAttribute('aria-current')).toBe('page');
    expect(button(el, 'dashboard')?.getAttribute('aria-current')).toBe('false');
  });

  it('emits nc-navigate with the item id', async () => {
    const el = await mount();
    const spy = vi.fn();
    el.addEventListener('nc-navigate', spy);

    button(el, 'register')?.click();

    expect(spy).toHaveBeenCalledOnce();
    expect(spy.mock.calls[0][0].detail).toEqual({ id: 'register' });
  });

  it('bubbles and composes the event so the app can listen at the shell', async () => {
    const el = await mount();
    const spy = vi.fn();
    document.body.addEventListener('nc-navigate', spy);
    button(el, 'review')?.click();
    expect(spy).toHaveBeenCalledOnce();
    document.body.removeEventListener('nc-navigate', spy);
  });

  it('does not navigate from a disabled item', async () => {
    const el = await mount({ items: NAV_ITEMS_WITH_DISABLED });
    const spy = vi.fn();
    el.addEventListener('nc-navigate', spy);

    button(el, 'register')?.click();

    expect(spy).not.toHaveBeenCalled();
  });

  it('exposes disabled state to assistive tech', async () => {
    const el = await mount({ items: NAV_ITEMS_WITH_DISABLED });
    expect(button(el, 'register')?.getAttribute('aria-disabled')).toBe('true');
    expect(button(el, 'dashboard')?.getAttribute('aria-disabled')).toBe('false');
  });

  it('keeps the label reachable when collapsed', async () => {
    const el = await mount({ collapsed: true });
    // The text is hidden by CSS, so the title attribute is what is left.
    expect(button(el, 'register')?.getAttribute('title')).toBe('Register');
  });

  it('reflects collapsed so CSS can key off it', async () => {
    const el = await mount({ collapsed: true });
    expect(el.hasAttribute('collapsed')).toBe(true);
  });

  it('renders the icon element named by an item', async () => {
    const el = await mount();
    expect(button(el, 'register')?.querySelector('wc-icon-register')).toBeTruthy();
  });

  it('renders items without icons', async () => {
    const el = await mount({ items: [{ id: 'x', label: 'X' }] });
    expect(button(el, 'x')?.textContent?.trim()).toBe('X');
  });
});

describePreviewA11y(preview);

describePrintHiding(WcNavSidebar, ':host');

describe('the rail', () => {
  const text = styleText(WcNavSidebar);

  it('slides between the column and the rail rather than snapping', () => {
    // Width is the only thing that differs between the two states, so it is
    // the only thing there is to animate.
    expect(text).toMatch(/:host\s*{[^}]*transition:\s*width\s*var\(--nc-transition-base/);
  });

  it('stops sliding for anyone who asked motion to stop', () => {
    // --nc-transition-base is already 0ms under the same preference; this
    // says so where someone reading the component will see it.
    expect(text).toMatch(
      /@media \(prefers-reduced-motion: reduce\)[\s\S]*:host[^{]*{[^}]*transition:\s*none/,
    );
  });

  it('clips the labels rather than reflowing them on the way through', () => {
    // A label is drawn at its full width from the first frame of an
    // expansion, and the rail is 56px wide for most of that frame's life.
    expect(text).toMatch(/:host\s*{[^}]*overflow-x:\s*hidden/);
    expect(text).toMatch(/\.brand,\s*button\s*{[^}]*white-space:\s*nowrap/);
  });
});

describe('wc-nav-sidebar on a phone', () => {
  // jsdom has no layout engine, so the rules are read the way the register's
  // fill rules are.
  const text = styleText(WcNavSidebar);

  it('keeps its labels when it is the drawer rather than the rail', () => {
    // Collapsed means "off-canvas" at this width, not "56px of icons": the
    // shell slides the whole sidebar away, and what slides back in has room
    // for the words.
    expect(text).toMatch(
      /@media \(max-width: 48rem\)[\s\S]*:host\(\[collapsed\]\) \.label[^{]*{[^}]*display:\s*revert/,
    );
  });

  it('keeps its full width when it is the drawer', () => {
    expect(text).toMatch(
      /@media \(max-width: 48rem\)[\s\S]*:host\(\[collapsed\]\)[^{]*{[^}]*width:\s*var\(--nc-sidebar-width/,
    );
  });
});
