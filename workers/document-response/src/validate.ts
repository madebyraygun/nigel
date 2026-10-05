export type Role = 'signer' | 'collaborator';
export type Action = 'accept' | 'request_changes';

export interface Manifest {
  version: number;
  checksum: string;
  state: 'open' | 'closed';
  recipients: { token: string; role: Role; name: string }[];
}

export interface ResponseRequest {
  token: string;
  recipientToken: string;
  version: number;
  checksum: string;
  action: Action;
  typedName?: string;
  consent?: boolean;
  note?: string;
}

export interface Refusal {
  status: 400 | 403 | 404 | 405 | 409 | 413 | 422 | 429 | 503;
  code: string;
  message: string;
}

export const NOTE_MAX = 4000;
export const TYPED_NAME_MAX = 200;

const CONTROL = /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f]/u;
const ANY_CONTROL = /[\u0000-\u001f\u007f-\u009f]/u;

const refuse = (status: Refusal['status'], code: string, message: string): Refusal => ({ status, code, message });

export function isRefusal(value: unknown): value is Refusal {
  return typeof value === 'object' && value !== null && 'status' in value && 'code' in value && 'message' in value;
}

export function normalizeName(name: string): string {
  return name.normalize('NFC').trim().replace(/\s+/gu, ' ').toUpperCase().toLowerCase();
}

export function parseRequest(body: unknown): ResponseRequest | Refusal {
  const bad = (message: string) => refuse(400, 'bad_request', message);
  if (typeof body !== 'object' || body === null || Array.isArray(body)) return bad('Expected a JSON object.');
  const b = body as Record<string, unknown>;
  if (typeof b.token !== 'string' || typeof b.recipientToken !== 'string' || typeof b.checksum !== 'string') {
    return bad('Missing or invalid token, recipientToken or checksum.');
  }
  if (typeof b.version !== 'number' || !Number.isInteger(b.version)) return bad('Missing or invalid version.');
  if (b.action !== 'accept' && b.action !== 'request_changes') return bad('Unknown action.');
  for (const [field, type] of [['typedName', 'string'], ['consent', 'boolean'], ['note', 'string']] as const) {
    if (b[field] !== undefined && typeof b[field] !== type) return bad(`Invalid ${field}.`);
  }
  return {
    token: b.token,
    recipientToken: b.recipientToken,
    version: b.version,
    checksum: b.checksum,
    action: b.action,
    typedName: b.typedName as string | undefined,
    consent: b.consent as boolean | undefined,
    note: b.note as string | undefined,
  };
}

export function checkRequest(request: ResponseRequest, manifest: Manifest | null): Refusal | null {
  if (manifest === null) return refuse(404, 'not_found', 'No such document.');
  if (manifest.state !== 'open') return refuse(409, 'closed', 'This document is no longer accepting responses.');
  const recipient = manifest.recipients.find((r) => r.token === request.recipientToken);
  if (!recipient) return refuse(403, 'forbidden', 'This link is not valid for this document.');
  if (recipient.role === 'collaborator' && request.action === 'accept') {
    return refuse(403, 'forbidden', 'Collaborators cannot accept a document.');
  }
  if (request.version !== manifest.version || request.checksum !== manifest.checksum) {
    return refuse(409, 'version_mismatch', 'This document has changed. Reload to see the latest version.');
  }
  if (request.action === 'accept') {
    if (request.consent !== true) return refuse(422, 'consent_required', 'Consent to sign electronically is required.');
    const typed = (request.typedName ?? '').trim();
    if ([...typed].length > TYPED_NAME_MAX || ANY_CONTROL.test(typed)) {
      return refuse(422, 'name_invalid', `The typed name must be at most ${TYPED_NAME_MAX} characters of plain text.`);
    }
    if (normalizeName(request.typedName ?? '') !== normalizeName(recipient.name)) {
      return refuse(422, 'name_mismatch', 'The typed name does not match the signer.');
    }
    return null;
  }
  const note = request.note;
  if (typeof note !== 'string' || note.trim() === '' || [...note].length > NOTE_MAX || CONTROL.test(note)) {
    return refuse(422, 'note_invalid', `The note must be 1 to ${NOTE_MAX} characters of plain text.`);
  }
  return null;
}
