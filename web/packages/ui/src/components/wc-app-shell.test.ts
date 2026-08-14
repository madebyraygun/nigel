import { describe, it, expect, afterEach } from 'vitest';
import './wc-app-shell.js';
import { WcAppShell } from './wc-app-shell.js';
import { dispatchNcToast } from './wc-toast.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { describePrintHiding } from '../../preview/print-suite.js';
import { styleText } from '../../preview/controls-suite.js';
import preview from './wc-app-shell.preview.js';

/** Everything the shell adopts into its shadow root. */
const text = styleText(WcAppShell);

async function mount(props: Partial<WcAppShell> = {}): Promise<WcAppShell> {
  const el = document.createElement('wc-app-shell');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-app-shell', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders the screen title in the header', async () => {
    const el = await mount({ screenTitle: 'Register' });
    expect(el.shadowRoot?.querySelector('.title')?.textContent).toBe('Register');
  });

  it('exposes the sidebar, header-actions, banner and default slots', async () => {
    const el = await mount();
    const names = [...(el.shadowRoot?.querySelectorAll('slot') ?? [])].map((s) =>
      s.getAttribute('name'),
    );
    expect(names).toContain('sidebar');
    expect(names).toContain('header-actions');
    expect(names).toContain('banner');
    expect(names).toContain(null);
  });

  it('wraps content in a main landmark', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('main')).toBeTruthy();
  });

  it('hosts exactly one toast region', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelectorAll('wc-toast').length).toBe(1);
  });

  it('shows toasts dispatched from its content', async () => {
    const el = await mount();
    const toast = el.shadowRoot?.querySelector('wc-toast');
    await toast?.updateComplete;

    dispatchNcToast(el, { message: 'Saved.' });
    await toast?.updateComplete;

    expect(toast?.shadowRoot?.querySelector('.toast')?.textContent).toContain('Saved.');
  });

  it('reflects sidebar-collapsed so CSS can key off it', async () => {
    const el = await mount({ sidebarCollapsed: true });
    expect(el.hasAttribute('sidebar-collapsed')).toBe(true);
  });

  it('exposes its furniture as parts', async () => {
    const el = await mount();
    // `::part()` is the only way a document stylesheet reaches inside a shadow
    // root, and the print sheet has to hide all three of these to give the page
    // over to the content.
    const parts = [...(el.shadowRoot?.querySelectorAll('[part]') ?? [])].map((node) =>
      node.getAttribute('part'),
    );
    expect(parts).toEqual(['sidebar', 'header', 'banner', 'content']);
  });
});

describePreviewA11y(preview);

describePrintHiding(WcAppShell, 'header', ".banner", "::slotted([slot='sidebar'])");

describe('wc-app-shell content area', () => {
  it('lets a screen ask for the whole window below the header', () => {
    // The register is the screen that needs this: its table has to end at the
    // bottom of the window rather than partway down it. A block content area
    // gives a slotted screen no height to divide up, so `flex: 1` on the
    // screen's own host would have nothing to grow into.
    expect(text).toMatch(/\.content\s*{[^}]*display:\s*flex/);
    expect(text).toMatch(/\.content\s*{[^}]*flex-direction:\s*column/);
    expect(text).toMatch(/\.content\s*{[^}]*min-height:\s*0/);
  });

  it('still scrolls a screen that is taller than the window', () => {
    expect(text).toMatch(/\.content\s*{[^}]*overflow:\s*auto/);
  });
});

describe('wc-app-shell on paper', () => {
  it('gives the whole page over to the content', () => {
    // On screen the shell is a 100vh flex box with a scrolling main. On paper
    // that clamps a ten-page report to one viewport-high box and throws the
    // rest away, so both the clamp and the scroll have to come off.
    expect(text).toMatch(/@media print[\s\S]*\.content[^{]*{[^}]*overflow:\s*visible/);
    expect(text).toMatch(/@media print[\s\S]*\.content[^{]*{[^}]*padding:\s*0/);
    expect(text).toMatch(/@media print[\s\S]*:host[^{]*{[^}]*height:\s*auto/);
  });

  it('keeps the parts it exposes', () => {
    // nigel-app is one boundary away and can still use them; they are cheap
    // and documented. What changed is that print no longer depends on them.
    const html = WcAppShell.prototype.render.toString();
    for (const part of ['sidebar', 'header', 'banner', 'content']) {
      expect(html).toContain(`part="${part}"`);
    }
  });
});
