import { describe, it, expect, afterEach, beforeEach } from 'vitest';
import './documents.js';
import type { NigelDocumentsScreen } from './documents.js';
import type { WcDocumentTable, WcDropzone } from '@nigel/ui';

import { ApiError } from '../api/index.js';
import {
  conflictError,
  documentFlags,
  FakeApiClient,
} from '../__mocks__/fake-api-client.js';
import { initializeAppStore, resetAppStore } from '../state/app-store.js';
import type { Client, DocumentDetail, DocumentListRow } from '../api/types.js';
import type { ScreenId } from './registry.js';

const CEDAR: Client = {
  id: 1,
  name: 'Cedar Systems',
  email: 'pat@cedar.test',
  billingAddress: null,
  notes: null,
  archivedAt: null,
};

const JUNIPER: Client = {
  id: 2,
  name: 'Juniper Labs',
  email: 'sam@juniper.test',
  billingAddress: null,
  notes: null,
  archivedAt: null,
};

const HARBOR: Client = {
  id: 3,
  name: 'Harbor & Vale',
  email: null,
  billingAddress: null,
  notes: null,
  archivedAt: '2026-09-01',
};

const ROWS: DocumentListRow[] = [
  {
    id: 12,
    title: 'Website redesign proposal',
    kind: 'Proposal',
    clientId: 1,
    clientName: 'Cedar Systems',
    status: 'sent',
    latestVersion: 2,
    sentAt: '2026-10-01',
    updatedAt: '2026-10-01',
  },
  {
    id: 11,
    title: 'Support agreement',
    kind: 'Agreement',
    clientId: 2,
    clientName: 'Juniper Labs',
    status: 'draft',
    latestVersion: 1,
    sentAt: null,
    updatedAt: '2026-09-20',
  },
];

function client(): FakeApiClient {
  const fake = new FakeApiClient();
  fake.clients = [CEDAR, JUNIPER, HARBOR];
  fake.documents = ROWS;
  return fake;
}

async function settle(el: NigelDocumentsScreen): Promise<void> {
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
}

interface Mounted {
  el: NigelDocumentsScreen;
  fake: FakeApiClient;
  routes: { screen: ScreenId; params: string }[];
}

async function mount(query = '', fake: FakeApiClient = client()): Promise<Mounted> {
  const store = initializeAppStore(fake, { reload: () => {} });
  await store.refreshStatus();

  const routes: { screen: ScreenId; params: string }[] = [];
  const el = document.createElement('nigel-documents-screen');
  el.client = fake;
  el.params = new URLSearchParams(query);
  el.navigate = (screen, params) => {
    routes.push({ screen, params: params?.toString() ?? '' });
  };
  document.body.appendChild(el);
  await settle(el);
  return { el, fake, routes };
}

function find<T extends Element = HTMLElement>(el: NigelDocumentsScreen, hook: string): T {
  const found = el.shadowRoot?.querySelector<T>(hook);
  if (!found) throw new Error(`no ${hook} on screen`);
  return found;
}

function pick(control: Element, value: string): void {
  (control as HTMLElement & { value: string }).value = value;
  control.dispatchEvent(new Event('change', { bubbles: true, composed: true }));
}

async function fillFiling(el: NigelDocumentsScreen): Promise<void> {
  find(el, 'wc-dropzone').dispatchEvent(
    new CustomEvent('nc-file-select', {
      detail: { file: new File(['%PDF-1.7'], 'proposal.pdf', { type: 'application/pdf' }) },
    }),
  );
  await settle(el);
  pick(find(el, '[data-file-client]'), '1');
  pick(find(el, '[data-file-kind]'), 'Proposal');
  const title = find(el, '[data-file-title]') as HTMLElement & { value: string };
  title.value = 'Phase two proposal';
  title.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
  await settle(el);
}

