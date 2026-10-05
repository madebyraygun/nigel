import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import './documents.js';
import type { NigelDocumentsScreen } from './documents.js';
import type {
  WcDocumentTable,
  WcDocumentTimeline,
  WcDropzone,
  WcRecipientEditor,
  WcSendDialog,
} from '@nigel/ui';

import { ApiError } from '../api/index.js';
import {
  conflictError,
  documentFlags,
  DOCUMENTS_CONFIGURED,
  FakeApiClient,
  UNLOCKED_STATUS,
} from '../__mocks__/fake-api-client.js';
import { initializeAppStore, resetAppStore } from '../state/app-store.js';
import type {
  Client,
  ClientContact,
  DocumentDetail,
  DocumentListRow,
} from '../api/types.js';
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

const CEDAR_CONTACTS: ClientContact[] = [
  {
    id: 1,
    clientId: 1,
    name: 'Pat Example',
    email: 'pat@cedar.test',
    title: null,
    isBilling: true,
    position: 0,
  },
  {
    id: 2,
    clientId: 1,
    name: 'Sam Example',
    email: 'sam@cedar.test',
    title: null,
    isBilling: false,
    position: 1,
  },
];

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
  fake.status = { ...UNLOCKED_STATUS, documents: DOCUMENTS_CONFIGURED };
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

async function answerConfirm(answer: boolean): Promise<void> {
  const ui = await import('@nigel/ui');
  vi.spyOn(ui, 'confirmDialog').mockResolvedValue(answer);
}

function documentDetail(overrides: Partial<DocumentDetail> = {}): DocumentDetail {
  return {
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
    status: 'draft',
    clientName: 'Cedar Systems',
    versions: [
      {
        id: 40,
        documentId: 12,
        number: 1,
        checksum: `sha256:${'a'.repeat(64)}`,
        sentAt: null,
        createdAt: '2026-09-28',
        recipients: [],
        signatures: [],
        changeRequests: [],
      },
    ],
    ...documentFlags('draft'),
    ...overrides,
  };
}

function withDetail(detail: DocumentDetail): FakeApiClient {
  const fake = client();
  fake.documentDetails[detail.id] = detail;
  return fake;
}

function sendDialog(el: NigelDocumentsScreen): WcSendDialog | null {
  return el.shadowRoot?.querySelector<WcSendDialog>('wc-send-dialog') ?? null;
}

