import type {
  DocumentTableRow,
  RecipientContactOption,
  RecipientEditorValue,
  SendStepState,
  SendStepView,
  TimelineEvent,
  TimelineVersion,
} from '@nigel/ui';

import type {
  ClientDetail,
  DocumentDetail,
  DocumentEmailPreviewRequest,
  DocumentListParams,
  DocumentListRow,
  DocumentSendRequest,
  DocumentSendResult,
  DocumentSendStep,
  DocumentVersionDetail,
} from '../api/types.js';
import { DOCUMENT_SEND_STEPS } from '../api/types.js';
import { documentSendDetailsOf } from './documents-errors.js';

/** The status filters the list offers. `all` is not a word the server knows. */
export const DOCUMENT_STATUS_FILTERS: { value: string; label: string }[] = [
  { value: 'all', label: 'All' },
  { value: 'draft', label: 'Draft' },
  { value: 'sent', label: 'Sent' },
  { value: 'changes_requested', label: 'Changes requested' },
  { value: 'accepted', label: 'Accepted' },
  { value: 'declined', label: 'Declined' },
  { value: 'executed', label: 'Executed' },
  { value: 'withdrawn', label: 'Withdrawn' },
];

/**
 * The list request a route asks for. An absent filter is omitted, and a status
 * outside the chips is dropped, because the server rejects a status it does
 * not know instead of ignoring it.
 */
export function documentListParams(params: URLSearchParams): DocumentListParams {
  const request: DocumentListParams = {};

  const status = params.get('status');
  if (
    status &&
    status !== 'all' &&
    DOCUMENT_STATUS_FILTERS.some((filter) => filter.value === status)
  ) {
    request.status = status;
  }

  const clientId = Number(params.get('clientId'));
  if (Number.isInteger(clientId) && clientId > 0) request.clientId = clientId;

  const kind = params.get('kind');
  if (kind) request.kind = kind;

  return request;
}

export function documentTableRows(rows: DocumentListRow[]): DocumentTableRow[] {
  return rows.map((row) => ({
    id: row.id,
    title: row.title,
    kind: row.kind,
    status: row.status,
    clientName: row.clientName,
    version: row.latestVersion,
    updatedAt: row.updatedAt,
    href: `#/documents?id=${row.id}`,
  }));
}

function eventsOf(version: DocumentVersionDetail): TimelineEvent[] {
  const signatures: TimelineEvent[] = version.signatures.map((s) => ({
    kind: 'signature',
    role: s.role,
    name: s.name,
    email: s.email,
    method: s.method,
    at: s.signedAt,
    typedName: s.typedName,
    title: s.title,
    ip: s.ip,
    userAgent: s.userAgent,
    note: null,
  }));
  const requests: TimelineEvent[] = version.changeRequests.map((c) => ({
    kind: 'change_request',
    role: null,
    name: c.name,
    email: c.email,
    method: c.method,
    at: c.requestedAt,
    typedName: null,
    title: null,
    ip: c.ip,
    userAgent: c.userAgent,
    note: c.note,
  }));
  return [...signatures, ...requests].sort((a, b) => a.at.localeCompare(b.at));
}

/** Every version, newest first, with its responses in the order they happened. */
export function timelineVersions(detail: DocumentDetail): TimelineVersion[] {
  return [...detail.versions]
    .sort((a, b) => b.number - a.number)
    .map((version) => ({
      number: version.number,
      checksum: version.checksum,
      sentAt: version.sentAt,
      createdAt: version.createdAt,
      recipients: version.recipients.map((r) => ({
        role: r.role,
        name: r.name,
        email: r.email,
      })),
      events: eventsOf(version),
      message: version.emailGreeting,
    }));
}

export function recipientContacts(client: ClientDetail): RecipientContactOption[] {
  return client.contacts.map((contact) => ({
    name: contact.name,
    email: contact.email,
    isBilling: contact.isBilling,
  }));
}

export function sendRequestFrom(value: RecipientEditorValue): DocumentSendRequest {
  const trimmed = (r: { name: string; email: string }) => ({
    name: r.name.trim(),
    email: r.email.trim(),
  });
  return {
    signer: trimmed(value.signer),
    collaborators: value.collaborators.map(trimmed),
  };
}

/**
 * An address the server accepts for a recipient: a local part, one `@`, a
 * domain with a dot, and no whitespace.
 */
export function isRecipientAddress(email: string): boolean {
  return /^[^@\s]+@[^@\s]*\.[^@\s]*$/.test(email);
}

function usableRecipient(r: { name: string; email: string }): boolean {
  return r.name.trim() !== '' && isRecipientAddress(r.email.trim());
}

/**
 * The recipients a preview can be built for, as they stand in the editor. A
 * row still being typed is left out rather than sent, so the preview does not
 * flash a validation error on every keystroke; an unusable signer falls back
 * to the server's default one. `previewRecipientNotes` says when either
 * happened.
 */
export function previewRequestFrom(
  value: RecipientEditorValue,
  greeting: string | undefined,
): DocumentEmailPreviewRequest {
  const { signer, collaborators } = sendRequestFrom(value);
  return {
    ...(usableRecipient(signer) ? { signer } : {}),
    collaborators: collaborators.filter(usableRecipient),
    ...(greeting === undefined ? {} : { greeting }),
  };
}

/** Where the preview's recipients differ from the editor's, in a sentence each. */
export function previewRecipientNotes(value: RecipientEditorValue): string[] {
  const notes: string[] = [];
  if (!usableRecipient(value.signer)) {
    notes.push('The signer row is incomplete, so the preview uses the default signer.');
  }
  const left = value.collaborators.filter((r) => !usableRecipient(r)).length;
  if (left > 0) {
    notes.push(
      `The preview leaves out ${left} incomplete ${left === 1 ? 'recipient' : 'recipients'}.`,
    );
  }
  return notes;
}

/** Our words for each step of a document send, in execution order. */
export const DOCUMENT_SEND_STEP_LABELS: Record<DocumentSendStep, string> = {
  config: 'Reading the document settings',
  load: 'Loading the document',
  render: 'Rendering the pages',
  freeze: 'Freezing the version',
  publish: 'Publishing the pages',
  manifest: 'Opening the response manifest',
  email: 'Emailing the recipients',
  record: 'Recording the send',
};

/**
 * The dialog's step list. A send is one request, so while it runs nothing is
 * known beyond the first step; a result or a failure says exactly where it got.
 */
export function documentSendStepViews(options: {
  running: boolean;
  result?: DocumentSendResult;
  error?: unknown;
}): SendStepView[] {
  const outcomes = new Map<string, SendStepState>();
  for (const { step, outcome } of options.result?.steps ?? []) outcomes.set(step, outcome);

  const details = options.error === undefined ? null : documentSendDetailsOf(options.error);
  for (const step of details?.completed ?? []) outcomes.set(step, 'ok');

  const running = options.running && !options.result && options.error === undefined;

  return DOCUMENT_SEND_STEPS.map((step, index) => ({
    step,
    label: DOCUMENT_SEND_STEP_LABELS[step],
    state:
      details?.step === step
        ? 'failed'
        : (outcomes.get(step) ?? (running && index === 0 ? 'running' : 'pending')),
  }));
}

/** The newest version that has been sent, or null when no version has been sent. */
export function latestSentVersion(detail: DocumentDetail): DocumentVersionDetail | null {
  const sent = detail.versions.filter((version) => version.sentAt !== null);
  if (sent.length === 0) return null;
  return sent.reduce((latest, version) => (version.number > latest.number ? version : latest));
}
