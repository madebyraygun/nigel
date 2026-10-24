import { describe, it, expect, afterEach } from 'vitest';
import type { CSSResult, LitElement } from 'lit';
import { stackedTableCss, STACKED_TABLE_MAX_WIDTH } from './stacked-table.js';
import { WcManagerTable } from './wc-manager-table.js';
import { WcInvoiceTable } from './wc-invoice-table.js';
import { WcDocumentTable } from './wc-document-table.js';

/**
 * Every list table stacks its rows into cards at a phone's width through the
 * one shared sheet, rather than each keeping its own copy of the layout.
 *
 * jsdom evaluates no container query, so what is checked is the contract the
 * sheet relies on: the sheet adopted after the component's own, a `.stack`
 * container around the table, one title cell per row, and a heading on every
 * cell the card shows as a labelled line.
 */
const TABLES: { ctor: typeof LitElement; tag: string; props: Record<string, unknown> }[] = [
  {
    ctor: WcManagerTable,
    tag: 'wc-manager-table',
    props: {
      caption: 'Accounts',
      columns: [
        { key: 'name', label: 'Name' },
        { key: 'type', label: 'Type' },
      ],
      rows: [{ id: 1, label: 'Operating', cells: ['Operating', 'Checking'] }],
      actions: [{ name: 'edit', label: 'Edit' }],
    },
  },
  {
    ctor: WcInvoiceTable,
    tag: 'wc-invoice-table',
    props: {
      rows: [
        {
          number: 1250,
          status: 'sent',
          clientName: 'Acme Co',
          total: 1200,
          balance: 1200,
          issueDate: '2026-10-01',
          dueDate: '2026-10-31',
          href: '#/invoices/1250',
        },
      ],
    },
  },
  {
    ctor: WcDocumentTable,
    tag: 'wc-document-table',
    props: {
      rows: [
        {
          id: 3,
          title: 'Website proposal',
          kind: 'proposal',
          status: 'draft',
          clientName: 'Juniper Labs',
          version: 1,
          updatedAt: '2026-09-28',
          href: '#/documents?id=3',
        },
      ],
    },
  },
];

async function mount(tag: string, props: Record<string, unknown>): Promise<LitElement> {
  const el = document.createElement(tag) as LitElement;
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('stacked list tables', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('queries the width it documents', () => {
    expect(String(stackedTableCss.cssText)).toContain(
      `@container (max-width: ${STACKED_TABLE_MAX_WIDTH}px)`,
    );
  });

  for (const { ctor, tag, props } of TABLES) {
    describe(tag, () => {
      it('adopts the shared sheet after its own', () => {
        const styles = (ctor.styles as unknown[]).flat(3) as CSSResult[];
        expect(styles.at(-1)).toBe(stackedTableCss);
      });

      it('wraps its table in the stacking container', async () => {
        const el = await mount(tag, props);
        expect(el.shadowRoot?.querySelector('.stack > table')).toBeTruthy();
      });

      it('titles each card once and labels every line under the title', async () => {
        const el = await mount(tag, props);
        const rows = [...(el.shadowRoot?.querySelectorAll('tbody tr') ?? [])];
        expect(rows.length).toBeGreaterThan(0);
        for (const row of rows) {
          const cells = [...row.querySelectorAll('td')];
          const roles = cells.map((td) => td.getAttribute('data-stack'));
          expect(roles.filter((role) => role === 'title')).toHaveLength(1);
          const unlabelled = cells.filter(
            (td) => td.getAttribute('data-stack') === null && !td.hasAttribute('data-label'),
          );
          expect(unlabelled).toEqual([]);
        }
      });

      it('keeps table roles on elements the stacked layout sets to block', async () => {
        const el = await mount(tag, props);
        const root = el.shadowRoot;
        expect(root?.querySelector('table')?.getAttribute('role')).toBe('table');
        expect(root?.querySelector('tbody tr')?.getAttribute('role')).toBe('row');
        expect(root?.querySelector('tbody td')?.getAttribute('role')).toBe('cell');
      });
    });
  }
});
