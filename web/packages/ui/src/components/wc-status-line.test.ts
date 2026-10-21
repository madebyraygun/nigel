import { describe, it, expect, afterEach, vi } from 'vitest';
import './wc-status-line.js';
import { WcStatusLine } from './wc-status-line.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { describePrintHiding } from '../../preview/print-suite.js';
import { styleText } from '../../preview/controls-suite.js';
import preview from './wc-status-line.preview.js';

async function mount(props: Partial<WcStatusLine> = {}): Promise<WcStatusLine> {
  const el = document.createElement('wc-status-line');
  Object.assign(
    el,
    {
      bookName: 'Acme Consulting LLC',
      bookKey: 'acme',
      path: '~/Documents/Nigel/acme-consulting',
      version: '1.0',
      reviewCount: 84,
    },
    props,
  );
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

const text = (el: WcStatusLine): string => el.shadowRoot?.textContent?.replace(/\s+/g, ' ') ?? '';

describe('wc-status-line', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('names the book with its monogram and the folder it lives in', async () => {
    const el = await mount();
    const book = el.shadowRoot?.querySelector('.book');
    expect(book?.querySelector('wc-monogram')?.getAttribute('seed')).toBe('acme');
    expect(book?.textContent).toContain('Acme Consulting LLC');
    expect(el.shadowRoot?.querySelector('.path')?.textContent).toBe(
      '~/Documents/Nigel/acme-consulting',
    );
  });

  it('leaves the path as selectable text rather than a control', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('.path')?.tagName).toBe('SPAN');
    expect(styleText(WcStatusLine)).toMatch(/\.path\s*{[^}]*user-select:\s*text/);
  });

  it('says encrypted with a closed lock', async () => {
    const el = await mount({ encrypted: true });
    expect(text(el)).toContain('encrypted');
    expect(text(el)).not.toContain('not encrypted');
    expect(el.shadowRoot?.querySelector('wc-icon-lock')).not.toBeNull();
  });

  it('says not encrypted with an open lock', async () => {
    const el = await mount({ encrypted: false });
    expect(text(el)).toContain('not encrypted');
    expect(el.shadowRoot?.querySelector('wc-icon-lock-open')).not.toBeNull();
  });

  it('asks for the palette scoped to books from the book', async () => {
    const el = await mount();
    const spy = vi.fn();
    document.body.addEventListener('nc-palette-open', spy);
    el.shadowRoot?.querySelector<HTMLButtonElement>('.book')?.click();
    document.body.removeEventListener('nc-palette-open', spy);
    expect(spy.mock.calls[0][0].detail).toEqual({ scope: 'books' });
  });

  it('asks for the whole palette from the hint, spelling the chord it is given', async () => {
    const el = await mount({ shortcut: 'Ctrl K', keyShortcuts: 'Control+K' });
    const hint = [...(el.shadowRoot?.querySelectorAll('button') ?? [])].find((b) =>
      b.textContent?.includes('palette'),
    );
    expect(hint?.textContent?.replace(/\s+/g, ' ').trim()).toBe('Ctrl K palette');
    expect(hint?.getAttribute('aria-keyshortcuts')).toBe('Control+K');
    const spy = vi.fn();
    document.body.addEventListener('nc-palette-open', spy);
    hint?.click();
    document.body.removeEventListener('nc-palette-open', spy);
    expect(spy.mock.calls[0][0].detail).toEqual({ scope: 'all' });
  });

  it('shows the review count and goes to Review when pressed', async () => {
    const el = await mount({ reviewCount: 84 });
    const button = el.shadowRoot?.querySelector<HTMLButtonElement>('.review');
    expect(button?.textContent?.replace(/\s+/g, ' ').trim()).toBe('84 to review');
    const spy = vi.fn();
    el.addEventListener('nc-navigate', spy);
    button?.click();
    expect(spy.mock.calls[0][0].detail).toEqual({ id: 'review' });
    expect(spy.mock.calls[0][0].composed).toBe(true);
  });

  it.each([0, undefined])('draws no review segment for %s', async (count) => {
    const el = await mount({ reviewCount: count });
    expect(el.shadowRoot?.querySelector('.review')).toBeNull();
  });

  it('shows the version', async () => {
    const el = await mount({ version: '1.0' });
    expect(text(el)).toContain('v1.0');
  });

  it('does not double a leading v', async () => {
    const el = await mount({ version: 'v2.3.1' });
    expect(text(el)).toContain('v2.3.1');
    expect(text(el)).not.toContain('vv');
  });

  it('omits the version in a store build', async () => {
    const el = await mount({ version: '1.0', distribution: 'store' });
    expect(text(el)).not.toContain('v1.0');
  });

  it('keeps the version in any other distribution', async () => {
    const el = await mount({ version: '1.0', distribution: 'direct' });
    expect(text(el)).toContain('v1.0');
  });

  it('keeps the full folder in the path tooltip while showing the abbreviated one', async () => {
    const el = await mount({
      path: '~/Documents/Nigel/acme-consulting',
      fullPath: '/home/you/Documents/Nigel/acme-consulting',
    });
    const path = el.shadowRoot?.querySelector('.path');
    expect(path?.textContent).toBe('~/Documents/Nigel/acme-consulting');
    expect(path?.getAttribute('title')).toBe('/home/you/Documents/Nigel/acme-consulting');
  });

  it('draws no book segment before the book has a name', async () => {
    const el = await mount({ bookName: '' });
    expect(el.shadowRoot?.querySelector('.book')).toBeNull();
    expect(el.shadowRoot?.querySelector('.path')).not.toBeNull();
  });

  it('insets the focus ring so the panel does not clip it', () => {
    expect(styleText(WcStatusLine)).toMatch(
      /button\.segment:focus-visible\s*{[^}]*outline-offset:\s*-2px/,
    );
  });

  it('drops the version, then the palette hint, as the line narrows', () => {
    const css = styleText(WcStatusLine);
    const version = css.indexOf('@container (max-width: 760px)');
    const hint = css.indexOf('@container (max-width: 620px)');
    expect(css).toMatch(/@container \(max-width: 760px\)\s*{\s*\.version\s*{\s*display:\s*none/);
    expect(css).toMatch(/@container \(max-width: 620px\)\s*{\s*\.hint\s*{\s*display:\s*none/);
    expect(version).toBeGreaterThan(-1);
    expect(hint).toBeGreaterThan(version);
  });

  it('lets the lock label and the book name shrink with an ellipsis', async () => {
    const css = styleText(WcStatusLine);
    expect(css).toMatch(/\.lock \.label\s*{[^}]*text-overflow:\s*ellipsis/);
    expect(css).toMatch(/\.book-name\s*{[^}]*text-overflow:\s*ellipsis/);
  });

  it('shows only the lock icon when narrow, keeping the label for assistive tech and the tooltip', async () => {
    const el = await mount({ encrypted: true });
    expect(styleText(WcStatusLine)).toMatch(
      /@container \(max-width: 560px\)\s*{\s*\.lock \.label\s*{[^}]*clip-path:\s*inset\(50%\)/,
    );
    expect(el.shadowRoot?.querySelector('.lock .label')?.textContent).toBe('encrypted');
    expect(el.shadowRoot?.querySelector('.lock')?.getAttribute('title')).toBe('Encrypted');
  });

  it('stands down below the mobile breakpoint', () => {
    expect(styleText(WcStatusLine)).toMatch(
      /@media \(max-width: 48rem\)\s*{\s*:host\s*{\s*display:\s*none/,
    );
  });
});

describePreviewA11y(preview);

describePrintHiding(WcStatusLine, ':host');