function actions(el: NigelDocumentsScreen): string[] {
  return [...(el.shadowRoot?.querySelectorAll('[data-action]') ?? [])].map(
    (action) => action.getAttribute('data-action') ?? '',
  );
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
    vi.restoreAllMocks();
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

  it('actions follow the can flags, never the status word', async () => {
    const none = await mount(
      'id=12',
      withDetail(
        documentDetail({
          status: 'sent',
          canEdit: false,
          canSend: false,
          canRevise: false,
          canAccept: false,
          canRequestChanges: false,
          canDecline: false,
          canCountersign: false,
          canWithdraw: false,
        }),
      ),
    );
    expect(none.el.shadowRoot?.querySelector('wc-document-status')).toBeTruthy();
    expect(actions(none.el)).toEqual([]);

    document.body.innerHTML = '';
    resetAppStore();

    const draft = await mount('id=12', withDetail(documentDetail()));
    expect(actions(draft.el)).toEqual(['send', 'edit', 'withdraw']);

    document.body.innerHTML = '';
    resetAppStore();

    const accepted = await mount(
      'id=12',
      withDetail(documentDetail({ status: 'accepted', ...documentFlags('accepted') })),
    );
    expect(actions(accepted.el)).toEqual(['countersign']);
  });

  it('shows the kind, the client and the preview with its PDF link', async () => {
    const { el, fake } = await mount('id=12', withDetail(documentDetail()));

    const header = find(el, 'header');
    expect(header.textContent).toContain('Proposal');
    expect(header.textContent).toContain('Cedar Systems');
    expect(el.shadowRoot?.querySelector('wc-document-frame')).toBeNull();
    expect(find(el, '[data-pdf-link]').getAttribute('href')).toBe('/document-preview/12.pdf');

    find(el, '[data-preview-toggle]').click();
    await settle(el);

    expect(fake.calls).toContain('documentPreviewHtml:12');
    expect((find(el, 'wc-document-frame') as HTMLElement & { srcdoc: string }).srcdoc).toBe(
      '<h1>Website redesign proposal</h1>',
    );
  });

  it('send posts the recipient editor’s value', async () => {
    const fake = withDetail(documentDetail());
    fake.clientContacts[1] = CEDAR_CONTACTS;
    const { el } = await mount('id=12', fake);

    find(el, '[data-action="send"]').click();
    await settle(el);

    expect(fake.calls).toContain('getClient:1');
    const dialog = sendDialog(el);
    expect(dialog?.mode).toBe('document');
    const editor = find<WcRecipientEditor>(el, 'wc-send-dialog > wc-recipient-editor');
    expect(editor.getAttribute('slot')).toBe('recipients');
    expect(editor.value).toEqual({
      signer: { name: 'Pat Example', email: 'pat@cedar.test' },
      collaborators: [],
    });
    expect(dialog?.blocked).toBe('');

    editor.dispatchEvent(
      new CustomEvent('nc-recipients-change', {
        detail: { value: { signer: { name: '', email: 'pat@cedar.test' }, collaborators: [] } },
      }),
    );
    await settle(el);
    expect(sendDialog(el)?.blocked).not.toBe('');
    sendDialog(el)?.dispatchEvent(new CustomEvent('nc-send-confirm'));
    await settle(el);
    expect(fake.calls.some((call) => call.startsWith('sendDocument'))).toBe(false);

    editor.dispatchEvent(
      new CustomEvent('nc-recipients-change', {
        detail: {
          value: {
            signer: { name: 'Pat Example ', email: 'pat@cedar.test' },
            collaborators: [{ name: 'Sam Example', email: 'sam@cedar.test' }],
          },
        },
      }),
    );
    await settle(el);
    expect(sendDialog(el)?.recipientCount).toBe(2);
    sendDialog(el)?.dispatchEvent(new CustomEvent('nc-send-confirm'));
    await settle(el);

    expect(fake.calls).toContain(
      `sendDocument:12:${JSON.stringify({
        signer: { name: 'Pat Example', email: 'pat@cedar.test' },
        collaborators: [{ name: 'Sam Example', email: 'sam@cedar.test' }],
      })}`,
    );
    const sent = sendDialog(el);
    expect(sent?.phase).toBe('sent');
    expect(sent?.steps.every((step) => step.state === 'ok')).toBe(true);
    expect(sent?.pageLinks.map((link) => link.href)).toEqual([
      'https://docs.example.test/d/doc12/r0/index.html',
      'https://docs.example.test/d/doc12/r1/index.html',
    ]);
    expect(sent?.pageLinks[0].label).toContain('Pat Example');
  });

  it('a send failure after an email went out offers no retry', async () => {
    const fake = withDetail(documentDetail());
    fake.sendDocumentError = new ApiError({
      code: 'upstream_failed',
      rawCode: 'upstream_failed',
      message: 'mailgun 500: internal error',
      status: 502,
      details: {
        reason: 'send_failed',
        step: 'email',
        service: 'mailgun',
        completed: ['config', 'load', 'render', 'freeze', 'publish', 'manifest'],
        emailed: ['pat@cedar.test'],
        documentStatus: 'draft',
      },
    });
    fake.clientContacts[1] = CEDAR_CONTACTS;
    const { el } = await mount('id=12', fake);

    find(el, '[data-action="send"]').click();
    await settle(el);
    sendDialog(el)?.dispatchEvent(new CustomEvent('nc-send-confirm'));
    await settle(el);

    const dialog = sendDialog(el);
    expect(dialog?.phase).toBe('failed');
    expect(dialog?.failure?.retryable).toBe(false);
    expect(dialog?.failure?.note).toContain('Already emailed: pat@cedar.test');
    expect(dialog?.steps.find((step) => step.step === 'email')?.state).toBe('failed');
    expect(dialog?.steps.find((step) => step.step === 'publish')?.state).toBe('ok');
  });

  it('withdraw shows its teardown warnings', async () => {
    const fake = withDetail(
      documentDetail({ status: 'sent', ...documentFlags('sent') }),
    );
    fake.documentWarnings = [
      'Could not replace the page for Pat Example: r2 403',
      'Could not close the response manifest: r2 403',
    ];
    const { el } = await mount('id=12', fake);

    await answerConfirm(false);
    find(el, '[data-action="withdraw"]').click();
    await settle(el);
    expect(fake.calls).not.toContain('withdrawDocument:12');

    await answerConfirm(true);
    find(el, '[data-action="withdraw"]').click();
    await settle(el);

    expect(fake.calls).toContain('withdrawDocument:12');
    const warnings = [...(el.shadowRoot?.querySelectorAll('[data-action-warning]') ?? [])];
    expect(warnings.map((warning) => warning.getAttribute('message'))).toEqual(
      fake.documentWarnings,
    );
    expect(find(el, 'wc-document-status').getAttribute('status')).toBe('withdrawn');

    warnings[0].dispatchEvent(new CustomEvent('nc-notice-action'));
    await settle(el);
    expect(el.shadowRoot?.querySelectorAll('[data-action-warning]')).toHaveLength(1);
  });

  it('a wrong-state refusal says so and refreshes the detail', async () => {
    const fake = withDetail(documentDetail({ status: 'sent', ...documentFlags('sent') }));
    const { el } = await mount('id=12', fake);
    fake.documentDetails[12] = documentDetail({ status: 'withdrawn', ...documentFlags('withdrawn') });
    fake.withdrawDocumentError = conflictError('document_wrong_state');

    await answerConfirm(true);
    find(el, '[data-action="withdraw"]').click();
    await settle(el);

    expect(find(el, '[data-action-error]').getAttribute('message')).toBe(
      'This document is not in a state that allows that. The view has been refreshed.',
    );
    expect(fake.calls.filter((call) => call === 'getDocument:12')).toHaveLength(2);
    expect(find(el, 'wc-document-status').getAttribute('status')).toBe('withdrawn');
    expect(actions(el)).toEqual([]);
  });

  it('accept collects the name and date, then confirms', async () => {
    const fake = withDetail(documentDetail({ status: 'sent', ...documentFlags('sent') }));
    const { el } = await mount('id=12', fake);

    find(el, '[data-action="accept"]').click();
    await settle(el);
    const dialog = find(el, 'wc-manager-dialog');
    expect(dialog.textContent).toContain('not a legal e-signature');

    const name = find(el, '[data-field-name]') as HTMLElement & { value: string };
    name.value = 'Pat Example';
    name.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
    const date = find(el, '[data-field-date]') as HTMLElement & { value: string };
    date.value = '2026-10-03';
    date.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
    await settle(el);

    await answerConfirm(true);
    dialog.dispatchEvent(new CustomEvent('nc-manager-save'));
    await settle(el);

    expect(fake.calls).toContain(
      `acceptDocument:12:${JSON.stringify({ name: 'Pat Example', date: '2026-10-03' })}`,
    );
    expect(find(el, 'wc-document-status').getAttribute('status')).toBe('accepted');
    expect(el.shadowRoot?.querySelector('wc-manager-dialog')).toBeNull();
  });

  it('notes in the timeline render as text', async () => {
    const note = '<b>Bold</b> & <img src=x onerror=alert(1)> scope';
    const fake = withDetail(
      documentDetail({
        status: 'changes_requested',
        ...documentFlags('changes_requested'),
        versions: [
          {
            id: 40,
            documentId: 12,
            number: 1,
            checksum: `sha256:${'a'.repeat(64)}`,
            sentAt: '2026-10-01',
            createdAt: '2026-09-28',
            recipients: [
              {
                id: 1,
                versionId: 40,
                role: 'signer',
                name: 'Pat Example',
                email: 'pat@cedar.test',
                position: 0,
                pageUrl: null,
              },
            ],
            signatures: [],
            changeRequests: [
              {
                id: 5,
                versionId: 40,
                recipientId: 1,
                name: 'Pat Example',
                email: 'pat@cedar.test',
                method: 'online',
                requestedAt: '2026-10-02T09:00:00Z',
                note,
                ip: '192.0.2.1',
                userAgent: 'Example/1.0',
                checksum: `sha256:${'a'.repeat(64)}`,
              },
            ],
          },
        ],
      }),
    );
    const { el } = await mount('id=12', fake);

    const timeline = find<WcDocumentTimeline>(el, 'wc-document-timeline');
    await timeline.updateComplete;
    expect(timeline.versions).toHaveLength(1);
    const rendered = timeline.shadowRoot?.querySelector('.note');
    expect(rendered?.textContent).toContain(note);
    expect(rendered?.querySelector('b, img')).toBeNull();
  });

  it('the send dialog promises a response form only when one is configured', async () => {
    const configured = withDetail(documentDetail());
    configured.clientContacts[1] = CEDAR_CONTACTS;
    const on = await mount('id=12', configured);
    find(on.el, '[data-action="send"]').click();
    await settle(on.el);
    expect(sendDialog(on.el)?.responseForm).toBe(true);

    document.body.innerHTML = '';
    resetAppStore();

    const fake = withDetail(documentDetail());
    fake.status = {
      ...UNLOCKED_STATUS,
      documents: { ...DOCUMENTS_CONFIGURED, responseForm: false },
    };
    const off = await mount('id=12', fake);
    find(off.el, '[data-action="send"]').click();
    await settle(off.el);
    expect(sendDialog(off.el)?.responseForm).toBe(false);
  });

  it('send is unavailable until sending is configured, and says which keys', async () => {
    const fake = withDetail(documentDetail());
    fake.status = {
      ...UNLOCKED_STATUS,
      documents: {
        ...DOCUMENTS_CONFIGURED,
        sendConfigured: false,
        missing: ['r2_private_bucket', 'documents_base_url'],
      },
    };
    const { el } = await mount('id=12', fake);

    const send = find(el, '[data-action="send"]');
    expect(send.hasAttribute('disabled')).toBe(true);
    expect(find(el, '[data-send-note]').textContent).toContain(
      'Sending documents needs r2_private_bucket, documents_base_url, which are not set.',
    );
    send.click();
    await settle(el);
    expect(sendDialog(el)).toBeNull();
  });

  it('sync is unavailable until syncing is configured', async () => {
    const fake = client();
    fake.status = {
      ...UNLOCKED_STATUS,
      documents: { ...DOCUMENTS_CONFIGURED, syncConfigured: false },
    };
    const { el } = await mount('', fake);

    const sync = find(el, '[data-sync]');
    expect(sync.hasAttribute('disabled')).toBe(true);
    expect(el.shadowRoot?.querySelector('[data-sync-note]')).toBeTruthy();

    document.body.innerHTML = '';
    resetAppStore();

    const configured = await mount();
    expect(find(configured.el, '[data-sync]').hasAttribute('disabled')).toBe(false);
    expect(configured.el.shadowRoot?.querySelector('[data-sync-note]')).toBeNull();
  });

  describe('the flow from draft to executed', () => {
    function state(el: NigelDocumentsScreen) {
      return {
        status: find(el, 'wc-document-status').getAttribute('status'),
        actions: actions(el).sort(),
        versions: find<WcDocumentTimeline>(el, 'wc-document-timeline').versions.length,
      };
    }

    async function open(el: NigelDocumentsScreen, query: string): Promise<void> {
      el.params = new URLSearchParams(query);
      await settle(el);
    }

    async function fill(el: NigelDocumentsScreen, hook: string, value: string): Promise<void> {
      const field = find(el, hook) as HTMLElement & { value: string };
      field.value = value;
      field.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
      await settle(el);
    }

    async function confirmDialogAction(el: NigelDocumentsScreen): Promise<void> {
      await answerConfirm(true);
      find(el, 'wc-manager-dialog').dispatchEvent(new CustomEvent('nc-manager-save'));
      await settle(el);
    }

    it('walks one document through its whole life', async () => {
      const fake = client();
      fake.clientContacts[1] = CEDAR_CONTACTS;
      const { el, routes } = await mount('', fake);

      await fillFiling(el);
      find(el, '[data-file]').click();
      await settle(el);
      expect(routes).toEqual([{ screen: 'documents', params: 'id=700' }]);
      await open(el, 'id=700');
      expect(state(el)).toEqual({
        status: 'draft',
        actions: ['edit', 'send', 'withdraw'],
        versions: 1,
      });

      find(el, '[data-action="send"]').click();
      await settle(el);
      const editor = find<WcRecipientEditor>(el, 'wc-send-dialog > wc-recipient-editor');
      editor.dispatchEvent(
        new CustomEvent('nc-recipients-change', {
          detail: {
            value: {
              signer: { name: 'Pat Example', email: 'pat@cedar.test' },
              collaborators: [{ name: 'Sam Example', email: 'sam@cedar.test' }],
            },
          },
        }),
      );
      await settle(el);
      sendDialog(el)?.dispatchEvent(new CustomEvent('nc-send-confirm'));
      await settle(el);
      expect(sendDialog(el)?.phase).toBe('sent');
      expect(state(el)).toEqual({
        status: 'sent',
        actions: ['accept', 'decline', 'requestChanges', 'revise', 'withdraw'],
        versions: 1,
      });

      await fake.requestDocumentChanges(700, {
        name: 'Pat Example',
        note: 'Tighten the scope.',
      });
      await open(el, '');
      fake.documentSyncResult = {
        documentsChecked: 1,
        recorded: 1,
        lines: [
          {
            documentId: 700,
            title: 'Phase two proposal',
            recorded: ['Pat Example requested changes to version 1'],
            refused: [],
            warnings: [],
            status: 'changes_requested',
          },
        ],
        failures: [],
      };
      find(el, '[data-sync]').click();
      await settle(el);
      expect(find(el, '[data-sync-report]').textContent).toContain('→ changes_requested');
      await open(el, 'id=700');
      expect(state(el)).toEqual({
        status: 'changes_requested',
        actions: ['accept', 'decline', 'revise', 'withdraw'],
        versions: 1,
      });

      find(el, '[data-action="revise"]').click();
      await settle(el);
      find(el, 'wc-manager-dialog wc-dropzone').dispatchEvent(
        new CustomEvent('nc-file-select', {
          detail: { file: new File(['%PDF-1.7'], 'proposal-v2.pdf', { type: 'application/pdf' }) },
        }),
      );
      await settle(el);
      await confirmDialogAction(el);
      expect(fake.calls).toContain('reviseDocument:700:proposal-v2.pdf');
      expect(state(el)).toEqual({
        status: 'draft',
        actions: ['edit', 'send', 'withdraw'],
        versions: 2,
      });

      find(el, '[data-action="send"]').click();
      await settle(el);
      sendDialog(el)?.dispatchEvent(new CustomEvent('nc-send-confirm'));
      await settle(el);
      expect(fake.calls.filter((call) => call.startsWith('sendDocument:700'))).toHaveLength(2);
      expect(state(el)).toEqual({
        status: 'sent',
        actions: ['accept', 'decline', 'requestChanges', 'revise', 'withdraw'],
        versions: 2,
      });

      find(el, '[data-action="accept"]').click();
      await settle(el);
      await fill(el, '[data-field-name]', 'Pat Example');
      await confirmDialogAction(el);
      expect(fake.calls).toContain(`acceptDocument:700:${JSON.stringify({ name: 'Pat Example' })}`);
      expect(state(el)).toEqual({ status: 'accepted', actions: ['countersign'], versions: 2 });

      find(el, '[data-action="countersign"]').click();
      await settle(el);
      await fill(el, '[data-field-name]', 'Sam Example');
      await confirmDialogAction(el);
      expect(fake.calls).toContain(
        `countersignDocument:700:${JSON.stringify({ name: 'Sam Example' })}`,
      );
      expect(state(el)).toEqual({ status: 'executed', actions: [], versions: 2 });
    });
  });
});
