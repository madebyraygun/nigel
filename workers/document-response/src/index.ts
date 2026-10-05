import type { Env } from './env.js';
import { checkRequest, isRefusal, parseRequest, type Manifest, type Refusal } from './validate.js';

export const RESPOND_PATH = '/d/respond';
export const BODY_MAX_BYTES = 64 * 1024;
const USER_AGENT_MAX = 512;
const TOKEN = /^[0-9a-f]{32}$/;

export function manifestKey(token: string): string {
  return `d/${token}/manifest.json`;
}

export function responseKey(token: string, version: number, recipientToken: string): string {
  return `d/${token}/v${version}/${recipientToken}.json`;
}

const json = (body: unknown, status: number) =>
  new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });

const reply = ({ status, code, message }: Refusal) => json({ code, message }, status);

async function readCapped(request: Request): Promise<string | null> {
  if (Number(request.headers.get('Content-Length')) > BODY_MAX_BYTES) return null;
  if (!request.body) return '';
  const reader = request.body.getReader();
  const decoder = new TextDecoder();
  let size = 0;
  let text = '';
  for (;;) {
    const { done, value } = await reader.read();
    if (done) return text + decoder.decode();
    size += value.byteLength;
    if (size > BODY_MAX_BYTES) {
      await reader.cancel();
      return null;
    }
    text += decoder.decode(value, { stream: true });
  }
}

function isManifest(value: unknown): value is Manifest {
  if (typeof value !== 'object' || value === null) return false;
  const m = value as Record<string, unknown>;
  return typeof m.version === 'number' && typeof m.checksum === 'string' && typeof m.state === 'string'
    && Array.isArray(m.recipients) && m.recipients.every((r) => typeof r === 'object' && r !== null);
}

function userAgent(request: Request): string | null {
  const ua = request.headers.get('User-Agent');
  return ua === null ? null : [...ua].slice(0, USER_AGENT_MAX).join('');
}

export async function handle(request: Request, env: Env, now: () => Date = () => new Date()): Promise<Response> {
  const url = new URL(request.url);
  if (url.pathname !== RESPOND_PATH) return reply({ status: 404, code: 'not_found', message: 'Not found.' });
  if (request.method !== 'POST') return reply({ status: 405, code: 'method_not_allowed', message: 'POST only.' });
  const text = await readCapped(request);
  if (text === null) return reply({ status: 413, code: 'payload_too_large', message: 'The request is too large.' });
  let body: unknown;
  try {
    body = JSON.parse(text);
  } catch {
    return reply({ status: 400, code: 'bad_request', message: 'The body is not JSON.' });
  }
  const parsed = parseRequest(body);
  if (isRefusal(parsed)) return reply(parsed);
  if (!TOKEN.test(parsed.token) || !TOKEN.test(parsed.recipientToken)) {
    return reply({ status: 400, code: 'bad_request', message: 'Missing or invalid token, recipientToken or checksum.' });
  }
  const { success } = await env.RATE_LIMITER.limit({ key: parsed.recipientToken });
  if (!success) return reply({ status: 429, code: 'rate_limited', message: 'Too many attempts. Wait a minute and try again.' });
  const stored = await env.PRIVATE.get(manifestKey(parsed.token));
  let manifest: Manifest | null = null;
  if (stored) {
    manifest = await stored.json<Manifest>().catch(() => null);
    if (!isManifest(manifest)) {
      return reply({ status: 503, code: 'unavailable', message: 'This document cannot take responses right now.' });
    }
  }
  const refusal = checkRequest(parsed, manifest);
  if (refusal) return reply(refusal);
  const record = {
    action: parsed.action,
    version: parsed.version,
    checksum: parsed.checksum,
    recipientToken: parsed.recipientToken,
    ...(parsed.action === 'accept' ? { typedName: parsed.typedName, consent: true } : {}),
    note: parsed.action === 'request_changes' ? parsed.note : null,
    receivedAt: now().toISOString().replace(/\.\d{3}Z$/, 'Z'),
    ip: request.headers.get('CF-Connecting-IP'),
    userAgent: userAgent(request),
  };
  const written = await env.PRIVATE.put(
    responseKey(parsed.token, parsed.version, parsed.recipientToken),
    JSON.stringify(record),
    { onlyIf: { etagDoesNotMatch: '*' }, httpMetadata: { contentType: 'application/json' } },
  );
  if (written === null) return reply({ status: 409, code: 'already_responded', message: 'A response for this version is already recorded.' });
  return json({ ok: true, message: 'Received: thank you' }, 200);
}

export default { fetch: (request: Request, env: Env) => handle(request, env) };
