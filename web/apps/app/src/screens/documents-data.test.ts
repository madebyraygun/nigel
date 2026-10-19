import { describe, it, expect } from 'vitest';

import {
  DOCUMENT_SEND_STEP_LABELS,
  DOCUMENT_STATUS_FILTERS,
  documentListParams,
  documentSendStepViews,
  documentTableRows,
  latestSentVersion,
  recipientContacts,
  sendRequestFrom,
  timelineVersions,
} from './documents-data.js';
import { ApiError } from '../api/index.js';
import { DOCUMENT_SEND_STEPS } from '../api/types.js';
import type {
  ClientDetail,
  DocumentDetail,
  DocumentListRow,
  DocumentVersionDetail,
} from '../api/types.js';

function version(overrides: Partial<DocumentVersionDetail>): DocumentVersionDetail {
  return {
    id: 1,
    documentId: 7,
    number: 1,
    checksum: `sha256:${'a'.repeat(64)}`,
    sentAt: null,
    createdAt: '2026-10-01T09:00:00Z',
    recipients: [],
    signatures: [],
    changeRequests: [],
    ...overrides,
  };
}

function detail(versions: DocumentVersionDetail[]): DocumentDetail {
  return {
    id: 7,
    clientId: 2,
    kindId: 1,
    kind: 'Proposal',
    title: 'Website refresh',
    declinedAt: null,
    declineNote: null,
    withdrawnAt: null,
    createdAt: '2026-10-01T09:00:00Z',
    updatedAt: '2026-10-02T09:00:00Z',
    status: 'sent',
    clientName: 'Cedar Systems',
    versions,
    canEdit: false,
    canSend: false,
    canRevise: true,
    canAccept: true,
    canRequestChanges: true,
    canDecline: true,
    canCountersign: false,
    canWithdraw: true,
  };
}

describe('DOCUMENT_STATUS_FILTERS', () => {
  it('offers all and the seven statuses', () => {
    expect(DOCUMENT_STATUS_FILTERS.map((f) => f.value)).toEqual([
      'all',
      'draft',
      'sent',
      'changes_requested',
      'accepted',
      'declined',
      'executed',
      'withdrawn',
    ]);
    expect(DOCUMENT_STATUS_FILTERS[3]?.label).toBe('Changes requested');
  });
});

describe('documentListParams', () => {
  it('omits an absent or all filter', () => {
    expect(documentListParams(new URLSearchParams('status=all'))).toEqual({});
    expect(documentListParams(new URLSearchParams(''))).toEqual({});
  });

  it('passes a known status, a client id and a kind', () => {
    expect(
      documentListParams(new URLSearchParams('status=sent&clientId=4&kind=Proposal')),
    ).toEqual({ status: 'sent', clientId: 4, kind: 'Proposal' });
  });

  it('drops an unknown status and a bad client id', () => {
    expect(documentListParams(new URLSearchParams('status=bogus&clientId=x'))).toEqual({});
  });
});

describe('documentTableRows', () => {
  it('maps a list row and links to the detail route', () => {
    const row: DocumentListRow = {
      id: 7,
      title: 'Website refresh',
      kind: 'Proposal',
      clientId: 2,
      clientName: null,
      status: 'draft',
      latestVersion: 3,
      sentAt: null,
      updatedAt: '2026-10-02T09:00:00Z',
    };
    expect(documentTableRows([row])).toEqual([
      {
        id: 7,
        title: 'Website refresh',
        kind: 'Proposal',
        status: 'draft',
        clientName: null,
        version: 3,
        updatedAt: '2026-10-02T09:00:00Z',
        href: '#/documents?id=7',
      },
    ]);
  });
});

