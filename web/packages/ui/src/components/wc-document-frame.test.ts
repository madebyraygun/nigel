import { describe, it, expect, afterEach } from 'vitest';
import './wc-document-frame.js';
import { PREVIEW_SANDBOX, type WcDocumentFrame } from './wc-document-frame.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-document-frame.preview.js';

async function mount(props: Partial<WcDocumentFrame> = {}): Promise<WcDocumentFrame> {
  const el = document.createElement('wc-document-frame');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function frame(el: WcDocumentFrame): HTMLIFrameElement | null | undefined {
  return el.shadowRoot?.querySelector<HTMLIFrameElement>('[data-frame]');
}

describe('wc-document-frame', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('frames srcdoc without granting it the app origin', async () => {
    // The header the route sets does not apply to a document the parent
    // supplies, so the sandbox attribute is the whole of the containment.
    expect(PREVIEW_SANDBOX).not.toContain('allow-same-origin');
    expect(PREVIEW_SANDBOX).not.toContain('allow-scripts');

    const el = await mount({ srcdoc: '<h1>Invoice #1251</h1>' });
    expect(frame(el)?.getAttribute('srcdoc')).toContain('Invoice #1251');
    expect(frame(el)?.getAttribute('sandbox')).toBe(PREVIEW_SANDBOX);
    expect(frame(el)?.hasAttribute('src')).toBe(false);
  });

  it('frames a src when given one instead', async () => {
    const el = await mount({ src: 'about:blank' });
    expect(frame(el)?.getAttribute('src')).toBe('about:blank');
    expect(frame(el)?.getAttribute('sandbox')).toBe(PREVIEW_SANDBOX);
  });

  it('shows a spinner until the frame loads', async () => {
    const el = await mount({ src: 'about:blank' });
    expect(el.shadowRoot?.querySelector('[data-loading]')).toBeTruthy();

    frame(el)?.dispatchEvent(new Event('load'));
    await el.updateComplete;
    expect(el.shadowRoot?.querySelector('[data-loading]')).toBeNull();
  });

  it('brings the spinner back when the document changes', async () => {
    const el = await mount({ srcdoc: '<p>one</p>' });
    frame(el)?.dispatchEvent(new Event('load'));
    await el.updateComplete;
    expect(el.shadowRoot?.querySelector('[data-loading]')).toBeNull();

    el.srcdoc = '<p>two</p>';
    await el.updateComplete;
    expect(el.shadowRoot?.querySelector('[data-loading]')).toBeTruthy();
  });

  it('labels the frame for a screen reader', async () => {
    const el = await mount({ srcdoc: '<p>x</p>', label: 'Invoice preview' });
    expect(frame(el)?.getAttribute('title')).toBe('Invoice preview');

    const unlabelled = await mount({ srcdoc: '<p>x</p>' });
    expect(frame(unlabelled)?.getAttribute('title')).toBe('Document preview');
  });

  it('takes its height from the caller', async () => {
    const el = await mount({ srcdoc: '<p>x</p>', height: '16rem' });
    const box = el.shadowRoot?.querySelector<HTMLElement>('.frame');
    expect(box?.getAttribute('style')).toContain('16rem');
  });
});

describePreviewA11y(preview);
