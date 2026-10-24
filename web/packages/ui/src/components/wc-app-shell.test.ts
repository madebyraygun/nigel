import { describe, it, expect, afterEach } from 'vitest';
import './wc-app-shell.js';
import { WcAppShell, type NarrowQuery } from './wc-app-shell.js';
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

  it('renders the screen title as the page heading', async () => {
    const el = await mount({ screenTitle: 'Register' });
    const heading = el.shadowRoot?.querySelector('h1');
    expect(heading?.textContent).toBe('Register');
    expect(heading?.closest('main')).toBeTruthy();
  });

  it('keeps a hidden title as a heading for assistive technology, outside the page head', async () => {
    const el = await mount({ screenTitle: 'Register', titleHidden: true });
    const heading = el.shadowRoot?.querySelector('h1');
    expect(heading?.textContent).toBe('Register');
    expect(heading?.classList.contains('title-hidden')).toBe(true);
    expect(heading?.closest('main')).toBeTruthy();
    expect(el.shadowRoot?.querySelector('[part="page-head"]')).toBeNull();
  });

  it('draws no page heading for a screen that brings its own', async () => {
    const el = await mount({ screenTitle: '' });
    expect(el.shadowRoot?.querySelector('h1')).toBeNull();
  });

  it('exposes the sidebar, page-actions, banner and default slots', async () => {
    const el = await mount({ screenTitle: 'Dashboard' });
    const names = [...(el.shadowRoot?.querySelectorAll('slot') ?? [])].map((s) =>
      s.getAttribute('name'),
    );
    expect(names).toContain('sidebar');
    expect(names).toContain('page-actions');
    expect(names).toContain('banner');
    expect(names).toContain(null);
  });

  it('pushes a page action to the right of the title row', () => {
    expect(text).toMatch(
      /\.page-head ::slotted\(\[slot='page-actions'\]\)\s*{[^}]*flex:\s*none;[^}]*margin-left:\s*auto/,
    );
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
    expect(parts).toEqual(['sidebar', 'header', 'nav-toggle', 'banner', 'content']);
  });

  it('exposes the page heading as a part when it draws one', async () => {
    const el = await mount({ screenTitle: 'Dashboard' });
    expect(el.shadowRoot?.querySelector('[part="page-head"]')).toBeTruthy();
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

describePrintHiding(
  WcAppShell,
  'header',
  '.banner',
  '.page-head',
  "::slotted([slot='sidebar'])",
);

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

  it('keeps every positioned descendant inside the scroller', () => {
    // A visually hidden label or a radio under its card is absolutely
    // positioned. Unless the content area is its containing block, the
    // window is, the area's clip does not reach it, and one below the fold
    // makes the document taller than the window — which then scrolls, frame
    // and all, under wheel, focus and scrollIntoView.
    expect(text).toMatch(/\.content\s*{[^}]*position:\s*relative/);
  });

  it('does not hand a scroll that reaches its end on to the page', () => {
    expect(text).toMatch(/\.content\s*{[^}]*overscroll-behavior:\s*contain/);
  });

  it('is exactly the visible window, toolbars included', () => {
    expect(text).toMatch(/:host\s*{[^}]*height:\s*100vh;\s*height:\s*100dvh/);
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

/**
 * jsdom's `matchMedia` answers `false` to everything, which is the wide
 * viewport. A narrow one has to be handed in, the way `color-mode.ts` hands in
 * its dark-mode query.
 */
function viewport(narrow: boolean): NarrowQuery {
  return {
    matches: narrow,
    addEventListener: () => {},
    removeEventListener: () => {},
  };
}

function toggleOf(el: WcAppShell): HTMLButtonElement | null {
  return el.shadowRoot?.querySelector('.nav-toggle') ?? null;
}

describe('wc-app-shell sidebar toggle', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('shows the header toggle only at phone width', () => {
    // Above the breakpoint the hamburger is in the sidebar's own header; two
    // would be one too many.
    expect(text).toMatch(/\.nav-toggle\s*{[^}]*display:\s*none/);
    expect(text).toMatch(/@media \(max-width: 48rem\)[\s\S]*\.nav-toggle\s*{[^}]*display:\s*inline-grid/);
  });

  it('puts a labelled toggle in the header', async () => {
    const el = await mount();
    const toggle = toggleOf(el);
    expect(toggle).toBeTruthy();
    expect(toggle?.getAttribute('aria-label')).toBeTruthy();
  });

  it('says whether the sidebar is showing', async () => {
    // The control is the only thing on screen that can say so once the sidebar
    // is off-canvas, and a button whose label never changes leaves a screen
    // reader with no way to know which way it will go.
    const el = await mount({ sidebarCollapsed: false });
    expect(toggleOf(el)?.getAttribute('aria-expanded')).toBe('true');

    el.sidebarCollapsed = true;
    await el.updateComplete;
    expect(toggleOf(el)?.getAttribute('aria-expanded')).toBe('false');
  });

  it('asks for the sidebar to be put away when it is showing', async () => {
    const el = await mount({ sidebarCollapsed: false });
    let asked: boolean | undefined;
    el.addEventListener('nc-sidebar-toggle', (e) => {
      asked = e.detail.collapsed;
    });

    toggleOf(el)?.click();

    expect(asked).toBe(true);
  });

  it('asks for the sidebar back when it is put away', async () => {
    const el = await mount({ sidebarCollapsed: true });
    let asked: boolean | undefined;
    el.addEventListener('nc-sidebar-toggle', (e) => {
      asked = e.detail.collapsed;
    });

    toggleOf(el)?.click();

    expect(asked).toBe(false);
  });

  it('does not decide for itself whether the sidebar is showing', async () => {
    // nigel-app owns the state: the sidebar is its slotted child and only it
    // can pass `collapsed` down. A shell that also flipped its own property
    // would be a second source of truth for one boolean.
    const el = await mount({ sidebarCollapsed: false });
    toggleOf(el)?.click();
    await el.updateComplete;
    expect(el.sidebarCollapsed).toBe(false);
  });
});

describe('wc-app-shell drawer', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('covers the content while the drawer is open', async () => {
    const el = await mount({ narrowQuery: viewport(true), sidebarCollapsed: false });
    expect(el.shadowRoot?.querySelector('.backdrop')).toBeTruthy();
  });

  it('leaves the content alone once the drawer is away', async () => {
    const el = await mount({ narrowQuery: viewport(true), sidebarCollapsed: true });
    expect(el.shadowRoot?.querySelector('.backdrop')).toBeNull();
  });

  it('draws no backdrop over a docked sidebar', async () => {
    // On a wide viewport the sidebar takes its own column and covers nothing,
    // so a backdrop would grey out the app for no reason.
    const el = await mount({ narrowQuery: viewport(false), sidebarCollapsed: false });
    expect(el.shadowRoot?.querySelector('.backdrop')).toBeNull();
  });

  it('closes the drawer when the backdrop is pressed', async () => {
    const el = await mount({ narrowQuery: viewport(true), sidebarCollapsed: false });
    let asked: boolean | undefined;
    el.addEventListener('nc-sidebar-toggle', (e) => {
      asked = e.detail.collapsed;
    });

    el.shadowRoot?.querySelector<HTMLElement>('.backdrop')?.click();

    expect(asked).toBe(true);
  });

  it('closes the drawer on Escape', async () => {
    const el = await mount({ narrowQuery: viewport(true), sidebarCollapsed: false });
    let asked: boolean | undefined;
    el.addEventListener('nc-sidebar-toggle', (e) => {
      asked = e.detail.collapsed;
    });

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));

    expect(asked).toBe(true);
  });

  it('closes the drawer when a screen is chosen', async () => {
    // The drawer covers the screen it just navigated to.
    const el = await mount({ narrowQuery: viewport(true), sidebarCollapsed: false });
    const sidebar = document.createElement('div');
    sidebar.slot = 'sidebar';
    el.appendChild(sidebar);
    let asked: boolean | undefined;
    el.addEventListener('nc-sidebar-toggle', (e) => {
      asked = e.detail.collapsed;
    });

    sidebar.dispatchEvent(
      new CustomEvent('nc-navigate', {
        detail: { id: 'register' },
        bubbles: true,
        composed: true,
      }),
    );

    expect(asked).toBe(true);
  });

  it('leaves a docked sidebar alone when a screen is chosen', async () => {
    // Navigating on a wide viewport must not collapse the sidebar to the rail:
    // it covers nothing, and every click would fold the nav away.
    const el = await mount({ narrowQuery: viewport(false), sidebarCollapsed: false });
    const sidebar = document.createElement('div');
    sidebar.slot = 'sidebar';
    el.appendChild(sidebar);
    let asked: boolean | undefined;
    el.addEventListener('nc-sidebar-toggle', (e) => {
      asked = e.detail.collapsed;
    });

    sidebar.dispatchEvent(
      new CustomEvent('nc-navigate', {
        detail: { id: 'register' },
        bubbles: true,
        composed: true,
      }),
    );

    expect(asked).toBeUndefined();
  });

  it('gives focus back to the toggle when the drawer closes', async () => {
    // The drawer took focus into itself; returning it to the control that
    // opened it is what lets a keyboard user carry on where they were.
    const el = await mount({ narrowQuery: viewport(true), sidebarCollapsed: false });

    el.sidebarCollapsed = true;
    await el.updateComplete;

    expect(el.shadowRoot?.activeElement).toBe(toggleOf(el));
  });

  it('does not chase focus on a wide viewport', async () => {
    // Collapsing to the rail there is not a dismissal, and stealing focus
    // would pull a keyboard user out of the screen they were working in.
    const el = await mount({ narrowQuery: viewport(false), sidebarCollapsed: false });

    el.sidebarCollapsed = true;
    await el.updateComplete;

    expect(el.shadowRoot?.activeElement).toBeNull();
  });
});

