import { describe, it, expect, afterEach } from 'vitest';
import { colorCss, colorDarkCss } from '@nigel/theme';
import './wc-invoice-status.js';
import {
  INVOICE_STATUS_WORDS,
  WcInvoiceStatus,
} from './wc-invoice-status.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { iconSvg } from '../__tests__/settle.js';
import preview from './wc-invoice-status.preview.js';

async function mount(status: string): Promise<WcInvoiceStatus> {
  const el = document.createElement('wc-invoice-status');
  el.status = status;
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

/** Every colour custom property the chip reads, in source order. */
function colorTokensUsed(): string[] {
  const css = [WcInvoiceStatus.styles].flat().map(String).join('\n');
  return [...css.matchAll(/(?:^|\s)color:\s*var\(([^)]*)\)/gm)].map((match) =>
    match[1].trim(),
  );
}

describe('the status chip’s colours', () => {
  const theme = `${colorCss}\n${colorDarkCss}`;

  it('names only tokens @nigel/theme defines, in both schemes', () => {
    // The bug this catches: `--nc-color-warning` does not exist, so its
    // literal fallback always won — a colour the theme's contrast test could
    // not see and never held to AA.
    const used = colorTokensUsed();
    expect(used.length).toBeGreaterThan(0);

    for (const token of used) {
      expect(theme, `${token} is not defined by @nigel/theme`).toContain(`${token}:`);
      expect(colorDarkCss.toString(), `${token} has no dark value`).toContain(
        `${token}:`,
      );
    }
  });

  it('carries no literal fallback that could win over a token', () => {
    // A fallback only ever renders when the token is missing, which is exactly
    // the case the contrast test cannot reach.
    expect(colorTokensUsed().filter((token) => token.includes(','))).toEqual([]);
  });

  it('reads partial as flagged rather than inventing a warning colour', () => {
    expect(colorTokensUsed()).toContain('--nc-color-flagged');
  });
});

describe('wc-invoice-status', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('knows the six statuses the data layer derives', () => {
    expect(INVOICE_STATUS_WORDS).toEqual([
      'draft',
      'sent',
      'partial',
      'paid',
      'overdue',
      'void',
    ]);
  });

  it.each(INVOICE_STATUS_WORDS)('renders %s as an icon and the word', async (status) => {
    const el = await mount(status);
    const chip = el.shadowRoot?.querySelector('.chip');
    expect(chip?.getAttribute('data-status')).toBe(status);
    expect(chip?.querySelector('.word')?.textContent).toBe(status);
    expect(chip?.querySelector('.mark')?.tagName.toLowerCase()).toBe(
      `wc-icon-status-${status}`,
    );
  });

  it('draws the mark as an SVG, not as a character the mono face lacks', async () => {
    // None of ◻ ◆ ◑ ● ▲ ⊘ is in IBM Plex Mono, so as text each one came from
    // whatever fallback face the browser found.
    const el = await mount('paid');
    const mark = el.shadowRoot?.querySelector('.mark');
    const svg = await iconSvg(mark);

    expect(svg.querySelectorAll('path').length).toBeGreaterThan(0);
    expect(mark?.textContent?.trim()).toBe('');
    // Decoration: the word is what a screen reader announces.
    expect(svg.getAttribute('aria-hidden')).toBe('true');
  });

  it('sizes the mark to the text it sits beside and lets it take the chip’s colour', async () => {
    // The chip declares no size of its own: `inline` is WcIconBase's own 1em
    // mode, so every mark in the app that sits in text asks for it the same way.
    const css = [WcInvoiceStatus.styles].flat().map(String).join('\n');
    expect(css).not.toContain('--nc-icon-size');

    const el = await mount('overdue');
    const mark = el.shadowRoot?.querySelector('.mark');
    expect(mark?.hasAttribute('inline')).toBe(true);
    // WcIconBase inherits currentColor, so a status keeps its own colour with
    // nothing per-status to declare.
    expect((await iconSvg(mark)).getAttribute('stroke')).toBe('currentColor');
  });

  it('renders a status it has never seen rather than blanking it', async () => {
    // `invoices.status` has no CHECK constraint, so a row written by the
    // InvoiceShelf importer or by hand cannot be assumed to be one of the six.
    const el = await mount('imported');
    expect(el.shadowRoot?.querySelector('.word')?.textContent).toBe('imported');
    expect(el.shadowRoot?.querySelector('.mark')?.tagName.toLowerCase()).toBe(
      'wc-icon-dot',
    );
  });

  it('keeps the mark it already drew when the chip re-renders', async () => {
    // A row in a list re-renders whenever anything around it changes. The mark
    // is a template, so Lit updates it in place; rebuilding the element would
    // re-upgrade a custom element the operator is looking at.
    const el = await mount('paid');
    const first = el.shadowRoot?.querySelector('.mark');

    el.requestUpdate();
    await el.updateComplete;
    expect(el.shadowRoot?.querySelector('.mark')).toBe(first);

    el.status = 'void';
    await el.updateComplete;
    expect(el.shadowRoot?.querySelector('.mark')?.tagName.toLowerCase()).toBe(
      'wc-icon-status-void',
    );
  });

  it.each(['constructor', 'toString', 'hasOwnProperty', '__proto__'])(
    'renders the neutral mark for the inherited property name %s',
    async (status) => {
      // The bug this catches: an object lookup answers `constructor` with a
      // function, and `document.createElement(fn)` throws — taking down the
      // chip and the Lit update of every invoice row beside it, where the
      // character it replaced simply fell through to the neutral mark.
      const el = await mount(status);
      expect(el.shadowRoot?.querySelector('.word')?.textContent).toBe(status);
      expect(el.shadowRoot?.querySelector('.mark')?.tagName.toLowerCase()).toBe(
        'wc-icon-dot',
      );
    },
  );
});

describePreviewA11y(preview);
