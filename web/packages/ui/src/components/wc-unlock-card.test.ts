import { describe, it, expect, afterEach } from 'vitest';
import './wc-unlock-card.js';
import type { WcUnlockCard, NcUnlockDetail } from './wc-unlock-card.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-unlock-card.preview.js';

async function mount(props: Partial<WcUnlockCard> = {}): Promise<WcUnlockCard> {
  const el = document.createElement('wc-unlock-card');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function passwordField(el: WcUnlockCard): HTMLElement & { value: string } {
  const input = el.shadowRoot?.querySelector('wa-input');
  if (!input) throw new Error('no password field');
  return input as HTMLElement & { value: string };
}

function submit(el: WcUnlockCard): void {
  el.shadowRoot?.querySelector('form')?.dispatchEvent(
    new Event('submit', { cancelable: true, bubbles: false }),
  );
}

/**
 * Everything a reader of the page could reach under `root`: text, attribute
 * names and values, and form-control values, descending into every shadow
 * root. `outerHTML` stops at the host and would miss all of it.
 */
function reachableText(root: Node): string {
  const parts: string[] = [];
  const visit = (node: Node): void => {
    if (node.nodeType === Node.TEXT_NODE) parts.push(node.textContent ?? '');
    if (node instanceof Element) {
      for (const attr of node.attributes) parts.push(attr.name, attr.value);
      const value = (node as Element & { value?: unknown }).value;
      if (typeof value === 'string') parts.push(value);
      if (node.shadowRoot) visit(node.shadowRoot);
    }
    node.childNodes.forEach(visit);
  };
  visit(root);
  return parts.join('\n');
}

describe('wc-unlock-card', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('emits the typed password on submit', async () => {
    const el = await mount();
    const seen: NcUnlockDetail[] = [];
    el.addEventListener('nc-unlock', (e) => seen.push((e as CustomEvent<NcUnlockDetail>).detail));

    passwordField(el).value = 'hunter2';
    submit(el);

    expect(seen).toEqual([{ password: 'hunter2' }]);
  });

  it('clears the field once the password is on its way', async () => {
    // A failed attempt should not leave the password sitting in the DOM.
    const el = await mount();
    passwordField(el).value = 'hunter2';
    submit(el);
    expect(passwordField(el).value).toBe('');
  });

  it('can see a password rendered anywhere inside its shadow tree', async () => {
    const el = await mount();
    const leak = document.createElement('span');
    leak.setAttribute('data-leak', 'hunter2');
    el.shadowRoot?.appendChild(leak);
    expect(reachableText(el)).toContain('hunter2');
  });

  it('never renders the password into the DOM', async () => {
    const el = await mount();
    passwordField(el).value = 'hunter2';
    expect(reachableText(el)).toContain('hunter2');

    submit(el);
    expect(reachableText(el)).not.toContain('hunter2');

    el.busy = true;
    el.error = 'Wrong password.';
    el.attemptsRemaining = 2;
    await el.updateComplete;
    expect(reachableText(el)).not.toContain('hunter2');
  });

  it('does not submit an empty password', async () => {
    const el = await mount();
    let fired = 0;
    el.addEventListener('nc-unlock', () => (fired += 1));
    submit(el);
    expect(fired).toBe(0);
  });

  it('does not submit while a request is in flight', async () => {
    const el = await mount({ busy: true });
    let fired = 0;
    el.addEventListener('nc-unlock', () => (fired += 1));
    passwordField(el).value = 'hunter2';
    submit(el);
    expect(fired).toBe(0);
  });

  it('shows the error and how many attempts are left', async () => {
    const el = await mount({ error: 'Wrong password.', attemptsRemaining: 2 });
    const status = el.shadowRoot?.querySelector('.status')?.textContent ?? '';
    expect(status).toContain('Wrong password.');
    expect(status).toContain('2 attempts remaining');
  });

  it('says attempt, singular, when one is left', async () => {
    const el = await mount({ error: 'Wrong password.', attemptsRemaining: 1 });
    expect(el.shadowRoot?.querySelector('.attempts')?.textContent).toContain(
      '1 attempt remaining',
    );
  });

  it('claims nothing about attempts when the server did not say', async () => {
    const el = await mount({ error: 'Could not reach the nigel server.' });
    expect(el.shadowRoot?.querySelector('.attempts')).toBeNull();
  });

  it('counts down the throttle while the request is in flight', async () => {
    // The server serves the delay before answering, so the countdown belongs to
    // the in-flight request rather than to a cooldown after it.
    const el = await mount({ busy: true, countdownSeconds: 4 });
    expect(el.shadowRoot?.querySelector('.countdown')?.textContent).toContain(
      'checking in 4s',
    );
  });

  it('announces status changes politely', async () => {
    const el = await mount({ error: 'Wrong password.' });
    const status = el.shadowRoot?.querySelector('.status');
    expect(status?.getAttribute('aria-live')).toBe('polite');
  });
});

describePreviewA11y(preview);
