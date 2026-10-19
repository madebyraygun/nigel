import { describe, it, expect, beforeEach } from 'vitest';
import { handle, manifestKey, responseKey } from '../src/index.js';
import { MemoryBucket, CountingLimiter } from './memory-r2.js';
import fixture from './fixtures/page-contract.json';

// The fixture is the pages Nigel renders (render.rs
// `the_worker_contract_fixture_matches_the_rendered_pages` keeps it current).
// These tests run each page's own inline script against a stub DOM and post
// what it builds straight into `handle`, so the page and the Worker cannot
// drift apart on a field name, a token or a type.

const unescape = (value: string) =>
  value.replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&amp;/g, '&');

interface StubForm {
  hidden: boolean;
  dataset: Record<string, string>;
  values: Record<string, string>;
  status: { textContent: string };
  submit?: (event: { preventDefault(): void }) => void;
  addEventListener(type: string, listener: (event: { preventDefault(): void }) => void): void;
  querySelector(selector: string): { textContent: string };
  querySelectorAll(selector: string): { disabled: boolean }[];
}

function formsOf(html: string): StubForm[] {
  return [...html.matchAll(/<form ([^>]*data-endpoint=[^>]*)>/g)].map((match) => {
    const dataset: Record<string, string> = {};
    for (const [, name, value] of match[1].matchAll(/data-([a-z-]+)="([^"]*)"/g)) {
      dataset[name.replace(/-([a-z])/g, (_, c: string) => c.toUpperCase())] = unescape(value);
    }
    const form: StubForm = {
      hidden: true,
      dataset,
      values: {},
      status: { textContent: '' },
      addEventListener(type, listener) {
        if (type === 'submit') form.submit = listener;
      },
      querySelector: () => form.status,
      querySelectorAll: () => [],
    };
    return form;
  });
}

let bucket: MemoryBucket;
let env: { PRIVATE: MemoryBucket; RATE_LIMITER: CountingLimiter };

function load(recipientToken: string) {
  const page = fixture.pages.find((p) => p.recipientToken === recipientToken)!;
  const script = /<script>([\s\S]*?)<\/script>/.exec(page.html)![1];
  const forms = formsOf(page.html);
  const dataset = Object.fromEntries(
    [...page.html.matchAll(/ data-(accept|request-changes)="([^"]*)"/g)].map(
      (m) => [m[1] === 'accept' ? 'accept' : 'requestChanges', m[2]],
    ),
  );
  const received = { hidden: true, textContent: '', dataset };
  const posted: unknown[] = [];
  const document = {
    querySelectorAll: (selector: string) => (selector === 'form[data-endpoint]' ? forms : []),
    getElementById: (id: string) => (id === 'received' ? received : null),
  };
  class FormData {
    constructor(private form: StubForm) {}
    get(name: string) {
      return this.form.values[name] ?? null;
    }
  }
  const fetch = (url: string, init: RequestInit) => {
    posted.push(JSON.parse(init.body as string));
    return handle(new Request(url, { ...init, headers: { ...(init.headers as Record<string, string>), 'CF-Connecting-IP': '203.0.113.7' } }), env);
  };
  new Function('document', 'FormData', 'fetch', script)(document, FormData, fetch);
  return { forms, received, posted };
}

async function submit(form: StubForm) {
  form.submit!({ preventDefault() {} });
  for (let i = 0; i < 20; i++) await new Promise((resolve) => setTimeout(resolve, 0));
}

const [signer, collaborator] = fixture.recipients;

beforeEach(() => {
  bucket = new MemoryBucket();
  const manifest = { version: fixture.version, checksum: fixture.checksum, state: 'open', recipients: fixture.recipients };
  bucket.objects.set(manifestKey(fixture.token), JSON.stringify(manifest));
  env = { PRIVATE: bucket, RATE_LIMITER: new CountingLimiter(10) };
});

describe('the page Nigel publishes, posting to this Worker', () => {
  it("posts an accept the Worker records for the signer's own token", async () => {
    const { forms, received, posted } = load(signer.token);
    const accept = forms.find((f) => f.dataset.action === 'accept')!;
    accept.values = { typedName: signer.name, consent: 'on' };
    await submit(accept);

    expect(Object.keys(posted[0] as object).sort()).toEqual(
      ['action', 'checksum', 'consent', 'recipientToken', 'token', 'typedName', 'version'],
    );
    expect(accept.status.textContent).toBe('');
    expect(received.hidden).toBe(false);
    expect(received.textContent).toBe('Thank you, your acceptance has been received.');
    const record = JSON.parse(bucket.objects.get(responseKey(fixture.token, fixture.version, signer.token))!);
    expect(record).toMatchObject({
      action: 'accept', version: fixture.version, checksum: fixture.checksum,
      recipientToken: signer.token, typedName: signer.name, consent: true,
    });
  });

  it("posts a change request the Worker records for the collaborator's own token", async () => {
    const { forms, received, posted } = load(collaborator.token);
    expect(forms.map((f) => f.dataset.action)).toEqual(['request_changes']);
    forms[0].values = { note: 'Please split phase two.' };
    await submit(forms[0]);

    expect(Object.keys(posted[0] as object).sort()).toEqual(
      ['action', 'checksum', 'note', 'recipientToken', 'token', 'version'],
    );
    expect(received.hidden).toBe(false);
    expect(received.textContent).toBe('Thank you, your request has been received.');
    const record = JSON.parse(bucket.objects.get(responseKey(fixture.token, fixture.version, collaborator.token))!);
    expect(record).toMatchObject({ action: 'request_changes', recipientToken: collaborator.token, note: 'Please split phase two.' });
  });
});
