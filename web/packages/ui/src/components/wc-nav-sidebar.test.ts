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
    // The text is cropped by the 56px box, so the title carries it whole.
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

  /** One @media block, so an assertion cannot bridge into a later rule that
      happens to say the same words for a different reason. */
  function blockOf(marker: string): string {
    const at = text.indexOf(marker);
    expect(at, `${marker} is not in the stylesheet`).toBeGreaterThan(-1);
    const next = text.indexOf('@media', at + marker.length);
    return next === -1 ? text.slice(at) : text.slice(at, next);
  }

  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('animates only behind the gesture attribute, never on the bare host', () => {
    // The transition exists only while a toggle holds data-animating, so a
    // resize — including one across the breakpoint, in either direction —
    // moves the width in a single frame.
    expect(text).toMatch(
      /:host\(\[data-animating\]\)\s*{[^}]*transition:\s*width\s*var\(--nc-transition-base/,
    );
    expect(text).not.toMatch(/:host\s*{[^}]*transition:/);
  });

  it('sets the gesture attribute on a toggle and clears it after', async () => {
    const el = await mount();
    el.collapsed = true;
    await el.updateComplete;
    expect(el.hasAttribute('data-animating')).toBe(true);
    await new Promise((resolve) => setTimeout(resolve, 350));
    expect(el.hasAttribute('data-animating')).toBe(false);
  });

  it('does not treat the mounted state as a gesture', async () => {
    const el = await mount({ collapsed: true });
    expect(el.hasAttribute('data-animating')).toBe(false);
  });

  it('stops sliding for anyone who asked motion to stop', () => {
    expect(blockOf('@media (prefers-reduced-motion: reduce)')).toMatch(
      /:host\(\[data-animating\]\)\s*{[^}]*transition:\s*none/,
    );
  });

  it('clips a long label to an ellipsis instead of wrapping it', () => {
    expect(text).toMatch(/:host\s*{[^}]*overflow-x:\s*hidden/);
    expect(text).toMatch(/\.brand,\s*button\s*{[^}]*white-space:\s*nowrap/);
    expect(text).toMatch(/\.label\s*{[^}]*text-overflow:\s*ellipsis/);
    expect(text).toMatch(/\.brand-name\s*{[^}]*text-overflow:\s*ellipsis/);
  });

  it('wipes in both directions: no width ever hides a label outright', () => {
    // Collapse reads as expand run backwards because the label keeps its
    // layout and the moving edge crops it; a display: none at frame one is
    // the pop this pins against. The print block hides the host, which is
    // hiding chrome from paper rather than a label from a state.
    expect(text).not.toMatch(/\.label[^{]*{[^}]*display:\s*none/);
    expect(text).not.toMatch(/\.brand-name[^{]*{[^}]*display:\s*none/);
  });

  it('keeps the whole label reachable when it is clipped', async () => {
    // Zoom or a large system font can push a label past the box in either
    // state, so the title is not conditional on the rail.
    const el = await mount({ items: NAV_ITEMS });
    const titles = [...(el.shadowRoot?.querySelectorAll('button') ?? [])].map((b) =>
      b.getAttribute('title'),
    );
    expect(titles).toEqual(NAV_ITEMS.map((item) => item.label));
  });

  it('leaves motion to the shell at drawer widths', () => {
    // Below the breakpoint wc-app-shell slides the whole sidebar with
    // transform from the outer tree, so even a gesture must not also
    // animate width here.
    expect(blockOf('@media (max-width: 48rem)')).toMatch(
      /:host\(\[data-animating\]\)\s*{[^}]*transition:\s*none/,
    );
  });

  it('keeps its full width when it is the drawer', () => {
    expect(blockOf('@media (max-width: 48rem)')).toMatch(
      /:host\(\[collapsed\]\)\s*{[^}]*width:\s*var\(--nc-sidebar-width/,
    );
  });
});
