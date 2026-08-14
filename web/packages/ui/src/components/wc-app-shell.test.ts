import { describe, it, expect, afterEach } from 'vitest';
import './wc-app-shell.js';
import { WcAppShell } from './wc-app-shell.js';
import { dispatchNcToast } from './wc-toast.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { describePrintHiding } from '../../preview/print-suite.js';
import { styleText } from '../../preview/controls-suite.js';
import {
  describeColumnLayout,
  describePrintsAsBlock,
  printedBox,
  resolvedBox,
} from '../../preview/layout-suite.js';
import preview from './wc-app-shell.preview.js';

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

  it('puts the screen slot inside the content area and nothing between them', async () => {
    const el = await mount();
    // A wrapper here would be a second box for the screen to fill, and the
    // screen would be as tall as its content again.
    expect(el.shadowRoot?.querySelector('main.content > slot:not([name])')).toBeTruthy();
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

describeColumnLayout(WcAppShell, '.content');

describePrintsAsBlock(WcAppShell, '.content');

describe('the content area', () => {
  it('gives the screen the whole of it', () => {
    // The screen is the only thing in the default slot; whether anything on it
    // can be centred vertically is decided by how tall the screen is allowed
    // to be, which is here and nowhere else.
    expect(resolvedBox(WcAppShell, '.content ::slotted(*)').flexGrow).toBe('1');
  });
});

describePrintHiding(WcAppShell, 'header', ".banner", "::slotted([slot='sidebar'])");

describe('wc-app-shell on paper', () => {
  const text = styleText(WcAppShell);

  it('gives the whole page over to the content', () => {
    // On screen the shell is a 100vh flex box with a scrolling main. On paper
    // that clamps a ten-page report to one viewport-high box and throws the
    // rest away, so both the clamp and the scroll have to come off.
    expect(text).toMatch(/@media print[\s\S]*\.content[^{]*{[^}]*overflow:\s*visible/);
    expect(text).toMatch(/@media print[\s\S]*\.content[^{]*{[^}]*padding:\s*0/);
    expect(text).toMatch(/@media print[\s\S]*:host[^{]*{[^}]*height:\s*auto/);
  });

  it('puts the screen back into block flow', () => {
    // The column is for a viewport: it hands out the height left over in one.
    // A sheet has none to hand out, and a flex container is not required to
    // fragment — Safari and older Chromium slice through a row rather than
    // break between two.
    expect(printedBox(WcAppShell, '.content ::slotted(*)').display).toBe('block');
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
