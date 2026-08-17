import { describe, it, expect, afterEach } from 'vitest';
import type { LitElement } from 'lit';
import './wc-invoice-preview.js';
import { PREVIEW_SANDBOX, type WcInvoicePreview } from './wc-invoice-preview.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-invoice-preview.preview.js';

/**
 * The iframe lives in `wc-document-frame` now — one iframe implementation and
 * one sandbox constant for the whole app — so a test that is about the iframe
 * reaches through the child it delegates to.
 */
async function innerFrame(el: WcInvoicePreview): Promise<Element | null | undefined> {
  const frame = el.shadowRoot?.querySelector('[data-frame]') as LitElement | null;
  await frame?.updateComplete;
  return frame?.shadowRoot?.querySelector('iframe');
}

async function mount(props: Partial<WcInvoicePreview> = {}): Promise<WcInvoicePreview> {
  const el = document.createElement('wc-invoice-preview');
  Object.assign(
    el,
    { src: 'about:blank', pdfSrc: 'about:blank#pdf' },
    props,
  );
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-invoice-preview', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('starts collapsed and frames nothing until it is opened', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('details')?.open).toBe(false);
    expect(el.shadowRoot?.querySelector('[data-frame]')).toBeNull();
  });

  it('frames the page once opened', async () => {
    const el = await mount({ open: true });
    const frame = await innerFrame(el);
    expect(frame).toBeTruthy();
    expect(frame?.getAttribute('src')).toBe('about:blank');
    expect(frame?.getAttribute('title')).toBe('Invoice preview');
  });

  it('never grants the frame allow-same-origin', async () => {
    // The document is served from the SPA's own origin; allow-same-origin
    // would hand a page rendered from invoice data the app's cookies and
    // storage, and there is nothing the preview needs them for.
    expect(PREVIEW_SANDBOX).not.toContain('allow-same-origin');
    expect(PREVIEW_SANDBOX).not.toContain('allow-scripts');

    const el = await mount({ open: true });
    const sandbox = (await innerFrame(el))?.getAttribute('sandbox');
    expect(sandbox).toBe(PREVIEW_SANDBOX);
  });

  it('names the unset keys without naming their values', async () => {
    const el = await mount({ open: true, missing: ['r2_bucket', 'public_base_url'] });
    const notice = el.shadowRoot?.querySelector('[data-missing]');
    expect(notice?.getAttribute('message')).toContain('r2_bucket, public_base_url');
    expect(notice?.getAttribute('message')).toContain('cannot be sent');
  });

  it('offers no PDF link on a build that cannot render one', async () => {
    const el = await mount({ open: true, pdfAvailable: false });
    expect(el.shadowRoot?.querySelector('[data-pdf-link]')).toBeNull();
    expect(el.shadowRoot?.querySelector('[data-pdf-unavailable]')?.textContent).toContain(
      'not available',
    );
  });

  it('offers both addresses when the build can render a PDF', async () => {
    const el = await mount({ open: true });
    expect(el.shadowRoot?.querySelector('[data-html-link]')).toBeTruthy();
    expect(el.shadowRoot?.querySelector('[data-pdf-link]')?.getAttribute('href')).toBe(
      'about:blank#pdf',
    );
  });

  it('renders the PDF target as a button when the target is an action', async () => {
    const run = async () => {};
    const el = await mount({ open: true, pdfTarget: { kind: 'action', run } });
    const link = el.shadowRoot?.querySelector('[data-pdf-link]');
    expect(link?.tagName).toBe('BUTTON');
    expect(link?.textContent?.trim()).toBe('Download the PDF');
  });

  it('runs the action when the PDF button is clicked', async () => {
    let ran = 0;
    const el = await mount({
      open: true,
      pdfTarget: {
        kind: 'action',
        run: async () => {
          ran += 1;
        },
      },
    });

    el.shadowRoot?.querySelector<HTMLButtonElement>('[data-pdf-link]')?.click();
    await el.updateComplete;

    expect(ran).toBe(1);
  });

  it('does not leave a rejected save as an unhandled rejection', async () => {
    const rejections: unknown[] = [];
    const onRejection = (reason: unknown) => rejections.push(reason);
    process.on('unhandledRejection', onRejection);

    try {
      const el = await mount({
        open: true,
        pdfTarget: { kind: 'action', run: () => Promise.reject(new Error('save failed')) },
      });

      el.shadowRoot?.querySelector<HTMLButtonElement>('[data-pdf-link]')?.click();
      await el.updateComplete;

      // Node reports an unhandled rejection on a later tick, once the
      // microtask queue that could still attach a handler has drained.
      await new Promise((resolve) => setTimeout(resolve, 0));
      await new Promise((resolve) => setTimeout(resolve, 0));
    } finally {
      process.off('unhandledRejection', onRejection);
    }

    expect(rejections).toEqual([]);
  });

  it('toasts a rejected PDF save rather than leaving the operator guessing', async () => {
    const toasts: string[] = [];
    const onToast = (event: Event) =>
      toasts.push((event as CustomEvent<{ message: string }>).detail.message);
    window.addEventListener('nc-toast', onToast);

    try {
      const el = await mount({
        open: true,
        pdfTarget: { kind: 'action', run: () => Promise.reject(new Error('disk full')) },
      });

      el.shadowRoot?.querySelector<HTMLButtonElement>('[data-pdf-link]')?.click();
      await el.updateComplete;
      await new Promise((resolve) => setTimeout(resolve, 0));
    } finally {
      window.removeEventListener('nc-toast', onToast);
    }

    expect(toasts).toHaveLength(1);
    expect(toasts[0]).toContain("Couldn't save the PDF.");
    expect(toasts[0]).toContain('disk full');
  });

  it('prefers pdfTarget over pdfSrc when both are set', async () => {
    const el = await mount({
      open: true,
      pdfSrc: '/api/invoices/1251/preview.pdf',
      pdfTarget: { kind: 'href', href: '/api/invoices/1251/preview.pdf?desktop' },
    });
    expect(el.shadowRoot?.querySelector('[data-pdf-link]')?.getAttribute('href')).toBe(
      '/api/invoices/1251/preview.pdf?desktop',
    );
  });
});

describePreviewA11y(preview);