describe('nigel-documents-screen', () => {
  beforeEach(() => {
    resetAppStore();
  });

  afterEach(() => {
    document.body.innerHTML = '';
    resetAppStore();
  });

  it('lists documents with their status', async () => {
    const { el, fake } = await mount();
    expect(fake.calls).toContain('getDocuments:');

    const rows = find<WcDocumentTable>(el, 'wc-document-table').rows;
    expect(rows.map((row) => [row.id, row.status, row.clientName])).toEqual([
      [12, 'sent', 'Cedar Systems'],
      [11, 'draft', 'Juniper Labs'],
    ]);
    expect(rows[0].href).toBe('#/documents?id=12');
  });

  it('filters are links', async () => {
    const { el, fake, routes } = await mount('status=sent');
    expect(fake.calls).toContain('getDocuments:status=sent');

    const chips = [...(el.shadowRoot?.querySelectorAll('[data-filter]') ?? [])];
    expect(chips.map((chip) => chip.getAttribute('href'))).toEqual([
      '#/documents',
      '#/documents?status=draft',
      '#/documents?status=sent',
      '#/documents?status=changes_requested',
      '#/documents?status=accepted',
      '#/documents?status=declined',
      '#/documents?status=executed',
      '#/documents?status=withdrawn',
    ]);
    expect(
      chips.find((chip) => chip.getAttribute('data-filter') === 'sent')?.getAttribute(
        'aria-current',
      ),
    ).toBe('true');

    pick(find(el, '[data-client-filter]'), '2');
    expect(routes).toEqual([{ screen: 'documents', params: 'status=sent&clientId=2' }]);
  });

  it('narrows to one client from ?clientId', async () => {
    const { fake } = await mount('clientId=2');
    expect(fake.calls).toContain('getDocuments:clientId=2');
  });

  it('a failed load is not a refused action', async () => {
    const failing = client();
    failing.documentsError = new ApiError({
      code: 'internal',
      rawCode: 'internal',
      message: 'The database is busy.',
      status: 500,
    });
    const failed = await mount('', failing);
    expect(failed.el.shadowRoot?.querySelector('wc-empty-state')).toBeTruthy();
    expect(failed.el.shadowRoot?.textContent).toContain('The database is busy.');
    expect(failed.el.shadowRoot?.querySelector('[data-action-error]')).toBeNull();
    expect(failed.el.shadowRoot?.querySelector('[data-reset]')?.getAttribute('href')).toBe(
      '#/documents',
    );

    document.body.innerHTML = '';
    resetAppStore();

    const refusing = client();
    refusing.syncDocumentsError = conflictError('sync_not_configured', {
      missing: ['r2_private_bucket'],
    });
    const { el } = await mount('', refusing);
    find(el, '[data-sync]').click();
    await settle(el);

    expect(find(el, '[data-action-error]').getAttribute('message')).toBe(
      'Syncing responses needs r2_private_bucket, which is not set.',
    );
    expect(el.shadowRoot?.querySelector('wc-empty-state')).toBeNull();
    expect(find<WcDocumentTable>(el, 'wc-document-table').rows).toHaveLength(2);
  });

  it('syncs responses and shows the report lines', async () => {
    const fake = client();
    fake.documentSyncResult = {
      documentsChecked: 1,
      recorded: 1,
      lines: [
        {
          documentId: 12,
          title: 'Website redesign proposal',
          recorded: ['Pat Example accepted version 2'],
          refused: [],
          warnings: [],
          status: 'accepted',
        },
      ],
      failures: [],
    };
    const { el } = await mount('', fake);

    find(el, '[data-sync]').click();
    await settle(el);

    expect(fake.calls).toContain('syncDocuments');
    expect(fake.calls.filter((call) => call.startsWith('getDocuments'))).toHaveLength(2);
    const report = find(el, '[data-sync-report]');
    expect(report.tagName.toLowerCase()).toBe('wc-notice-bar');
    expect(report.textContent).toContain(
      '#12 Website redesign proposal: Pat Example accepted version 2 → accepted',
    );
  });

  it('offers active clients and the server’s kinds for filing', async () => {
    const { el, fake } = await mount();
    expect(fake.calls).toContain('getDocumentKinds');

    const clients = [...find(el, '[data-file-client]').querySelectorAll('wa-option')]
      .map((option) => option.textContent?.trim())
      .filter((label) => label !== 'Choose a client…');
    expect(clients).toEqual(['Cedar Systems', 'Juniper Labs']);

    const kinds = [...find(el, '[data-file-kind]').querySelectorAll('wa-option')].map(
      (option) => option.getAttribute('value'),
    );
    expect(kinds).toEqual(['Proposal', 'Estimate', 'Agreement']);
  });

  it('filing a PDF creates a draft and opens it', async () => {
    const { el, fake, routes } = await mount();
    expect(find(el, '[data-file]').hasAttribute('disabled')).toBe(true);

    await fillFiling(el);
    expect(find<WcDropzone>(el, 'wc-dropzone').filename).toBe('proposal.pdf');
    expect(find(el, '[data-file]').hasAttribute('disabled')).toBe(false);

    find(el, '[data-file]').click();
    await settle(el);

    expect(fake.calls).toContain(
      `createDocument:${JSON.stringify({
        clientId: 1,
        kind: 'Proposal',
        title: 'Phase two proposal',
        file: 'proposal.pdf',
      })}`,
    );
    expect(routes).toEqual([{ screen: 'documents', params: 'id=700' }]);
  });

  it('a duplicate filing shows the guardrail sentence', async () => {
    const fake = client();
    fake.createDocumentError = conflictError('duplicate_document');
    const { el, routes } = await mount('', fake);

    await fillFiling(el);
    find(el, '[data-file]').click();
    await settle(el);

    expect(find(el, '[data-action-error]').getAttribute('message')).toBe(
      'This PDF is already filed for this client.',
    );
    expect(routes).toEqual([]);
    expect(find<WcDropzone>(el, 'wc-dropzone').filename).toBe('proposal.pdf');
  });

  it('the dropzone accepts only .pdf', async () => {
    const { el } = await mount();
    const dropzone = find<WcDropzone>(el, 'wc-dropzone');
    expect(dropzone.getAttribute('accept')).toBe('.pdf');

    dropzone.dispatchEvent(
      new CustomEvent('nc-file-error', { detail: { message: 'Not a PDF.' } }),
    );
    await settle(el);
    expect(dropzone.error).toBe('Not a PDF.');
  });

  it('opens a document from ?id', async () => {
    const fake = client();
    const detail: DocumentDetail = {
      id: 12,
      clientId: 1,
      kindId: 1,
      kind: 'Proposal',
      title: 'Website redesign proposal',
      declinedAt: null,
      declineNote: null,
      withdrawnAt: null,
      createdAt: '2026-09-28',
      updatedAt: '2026-10-01',
      status: 'sent',
      clientName: 'Cedar Systems',
      versions: [],
      ...documentFlags('sent'),
    };
    fake.documentDetails[12] = detail;
    const { el } = await mount('id=12', fake);

    expect(fake.calls).toContain('getDocument:12');
    expect(el.shadowRoot?.querySelector('h2')?.textContent).toContain(
      'Website redesign proposal',
    );
    expect(el.shadowRoot?.querySelector('[data-back]')?.getAttribute('href')).toBe(
      '#/documents',
    );
  });
});