describe('wc-app-shell on a phone', () => {
  it('takes the sidebar out of the flow and lays it over the content', () => {
    // Docked, the sidebar is 224px of a 390px viewport and the content is left
    // with a 40-character column. jsdom has no layout engine, so the rules are
    // read the way the register's fill rules are.
    expect(text).toMatch(
      /@media \(max-width: 48rem\)[\s\S]*::slotted\(\[slot='sidebar'\]\)[^{]*{[^}]*position:\s*fixed/,
    );
  });

  it('slides the drawer off-canvas when the sidebar is put away', () => {
    expect(text).toMatch(
      /@media \(max-width: 48rem\)[\s\S]*:host\(\[sidebar-collapsed\]\)[^{]*::slotted\(\[slot='sidebar'\]\)[^{]*{[^}]*transform:\s*translateX\(calc\(-100% - 2 \* var\(--nc-window-inset\)\)\)/,
    );
  });

  it('narrows the gutter so the cards get the width', () => {
    // 16px each side is a tenth of a phone's width spent on margin.
    expect(text).toMatch(/@media \(max-width: 48rem\)[\s\S]*\.content[^{]*{[^}]*padding:\s*10px/);
  });

  it('stops sliding for anyone who asked motion to stop', () => {
    expect(text).toMatch(
      /@media \(prefers-reduced-motion: reduce\)[\s\S]*::slotted\(\[slot='sidebar'\]\)[^{]*{[^}]*transition:\s*none/,
    );
  });
});

describe('wc-app-shell top bar', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('hands the book and the trail to the breadcrumb', async () => {
    const trail = [{ label: 'Invoices', id: 'invoices' }, { label: '#1254' }];
    const el = await mount({ bookName: 'Acme Consulting LLC', bookKey: 'acme', trail });
    const crumb = el.shadowRoot?.querySelector('wc-breadcrumb');
    expect(crumb?.bookName).toBe('Acme Consulting LLC');
    expect(crumb?.bookKey).toBe('acme');
    expect(crumb?.trail).toEqual(trail);
  });

  it('sits the breadcrumb, the search pill and refresh in the header', async () => {
    const el = await mount();
    const header = el.shadowRoot?.querySelector('header');
    expect(header?.querySelector('wc-breadcrumb')).toBeTruthy();
    expect(header?.querySelector('wc-search-pill')).toBeTruthy();
    expect(header?.querySelector('wa-button.refresh wc-icon-refresh[label="Refresh"]')).toBeTruthy();
  });

  it('asks for a refresh rather than doing one', async () => {
    const el = await mount();
    let asked = 0;
    el.addEventListener('nc-refresh', () => (asked += 1));
    el.shadowRoot?.querySelector<HTMLButtonElement>('wa-button.refresh')?.click();
    expect(asked).toBe(1);
  });

  it('passes the shortcut on to the search pill', async () => {
    const el = await mount({ searchShortcut: 'Ctrl K', searchKeyShortcuts: 'Control+K' });
    const pill = el.shadowRoot?.querySelector('wc-search-pill');
    expect(pill?.shortcut).toBe('Ctrl K');
    expect(pill?.keyShortcuts).toBe('Control+K');
  });
});

describe('wc-app-shell scroll', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('puts the content back at the top when the screen changes', async () => {
    const el = await mount({ scrollKey: 'dashboard' });
    const content = el.shadowRoot?.querySelector<HTMLElement>('.content');
    if (!content) throw new Error('no content area');
    content.scrollTop = 240;

    el.scrollKey = 'register';
    await el.updateComplete;

    expect(content.scrollTop).toBe(0);
  });

  it('leaves the scroll alone on any other update', async () => {
    const el = await mount({ scrollKey: 'dashboard' });
    const content = el.shadowRoot?.querySelector<HTMLElement>('.content');
    if (!content) throw new Error('no content area');
    content.scrollTop = 240;

    el.screenTitle = 'Dashboard, again';
    await el.updateComplete;

    expect(content.scrollTop).toBe(240);
  });
});

describe('wc-app-shell without a sidebar', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('hides the sidebar slot and the toggle that would open it', async () => {
    const el = await mount({ hideSidebar: true, narrowQuery: viewport(true) });
    expect(el.hasAttribute('hide-sidebar')).toBe(true);
    expect(toggleOf(el)).toBeNull();
    expect(text).toMatch(
      /:host\(\[hide-sidebar\]\) ::slotted\(\[slot='sidebar'\]\)\s*{[^}]*display:\s*none/,
    );
  });

  it('keeps the breadcrumb and the top bar', async () => {
    const el = await mount({ hideSidebar: true, bookName: 'Acme Consulting LLC' });
    expect(el.shadowRoot?.querySelector('header wc-breadcrumb')).toBeTruthy();
    expect(el.shadowRoot?.querySelector('header wc-search-pill')).toBeTruthy();
  });

  it('never draws the drawer backdrop', async () => {
    const el = await mount({
      hideSidebar: true,
      narrowQuery: viewport(true),
      sidebarCollapsed: false,
    });
    expect(el.shadowRoot?.querySelector('.backdrop')).toBeNull();
  });
});

describe('wc-app-shell status line', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  const info = { path: '~/Documents/Nigel/acme-consulting', encrypted: true, version: '1.0' };

  it('draws the status line below the content from what it is handed', async () => {
    const el = await mount({
      bookName: 'Acme Consulting LLC',
      statusLine: info,
      reviewCount: 7,
      searchShortcut: 'Ctrl K',
    });
    const line = el.shadowRoot?.querySelector('main + wc-status-line');
    expect(line?.getAttribute('path')).toBe(info.path);
    expect(line?.hasAttribute('encrypted')).toBe(true);
    expect(line?.getAttribute('book-name')).toBe('Acme Consulting LLC');
    expect(line?.getAttribute('shortcut')).toBe('Ctrl K');
    expect((line as unknown as { reviewCount: number }).reviewCount).toBe(7);
  });

  it('draws none when it is handed nothing, as in a hosted window', async () => {
    const el = await mount({ hideSidebar: true });
    expect(el.shadowRoot?.querySelector('wc-status-line')).toBeNull();
  });

  it('passes a store build through so the line can drop the version', async () => {
    const el = await mount({ statusLine: { ...info, distribution: 'store' } });
    expect(el.shadowRoot?.querySelector('wc-status-line')?.getAttribute('distribution')).toBe(
      'store',
    );
  });
});
