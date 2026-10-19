import { describe, it, expect, afterEach } from 'vitest';
import './wc-document-table.js';
import type { DocumentTableRow, WcDocumentTable } from './wc-document-table.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-document-table.preview.js';

const ROWS: DocumentTableRow[] = [
  {
    id: 12,
    title: 'Master services agreement',
    kind: 'contract',
    status: 'sent',
    clientName: 'Cedar Systems',
    version: 2,
    updatedAt: '2026-09-20',
    href: '#/documents?id=12',
  },
  {
    id: 7,
    title: 'Website proposal',
    kind: 'proposal',
    status: 'draft',
    clientName: null,
    version: 1,
    updatedAt: '2026-09-02',
  },
];

async function mount(props: Partial<WcDocumentTable> = {}): Promise<WcDocumentTable> {
  const el = document.createElement('wc-document-table');
  Object.assign(el, { rows: ROWS }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-document-table', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders one row per document, keyed by id', async () => {
    const el = await mount();
    const ids = [...(el.shadowRoot?.querySelectorAll('tr[data-row]') ?? [])].map((tr) =>
      tr.getAttribute('data-row'),
    );
    expect(ids).toEqual(['12', '7']);
  });

  it('links a title only when given an address', async () => {
    const el = await mount();
    const links = [...(el.shadowRoot?.querySelectorAll('a') ?? [])].map((a) => [
      a.getAttribute('href'),
      a.textContent?.trim(),
    ]);
    expect(links).toEqual([['#/documents?id=12', 'Master services agreement']]);
    const plain = el.shadowRoot?.querySelector('tr[data-row="7"] td');
    expect(plain?.textContent?.trim()).toBe('Website proposal');
  });

  it('shows an em dash for a missing client', async () => {
    const el = await mount();
    const cells = [...(el.shadowRoot?.querySelectorAll('tr[data-row="7"] td') ?? [])].map(
      (td) => td.textContent?.trim(),
    );
    expect(cells[3]).toBe('—');
  });

  it('lays out title, status, kind, client, version and updated', async () => {
    const el = await mount();
    const cells = [...(el.shadowRoot?.querySelectorAll('tr[data-row="12"] td') ?? [])];
    expect(cells[1].querySelector('wc-document-status')?.getAttribute('status')).toBe(
      'sent',
    );
    expect(cells.map((td, i) => (i === 1 ? '' : td.textContent?.trim()))).toEqual([
      'Master services agreement',
      '',
      'contract',
      'Cedar Systems',
      'v2',
      '2026-09-20',
    ]);
  });

  it('says it is loading before it says it is empty', async () => {
    const el = await mount({ rows: [], loading: true });
    expect(el.shadowRoot?.querySelector('wc-spinner')).toBeTruthy();
    expect(el.shadowRoot?.querySelector('[data-empty]')).toBeNull();
  });

  it('renders the empty message it was given', async () => {
    const el = await mount({ rows: [], emptyMessage: 'Nothing filed.' });
    expect(el.shadowRoot?.querySelector('[data-empty]')?.textContent).toContain(
      'Nothing filed.',
    );
  });
});

describePreviewA11y(preview);
