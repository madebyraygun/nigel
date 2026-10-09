import { describe, it, expect, beforeEach } from 'vitest';
import { BODY_MAX_BYTES, changesRequestedKey, handle, manifestKey, responseKey } from '../src/index.js';
import { MemoryBucket, CountingLimiter } from './memory-r2.js';

const DOC = '0123456789abcdef0123456789abcdef';
const SIGNER = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
const COLLABORATOR = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
const MANIFEST = { version: 2, checksum: 'sha256:ab', state: 'open', recipients: [
  { token: SIGNER, role: 'signer', name: 'Pat Example' }, { token: COLLABORATOR, role: 'collaborator', name: 'Sam Example' } ] };
const ACCEPT = { token: DOC, recipientToken: SIGNER, version: 2, checksum: 'sha256:ab', action: 'accept', typedName: 'pat example', consent: true };
const CHANGES = { token: DOC, recipientToken: COLLABORATOR, version: 2, checksum: 'sha256:ab', action: 'request_changes', note: 'x' };

let bucket: MemoryBucket;
let env: { PRIVATE: MemoryBucket; RATE_LIMITER: CountingLimiter };
const postRaw = (body: string, path = '/d/respond', headers: Record<string, string> = { 'CF-Connecting-IP': '203.0.113.7', 'User-Agent': 'UA' }) =>
  handle(new Request(`https://docs.example.test${path}`, { method: 'POST', body, headers }), env, () => new Date('2026-10-05T17:04:11.123Z'));
const post = (body: unknown, path = '/d/respond') => postRaw(JSON.stringify(body), path);
const responses = () => [...bucket.objects.keys()].filter((k) => k !== manifestKey(DOC));

beforeEach(() => {
  bucket = new MemoryBucket();
  bucket.objects.set(manifestKey(DOC), JSON.stringify(MANIFEST));
  env = { PRIVATE: bucket, RATE_LIMITER: new CountingLimiter(10) };
});

describe('POST /d/respond', () => {
  it('writes the response the sync expects', async () => {
    const res = await post(ACCEPT);
    expect(res.status).toBe(200);
    expect(await res.json()).toEqual({ ok: true, message: 'Received: thank you' });
    expect(JSON.parse(bucket.objects.get(responseKey(DOC, 2, SIGNER))!)).toEqual({
      action: 'accept', version: 2, checksum: 'sha256:ab', recipientToken: SIGNER, typedName: 'pat example',
      consent: true, note: null, receivedAt: '2026-10-05T17:04:11Z', ip: '203.0.113.7', userAgent: 'UA' });
  });
  it('writes a change request without consent or a typed name', async () => {
    expect((await post(CHANGES)).status).toBe(200);
    expect(JSON.parse(bucket.objects.get(responseKey(DOC, 2, COLLABORATOR))!)).toEqual({
      action: 'request_changes', version: 2, checksum: 'sha256:ab', recipientToken: COLLABORATOR,
      note: 'x', receivedAt: '2026-10-05T17:04:11Z', ip: '203.0.113.7', userAgent: 'UA' });
  });
  it('takes receivedAt, ip and userAgent from the request, never the body', async () => {
    const res = await postRaw(JSON.stringify({ ...ACCEPT, receivedAt: '2000-01-01T00:00:00Z', ip: '198.51.100.1', userAgent: 'forged' }), '/d/respond', {});
    expect(res.status).toBe(200);
    const record = JSON.parse(bucket.objects.get(responseKey(DOC, 2, SIGNER))!);
    expect(record).toMatchObject({ receivedAt: '2026-10-05T17:04:11Z', ip: null, userAgent: null });
  });
  it('truncates a user agent longer than 512 characters', async () => {
    await postRaw(JSON.stringify(ACCEPT), '/d/respond', { 'User-Agent': 'U'.repeat(600) });
    expect(JSON.parse(bucket.objects.get(responseKey(DOC, 2, SIGNER))!).userAgent).toBe('U'.repeat(512));
  });
  it('is write-once: the second post gets 409 already_responded', async () => {
    expect((await post(ACCEPT)).status).toBe(200);
    const again = await post({ ...ACCEPT, action: 'request_changes', note: 'x', typedName: undefined, consent: undefined });
    expect(again.status).toBe(409);
    expect((await again.json()).code).toBe('already_responded');
    expect(JSON.parse(bucket.objects.get(responseKey(DOC, 2, SIGNER))!).action).toBe('accept');
  });
  it('two_concurrent_posts_write_one_response', async () => {
    const [a, b] = await Promise.all([post(ACCEPT), post(ACCEPT)]);
    expect([a.status, b.status].sort()).toEqual([200, 409]);
    expect(responses()).toHaveLength(1);
  });
  it('rate-limits per recipient token', async () => {
    env.RATE_LIMITER = new CountingLimiter(1);
    expect((await post({ ...ACCEPT, typedName: 'wrong' })).status).toBe(422);
    const limited = await post(ACCEPT);
    expect(limited.status).toBe(429);
    expect((await limited.json()).code).toBe('rate_limited');
    expect((await post(CHANGES)).status).toBe(200);
  });
  it('refuses a version mismatch, a closed manifest, and a missing one', async () => {
    const mismatch = await post({ ...ACCEPT, version: 1 });
    expect(mismatch.status).toBe(409);
    expect((await mismatch.json()).code).toBe('version_mismatch');
    bucket.objects.set(manifestKey(DOC), JSON.stringify({ ...MANIFEST, state: 'closed' }));
    expect((await post(ACCEPT)).status).toBe(409);
    expect((await post({ ...ACCEPT, token: 'c'.repeat(32) })).status).toBe(404);
    expect(responses()).toHaveLength(0);
  });
  it('answers 404 off the respond path, 405 for other methods and 400 for a body that is not JSON', async () => {
    expect((await post(ACCEPT, '/d/other')).status).toBe(404);
    expect((await handle(new Request('https://docs.example.test/d/respond'), env)).status).toBe(405);
    expect((await postRaw('{')).status).toBe(400);
  });
  it('never writes to anything but the response key under the document', async () => {
    await post(ACCEPT);
    expect(responses()).toEqual([`d/${DOC}/v2/${SIGNER}.json`]);
  });
  it('a change request closes the version: a later accept is refused and nothing is written', async () => {
    expect((await post(CHANGES)).status).toBe(200);
    expect(responses().sort()).toEqual([`d/${DOC}/v2/${COLLABORATOR}.json`, changesRequestedKey(DOC, 2)].sort());
    const accept = await post(ACCEPT);
    expect(accept.status).toBe(409);
    expect(await accept.json()).toEqual({
      code: 'changes_requested',
      message: 'Changes have been requested on this version. A new document will be sent.',
    });
    expect(bucket.objects.has(responseKey(DOC, 2, SIGNER))).toBe(false);
  });
  it('an accept writes no marker', async () => {
    expect((await post(ACCEPT)).status).toBe(200);
    expect(bucket.objects.has(changesRequestedKey(DOC, 2))).toBe(false);
  });
  it('a change request on one version does not close the next', async () => {
    await post(CHANGES);
    bucket.objects.set(manifestKey(DOC), JSON.stringify({ ...MANIFEST, version: 3 }));
    expect((await post({ ...ACCEPT, version: 3 })).status).toBe(200);
  });
});

