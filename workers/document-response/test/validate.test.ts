import { describe, it, expect } from 'vitest';
import { checkRequest, normalizeName, parseRequest, isRefusal, type Manifest, type ResponseRequest } from '../src/validate.js';

const manifest: Manifest = {
  version: 2, checksum: 'sha256:ab', state: 'open',
  recipients: [
    { token: 'rs', role: 'signer', name: 'Zoë Example' },
    { token: 'rc', role: 'collaborator', name: 'Sam Example' },
  ],
};
const accept: ResponseRequest = { token: 'doc', recipientToken: 'rs', version: 2, checksum: 'sha256:ab', action: 'accept', typedName: 'Zoë Example', consent: true };
const changes: ResponseRequest = { token: 'doc', recipientToken: 'rc', version: 2, checksum: 'sha256:ab', action: 'request_changes', note: 'Fix the dates' };

describe('the role matrix', () => {
  it('lets a signer accept or request changes', () => {
    expect(checkRequest(accept, manifest)).toBeNull();
    expect(checkRequest({ ...changes, recipientToken: 'rs' }, manifest)).toBeNull();
  });
  it('lets a collaborator only request changes', () => {
    expect(checkRequest(changes, manifest)).toBeNull();
    expect(checkRequest({ ...accept, recipientToken: 'rc', typedName: 'Sam Example' }, manifest)?.status).toBe(403);
  });
  it('refuses a token the manifest does not name', () => {
    expect(checkRequest({ ...accept, recipientToken: 'nobody' }, manifest)?.code).toBe('forbidden');
  });
});

describe('names', () => {
  it('names_match_across_case_whitespace_and_normalization_but_not_letters', () => {
    for (const typed of ['zoë example', '  ZOË   Example ', 'Zoë Example']) {
      expect(checkRequest({ ...accept, typedName: typed }, manifest), typed).toBeNull();
    }
    for (const typed of ['Zoe Example', 'Zoë Exampl', '']) {
      expect(checkRequest({ ...accept, typedName: typed }, manifest)?.code, typed).toBe('name_mismatch');
    }
    expect(normalizeName('STRASSE')).toBe(normalizeName('straße'));
  });
  it('refuses a typed name carrying a control character', () => {
    for (const typed of ['Zoë\tExample', 'Zoë\nExample', 'Zoë\u0085Example', 'Zoë Example\u0000']) {
      expect(checkRequest({ ...accept, typedName: typed }, manifest)?.code, JSON.stringify(typed)).toBe('name_invalid');
    }
  });
});

describe('consent and notes', () => {
  it('requires consent to be exactly true', () => {
    for (const consent of [false, undefined]) {
      expect(checkRequest({ ...accept, consent }, manifest)?.code).toBe('consent_required');
    }
  });
  it('bounds the note at 1 to 4000 characters of text', () => {
    expect(checkRequest({ ...changes, note: '' }, manifest)?.code).toBe('note_invalid');
    expect(checkRequest({ ...changes, note: '   ' }, manifest)?.code).toBe('note_invalid');
    expect(checkRequest({ ...changes, note: '😀'.repeat(4000) }, manifest)).toBeNull();
    expect(checkRequest({ ...changes, note: 'x'.repeat(4001) }, manifest)?.code).toBe('note_invalid');
    expect(checkRequest({ ...changes, note: 'bell\u0007' }, manifest)?.code).toBe('note_invalid');
    expect(checkRequest({ ...changes, note: '<script>alert(1)</script>' }, manifest)).toBeNull();
  });
});

describe('the manifest', () => {
  it('is 404 when missing and 409 when closed', () => {
    expect(checkRequest(accept, null)?.status).toBe(404);
    expect(checkRequest(accept, { ...manifest, state: 'closed' })?.code).toBe('closed');
  });
  it('refuses a version or checksum it does not carry', () => {
    expect(checkRequest({ ...accept, version: 1 }, manifest)?.code).toBe('version_mismatch');
    expect(checkRequest({ ...accept, checksum: 'sha256:00' }, manifest)?.code).toBe('version_mismatch');
  });
});

describe('parsing', () => {
  it('refuses a malformed body with 400', () => {
    for (const body of [null, 'x', {}, { ...accept, version: '2' }, { ...accept, action: 'delete' }]) {
      const parsed = parseRequest(body);
      expect(isRefusal(parsed) && parsed.status, JSON.stringify(body)).toBe(400);
    }
  });
});
