import { describe, it, expect, afterEach, vi } from 'vitest';
import './wc-command-palette.js';
import {
  WcCommandPalette,
  type NcPaletteSelectDetail,
  type PaletteBook,
  type PaletteScreen,
} from './wc-command-palette.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { describePrintHiding } from '../../preview/print-suite.js';
import preview from './wc-command-palette.preview.js';

const BOOKS: PaletteBook[] = [
  {
    id: 'acme',
    name: 'Acme Consulting LLC',
    kind: 'Business',
    path: '~/Documents/Nigel/acme-consulting',
    encrypted: true,
    open: true,
  },
];

const SCREENS: PaletteScreen[] = [
  { id: 'dashboard', label: 'Dashboard', icon: 'wc-icon-dashboard' },
  { id: 'register', label: 'Register', icon: 'wc-icon-register' },
  { id: 'invoices', label: 'Invoices', icon: 'wc-icon-invoice' },
  { id: 'undo', label: 'Undo last import', icon: 'wc-icon-undo' },
];

async function mount(props: Partial<WcCommandPalette> = {}): Promise<WcCommandPalette> {
  const el = document.createElement('wc-command-palette');
  Object.assign(el, { books: BOOKS, screens: SCREENS, ...props });
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

const input = (el: WcCommandPalette) =>
  el.shadowRoot?.querySelector('input') as HTMLInputElement;
const options = (el: WcCommandPalette) => [
  ...(el.shadowRoot?.querySelectorAll('[role="option"]') ?? []),
];
const labels = (el: WcCommandPalette) =>
  options(el).map((o) => o.textContent?.replace(/\s+/g, ' ').trim());

async function type(el: WcCommandPalette, text: string): Promise<void> {
  input(el).value = text;
  input(el).dispatchEvent(new Event('input'));
  await el.updateComplete;
}

async function press(el: WcCommandPalette, key: string): Promise<KeyboardEvent> {
  const event = new KeyboardEvent('keydown', { key, bubbles: true, composed: true, cancelable: true });
  input(el).dispatchEvent(event);
  await el.updateComplete;
  return event;
}

function listen<T>(name: string): T[] {
  const seen: T[] = [];
  document.addEventListener(name, (e) => seen.push((e as CustomEvent<T>).detail));
  return seen;
}

describe('wc-command-palette', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders nothing while closed', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('[role="dialog"]')).toBeNull();
  });

  it('is a modal dialog around a combobox that owns the listbox', async () => {
    const el = await mount({ open: true });
    const dialog = el.shadowRoot?.querySelector('[role="dialog"]');
    expect(dialog?.getAttribute('aria-modal')).toBe('true');
    expect(input(el).getAttribute('role')).toBe('combobox');
    expect(input(el).getAttribute('aria-controls')).toBe('results');
    expect(el.shadowRoot?.getElementById('results')?.getAttribute('role')).toBe('listbox');
  });

  it('lists the open book first, then every screen', async () => {
    const el = await mount({ open: true });
    const headings = [...(el.shadowRoot?.querySelectorAll('.heading') ?? [])].map(
      (h) => h.textContent,
    );
    expect(headings).toEqual(['Books', 'Go to']);
    expect(options(el)).toHaveLength(1 + SCREENS.length);
    expect(labels(el)[0]).toContain('Acme Consulting LLC');
    expect(labels(el)[0]).toContain('Open');
    expect(labels(el)[0]).toContain('Business · ~/Documents/Nigel/acme-consulting');
    expect(labels(el)[0]).toContain('Encrypted');
    expect(labels(el)[0]).toContain('Open now');
    expect(labels(el)).toContain('Undo last importscreen');
  });

  it('shows only the books when scoped to them', async () => {
    const el = await mount({ open: true, scope: 'books' });
    expect(options(el)).toHaveLength(1);
    expect(input(el).placeholder).toBe('Search books…');
  });

  it('focuses the input when it opens', async () => {
    const el = await mount({ open: true });
    expect(el.shadowRoot?.activeElement).toBe(input(el));
  });

  it('gives focus back to whatever opened it, and opens empty next time', async () => {
    const opener = document.createElement('button');
    document.body.appendChild(opener);
    opener.focus();
    const el = await mount({ open: true });
    await type(el, 'inv');
    el.open = false;
    await el.updateComplete;
    expect(document.activeElement).toBe(opener);
    el.open = true;
    await el.updateComplete;
    expect(input(el).value).toBe('');
  });

  it.each([
    ['a screen label', 'REGIST', ['Register']],
    ['the book name', 'acme', ['Acme Consulting LLC']],
    ['the book kind', 'business', ['Acme Consulting LLC']],
    ['the book path', 'documents/nigel', ['Acme Consulting LLC']],
  ])('matches %s case-insensitively', async (_what, query, expected) => {
    const el = await mount({ open: true });
    await type(el, query);
    expect(options(el).map((o) => o.textContent)).toEqual(
      expected.map((label) => expect.stringContaining(label)),
    );
  });

  it('names the query when nothing matches', async () => {
    const el = await mount({ open: true });
    await type(el, 'zebra');
    expect(options(el)).toHaveLength(0);
    expect(el.shadowRoot?.querySelector('.empty')?.textContent).toBe('Nothing matches "zebra".');
    expect(el.shadowRoot?.querySelector('[role="status"]')?.textContent).toBe(
      'Nothing matches "zebra".',
    );
    expect(input(el).getAttribute('aria-expanded')).toBe('false');
    expect(input(el).hasAttribute('aria-activedescendant')).toBe(false);
  });

  it('moves through books and screens as one list, wrapping at the ends', async () => {
    const el = await mount({ open: true });
    const active = () => input(el).getAttribute('aria-activedescendant');
    expect(active()).toBe('option-0');
    await press(el, 'ArrowDown');
    expect(active()).toBe('option-1');
    expect(options(el)[1].getAttribute('aria-selected')).toBe('true');
    expect(options(el)[0].getAttribute('aria-selected')).toBe('false');
    await press(el, 'ArrowUp');
    await press(el, 'ArrowUp');
    expect(active()).toBe(`option-${SCREENS.length}`);
  });

  it('starts the highlight over when the query changes', async () => {
    const el = await mount({ open: true });
    await press(el, 'ArrowDown');
    await type(el, 'e');
    expect(input(el).getAttribute('aria-activedescendant')).toBe('option-0');
  });

  it('opens the highlighted row on Enter', async () => {
    const el = await mount({ open: true });
    const picks = listen<NcPaletteSelectDetail>('nc-palette-select');
    await press(el, 'Enter');
    await press(el, 'ArrowDown');
    await press(el, 'ArrowDown');
    await press(el, 'Enter');
    expect(picks).toEqual([
      { kind: 'book', id: 'acme' },
      { kind: 'screen', id: 'register' },
    ]);
  });

  it('opens a row on click', async () => {
    const el = await mount({ open: true });
    const picks = listen<NcPaletteSelectDetail>('nc-palette-select');
    (options(el)[3] as HTMLElement).click();
    expect(picks.at(-1)).toEqual({ kind: 'screen', id: 'invoices' });
  });

  it('asks to close on Escape and on the backdrop', async () => {
    const el = await mount({ open: true });
    const spy = vi.fn();
    document.addEventListener('nc-palette-close', spy);
    const escape = await press(el, 'Escape');
    (el.shadowRoot?.querySelector('.backdrop') as HTMLElement).click();
    document.removeEventListener('nc-palette-close', spy);
    expect(spy).toHaveBeenCalledTimes(2);
    expect(escape.defaultPrevented).toBe(true);
  });

  it('keeps focus in the input when the rest of the panel is pressed', async () => {
    const el = await mount({ open: true });
    const press = (target: Element | null | undefined) => {
      const event = new MouseEvent('mousedown', { bubbles: true, composed: true, cancelable: true });
      target?.dispatchEvent(event);
      return event.defaultPrevented;
    };
    const root = el.shadowRoot!;
    expect(press(root.querySelector('.heading'))).toBe(true);
    expect(press(root.querySelector('footer'))).toBe(true);
    expect(press(root.querySelector('[role="option"]'))).toBe(true);
    expect(press(root.querySelector('.results'))).toBe(true);
    expect(press(input(el))).toBe(false);
  });

  it('answers keys wherever in the panel they arrive', async () => {
    const el = await mount({ open: true });
    const spy = vi.fn();
    document.addEventListener('nc-palette-close', spy);
    el.shadowRoot
      ?.querySelector('footer')
      ?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    document.removeEventListener('nc-palette-close', spy);
    expect(spy).toHaveBeenCalledOnce();
  });

  it('leaves Enter and the arrows to an IME mid-composition', async () => {
    const el = await mount({ open: true });
    const picks = listen<NcPaletteSelectDetail>('nc-palette-select');
    const compose = (init: KeyboardEventInit) =>
      input(el).dispatchEvent(
        new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init }),
      );
    compose({ key: 'Enter', isComposing: true });
    compose({ key: 'Enter', keyCode: 229 });
    compose({ key: 'ArrowDown', isComposing: true });
    await el.updateComplete;
    expect(picks).toEqual([]);
    expect(input(el).getAttribute('aria-activedescendant')).toBe('option-0');
  });

  it('returns focus to the opener it is given rather than what had focus', async () => {
    const opener = document.createElement('button');
    const other = document.createElement('button');
    document.body.append(opener, other);
    other.focus();
    const el = await mount({ open: true, opener });
    el.open = false;
    await el.updateComplete;
    expect(document.activeElement).toBe(opener);
  });

  it('keeps Tab inside the dialog', async () => {
    const el = await mount({ open: true });
    const tab = await press(el, 'Tab');
    expect(tab.defaultPrevented).toBe(true);
  });
});

describePreviewA11y(preview);

describePrintHiding(WcCommandPalette, ':host');