describe('hostile and malformed input', () => {
  it('refuses a token or recipient token that is not 32 lowercase hex before building a key', async () => {
    const touched: string[] = [];
    const get = bucket.get.bind(bucket);
    bucket.get = (key) => { touched.push(key); return get(key); };
    for (const token of ['../../x', `${DOC}/..`, DOC.toUpperCase(), DOC.slice(1), `${DOC}0`, '']) {
      for (const body of [{ ...ACCEPT, token }, { ...ACCEPT, recipientToken: token }]) {
        const res = await post(body);
        expect(res.status, JSON.stringify(body)).toBe(400);
        expect((await res.json()).code).toBe('bad_request');
      }
    }
    expect(touched).toEqual([]);
    expect(responses()).toHaveLength(0);
  });
  it('refuses an oversized body with 413 before parsing', async () => {
    const res = await postRaw(JSON.stringify({ ...CHANGES, note: 'x'.repeat(BODY_MAX_BYTES) }));
    expect(res.status).toBe(413);
    expect((await res.json()).code).toBe('payload_too_large');
    expect((await postRaw('{'.repeat(BODY_MAX_BYTES + 1))).status).toBe(413);
  });
  it('refuses a typed name longer than 200 characters', async () => {
    const name = 'Pat '.padEnd(201, 'x');
    bucket.objects.set(manifestKey(DOC), JSON.stringify({ ...MANIFEST, recipients: [{ token: SIGNER, role: 'signer', name }] }));
    const res = await post({ ...ACCEPT, typedName: name });
    expect(res.status).toBe(422);
    expect((await res.json()).code).toBe('name_invalid');
    expect(responses()).toHaveLength(0);
  });
  it('refuses a typed name with a tab or newline that Nigel would refuse, and writes nothing', async () => {
    for (const typedName of ['pat\texample', 'pat\nexample']) {
      const res = await post({ ...ACCEPT, typedName });
      expect(res.status, JSON.stringify(typedName)).toBe(422);
      expect((await res.json()).code).toBe('name_invalid');
    }
    expect(responses()).toHaveLength(0);
    expect((await post(ACCEPT)).status).toBe(200);
  });
  it('answers a malformed manifest with the standard envelope', async () => {
    for (const manifest of ['{', 'null', '{"state":"open"}', '{"version":2,"checksum":"sha256:ab","state":"open","recipients":[null]}']) {
      bucket.objects.set(manifestKey(DOC), manifest);
      const res = await post(ACCEPT);
      expect(res.status, manifest).toBe(503);
      expect(await res.json()).toEqual({ code: 'unavailable', message: 'This document cannot take responses right now.' });
    }
    expect(responses()).toHaveLength(0);
  });
  it('matches an NFD-typed signer name against an NFC manifest name', async () => {
    bucket.objects.set(manifestKey(DOC), JSON.stringify({ ...MANIFEST, recipients: [{ token: SIGNER, role: 'signer', name: 'Zoë Example' }] }));
    expect((await post({ ...ACCEPT, typedName: 'zoë example' })).status).toBe(200);
  });
  it('refuses type confusion with 400 and writes nothing', async () => {
    for (const body of [
      { ...ACCEPT, consent: 'true' },
      { ...CHANGES, note: ['x'] },
      { ...CHANGES, note: { text: 'x' } },
      { ...CHANGES, note: null },
      { ...ACCEPT, version: 2.5 },
      { ...ACCEPT, token: 1 },
      [ACCEPT],
    ]) {
      const res = await post(body);
      expect(res.status, JSON.stringify(body)).toBe(400);
    }
    expect(responses()).toHaveLength(0);
  });
});