describe('timelineVersions', () => {
  const signer = { id: 1, versionId: 2, role: 'signer' as const, name: 'Pat Example', email: 'pat@cedar.test', position: 0, pageUrl: null };

  it('lists versions newest first with events in time order', () => {
    const v2 = version({
      id: 2,
      number: 2,
      sentAt: '2026-10-03T09:00:00Z',
      recipients: [signer],
      changeRequests: [
        {
          id: 1,
          versionId: 2,
          recipientId: 1,
          name: 'Pat Example',
          email: 'pat@cedar.test',
          method: 'online',
          requestedAt: '2026-10-04T09:00:00Z',
          note: 'Please swap <b>section 2</b> & 3',
          ip: '203.0.113.9',
          userAgent: 'TestAgent',
          checksum: 'sha256:x',
        },
      ],
      signatures: [
        {
          id: 1,
          versionId: 2,
          recipientId: null,
          role: 'countersign',
          name: 'Sam Example',
          email: null,
          method: 'manual',
          signedAt: '2026-10-06T09:00:00Z',
          typedName: 'Sam Example',
          ip: null,
          userAgent: null,
          checksum: 'sha256:x',
        },
        {
          id: 2,
          versionId: 2,
          recipientId: 1,
          role: 'client',
          name: 'Pat Example',
          email: 'pat@cedar.test',
          method: 'online',
          signedAt: '2026-10-05T09:00:00Z',
          typedName: 'Pat Example',
          ip: '203.0.113.9',
          userAgent: 'TestAgent',
          checksum: 'sha256:x',
        },
      ],
    });
    const views = timelineVersions(detail([version({ id: 1, number: 1 }), v2]));

    expect(views.map((v) => v.number)).toEqual([2, 1]);
    expect(views[0]?.recipients).toEqual([
      { role: 'signer', name: 'Pat Example', email: 'pat@cedar.test' },
    ]);
    expect(views[0]?.events.map((e) => [e.kind, e.role, e.at])).toEqual([
      ['change_request', null, '2026-10-04T09:00:00Z'],
      ['signature', 'client', '2026-10-05T09:00:00Z'],
      ['signature', 'countersign', '2026-10-06T09:00:00Z'],
    ]);
    expect(views[0]?.events[0]?.note).toBe('Please swap <b>section 2</b> & 3');
    expect(views[0]?.events[1]?.typedName).toBe('Pat Example');
    expect(views[0]?.events[1]?.note).toBeNull();
  });
});

describe('recipientContacts', () => {
  it('keeps name, email and billing flag', () => {
    const client = {
      contacts: [
        { id: 1, clientId: 2, name: 'Pat Example', email: 'pat@cedar.test', title: 'CEO', isBilling: true, position: 0 },
        { id: 2, clientId: 2, name: null, email: 'ops@cedar.test', title: null, isBilling: false, position: 1 },
      ],
    } as ClientDetail;
    expect(recipientContacts(client)).toEqual([
      { name: 'Pat Example', email: 'pat@cedar.test', isBilling: true },
      { name: null, email: 'ops@cedar.test', isBilling: false },
    ]);
  });
});

describe('sendRequestFrom', () => {
  it('trims every name and address', () => {
    expect(
      sendRequestFrom({
        signer: { name: ' Pat Example ', email: ' pat@cedar.test ' },
        collaborators: [{ name: ' Sam Example', email: 'sam@cedar.test ' }],
      }),
    ).toEqual({
      signer: { name: 'Pat Example', email: 'pat@cedar.test' },
      collaborators: [{ name: 'Sam Example', email: 'sam@cedar.test' }],
    });
  });
});

describe('documentSendStepViews', () => {
  it('labels every step in order', () => {
    expect(Object.keys(DOCUMENT_SEND_STEP_LABELS)).toEqual([...DOCUMENT_SEND_STEPS]);
  });

  it('shows the first step running and the rest pending while sending', () => {
    const views = documentSendStepViews({ running: true });
    expect(views.map((v) => v.state)).toEqual([
      'running',
      ...Array(DOCUMENT_SEND_STEPS.length - 1).fill('pending'),
    ]);
    expect(views[0]).toMatchObject({ step: 'config', label: DOCUMENT_SEND_STEP_LABELS.config });
  });

  it('marks a result’s steps with their outcome', () => {
    const views = documentSendStepViews({
      running: false,
      result: {
        document: detail([]),
        steps: DOCUMENT_SEND_STEPS.map((step) => ({ step, outcome: 'ok' as const })),
        links: [],
        configWarnings: [],
        warnings: [],
      },
    });
    expect(views.every((v) => v.state === 'ok')).toBe(true);
  });

  it('marks completed steps ok and the failed one failed', () => {
    const error = new ApiError({
      code: 'upstream_failed',
      rawCode: 'upstream_failed',
      message: 'mailgun 500',
      status: 502,
      details: { step: 'email', completed: ['config', 'load', 'render', 'freeze', 'publish', 'manifest'] },
    });
    const states = documentSendStepViews({ running: false, error }).map((v) => v.state);
    expect(states).toEqual(['ok', 'ok', 'ok', 'ok', 'ok', 'ok', 'failed', 'pending']);
  });
});

describe('latestSentVersion', () => {
  it('returns the highest-numbered sent version', () => {
    const d = detail([
      version({ id: 1, number: 1, sentAt: '2026-10-02T09:00:00Z' }),
      version({ id: 2, number: 2, sentAt: '2026-10-03T09:00:00Z' }),
      version({ id: 3, number: 3, sentAt: null }),
    ]);
    expect(latestSentVersion(d)?.number).toBe(2);
  });

  it('is null when nothing was sent', () => {
    expect(latestSentVersion(detail([version({})]))).toBeNull();
  });
});
