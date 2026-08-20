import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import './setup.js';
import type { NigelSetupScreen } from './setup.js';
import { ApiError, appLocked } from '../api/index.js';
import { initializeAppStore, resetAppStore } from '../state/app-store.js';
import { FakeApiClient, UNINITIALIZED_STATUS } from '../__mocks__/fake-api-client.js';

/**
 * The setup screen driven entirely by FakeApiClient — no network, no server,
 * and every assertion is about which api method the screen chose to call with
 * what.
 */
let reloads = 0;

async function mount(client = new FakeApiClient()): Promise<{
  el: NigelSetupScreen;
  client: FakeApiClient;
}> {
  reloads = 0;
  client.status = UNINITIALIZED_STATUS;
  const store = initializeAppStore(client, { reload: () => (reloads += 1) });
  await store.refreshStatus();
  client.calls.length = 0;

  const el = document.createElement('nigel-setup-screen');
  document.body.appendChild(el);
  await el.updateComplete;
  return { el, client };
}

async function settle(el: NigelSetupScreen): Promise<void> {
  await new Promise((r) => setTimeout(r, 0));
  await el.updateComplete;
}

const button = (el: NigelSetupScreen, text: string) =>
  [...(el.shadowRoot?.querySelectorAll('wa-button') ?? [])].find((b) =>
    b.textContent?.includes(text),
  ) as HTMLElement | undefined;

const wordmark = (el: NigelSetupScreen) =>
  el.shadowRoot?.querySelector('wc-wordmark') as (HTMLElement & { reveal: number }) | undefined;

/** The arrival's own elements — the ones that enter after the wordmark. */
const introNodes = (el: NigelSetupScreen) => [
  ...(el.shadowRoot?.querySelectorAll('.intro') ?? []),
];

const field = (el: NigelSetupScreen, label: string) =>
  [...(el.shadowRoot?.querySelectorAll('wa-input') ?? [])].find(
    (i) => i.getAttribute('label') === label,
  ) as (HTMLElement & { value: string }) | undefined;

/** Type into a field the way a user does — the screen reads the input event. */
async function typeInto(el: NigelSetupScreen, label: string, value: string): Promise<void> {
  const input = field(el, label);
  if (!input) throw new Error(`no field labelled ${label}`);
  input.value = value;
  input.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
  await el.updateComplete;
}

/** Answer the motion query the gate and its components both read. */
function preferReducedMotion(): void {
  vi.spyOn(window, 'matchMedia').mockImplementation(
    (query: string) =>
      ({
        matches: query.includes('prefers-reduced-motion'),
        media: query,
        onchange: null,
        addEventListener: () => {},
        removeEventListener: () => {},
        addListener: () => {},
        removeListener: () => {},
        dispatchEvent: () => false,
      }) as unknown as MediaQueryList,
  );
}

/** Land the intro the way a key does, so the arrival's buttons are live. */
async function landIntro(el: NigelSetupScreen): Promise<void> {
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', bubbles: true }));
  await el.updateComplete;
}

/** Walk arrival -> profile, choosing business or personal. */
async function toIdentity(el: NigelSetupScreen, profile: 'business' | 'personal') {
  await landIntro(el);
  button(el, 'Set up my books')?.click();
  await el.updateComplete;
  (el.shadowRoot?.querySelector(`[data-profile="${profile}"]`) as HTMLElement)?.click();
  await el.updateComplete;
}

async function toFirstMove(el: NigelSetupScreen) {
  await toIdentity(el, 'business');
  await typeInto(el, 'Your name', 'Marta');
  await typeInto(el, 'Business name', 'Cedar Systems');
  button(el, 'Carry on')?.click();
  await el.updateComplete;
}

describe('setup screen', () => {
  beforeEach(() => {
    resetAppStore();
    appLocked.set(false);
  });

  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it("opens on the arrival, in Nigel's voice", async () => {
    const { el } = await mount();
    expect(el.shadowRoot?.textContent).toContain("Hello. I'm Nigel.");
    expect(el.shadowRoot?.querySelector('wc-wordmark')).not.toBeNull();
  });

  it('holds the arrival copy back until the wordmark is drawn', async () => {
    // The panel is one animation, not a drawn mark above copy that was there
    // all along: everything the reader can act on waits for the assembly.
    const { el } = await mount();
    expect(wordmark(el)?.reveal).toBeLessThan(1);
    for (const node of introNodes(el)) {
      expect(node.classList.contains('intro-ready')).toBe(false);
      expect(node.classList.contains('intro-instant')).toBe(false);
    }
  });

  it('fades the copy in once the reveal finishes', async () => {
    vi.useFakeTimers();
    try {
      const { el } = await mount();
      vi.advanceTimersByTime(1200);
      await el.updateComplete;

      expect(wordmark(el)?.reveal).toBe(1);
      for (const node of introNodes(el)) {
        expect(node.classList.contains('intro-ready')).toBe(true);
      }
    } finally {
      vi.useRealTimers();
    }
  });

  it('lands the intro on a keypress without leaving the arrival', async () => {
    // The key skips the theatrics, not the question. Somebody who has seen the
    // assembly once gets the panel at once, on the step they were already on.
    const { el } = await mount();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', bubbles: true }));
    await el.updateComplete;

    expect(el.shadowRoot?.textContent).toContain("Hello. I'm Nigel.");
    expect(el.shadowRoot?.textContent).not.toContain('Who are we keeping books for?');
    expect(wordmark(el)?.reveal).toBe(1);
    for (const node of introNodes(el)) {
      expect(node.classList.contains('intro-instant')).toBe(true);
    }
  });

  it('stops the ticker when the intro is landed early', async () => {
    vi.useFakeTimers();
    try {
      const { el } = await mount();
      expect(vi.getTimerCount()).toBeGreaterThan(0);

      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', bubbles: true }));
      await el.updateComplete;

      expect(vi.getTimerCount()).toBe(0);
    } finally {
      vi.useRealTimers();
    }
  });

  it('leaves the arrival only when asked to', async () => {
    const { el } = await mount();
    await landIntro(el);

    button(el, 'Set up my books')?.click();
    await el.updateComplete;

    expect(el.shadowRoot?.textContent).toContain('Who are we keeping books for?');
  });

  it('stops listening for that key once the gate is gone', async () => {
    // A window listener outlives the element it was registered from unless it
    // is taken off, and this one drives the reveal.
    const { el } = await mount();
    el.remove();

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', bubbles: true }));
    await el.updateComplete;

    expect(wordmark(el)?.reveal).toBeLessThan(1);
  });

  it('runs a reveal ticker so the wordmark assembles', async () => {
    vi.useFakeTimers();
    try {
      await mount();
      expect(vi.getTimerCount()).toBeGreaterThan(0);
    } finally {
      vi.useRealTimers();
    }
  });

  it('starts no ticker when motion is unwelcome', async () => {
    // `wc-wordmark` draws itself whole in that case and ignores `reveal`
    // entirely, so the interval would re-render the gate thirty times over to
    // no visible effect.
    preferReducedMotion();
    vi.useFakeTimers();
    try {
      const { el } = await mount();
      expect(vi.getTimerCount()).toBe(0);
      expect(el.shadowRoot?.textContent).toContain("Hello. I'm Nigel.");
    } finally {
      vi.useRealTimers();
    }
  });

  it('asks nothing of the server before the last step', async () => {
    const { el, client } = await mount();
    await toFirstMove(el);
    expect(client.calls).toEqual([]);
  });

  it('labels the company field from the profile', async () => {
    const { el } = await mount();
    await toIdentity(el, 'business');
    expect(field(el, 'Business name')?.getAttribute('hint')).toBe(
      'It goes on your reports and invoices.',
    );
  });

  it('labels it for a household on personal books', async () => {
    const { el } = await mount();
    await toIdentity(el, 'personal');
    expect(field(el, 'Household name')?.getAttribute('hint')).toBe('It goes on your reports.');
    expect(field(el, 'Business name')).toBeUndefined();
  });

  it('asks for the password twice only once one has been typed', async () => {
    const { el } = await mount();
    await toIdentity(el, 'business');
    expect(field(el, 'Type it again')).toBeUndefined();

    await typeInto(el, 'Password (optional but recommended)', 'hunter2');

    expect(field(el, 'Type it again')).toBeDefined();
  });

  it('refuses to move on when the two passwords differ', async () => {
    const { el } = await mount();
    await toIdentity(el, 'business');
    await typeInto(el, 'Your name', 'Marta');
    await typeInto(el, 'Business name', 'Cedar Systems');
    await typeInto(el, 'Password (optional but recommended)', 'hunter2');
    await typeInto(el, 'Type it again', 'hunter3');

    button(el, 'Carry on')?.click();
    await el.updateComplete;

    expect(el.shadowRoot?.textContent).toContain("Those two don't match");
    expect(el.shadowRoot?.textContent).not.toContain('How shall we start?');
  });

  it('sends the fresh plan exactly as the route expects it', async () => {
    const { el, client } = await mount();
    await toFirstMove(el);

    button(el, 'Start fresh')?.click();
    await settle(el);

    expect(client.calls).toContain(
      `setup:${JSON.stringify({
        userName: 'Marta',
        companyName: 'Cedar Systems',
        profile: 'business',
        action: 'fresh',
      })}`,
    );
  });

  it('sends the demo plan with the demo action', async () => {
    const { el, client } = await mount();
    await landIntro(el);

    button(el, 'Show me the demo')?.click();
    await settle(el);

    expect(client.calls.some((c) => c.startsWith('setup:') && c.includes('"action":"demo"'))).toBe(
      true,
    );
  });

  it('carries the password through when one was set', async () => {
    const { el, client } = await mount();
    await toIdentity(el, 'business');
    await typeInto(el, 'Your name', 'Marta');
    await typeInto(el, 'Business name', 'Cedar Systems');
    await typeInto(el, 'Password (optional but recommended)', 'hunter2');
    await typeInto(el, 'Type it again', 'hunter2');
    button(el, 'Carry on')?.click();
    await el.updateComplete;

    button(el, 'Start fresh')?.click();
    await settle(el);

    expect(client.calls.some((c) => c.includes('"password":"hunter2"'))).toBe(true);
  });

  it('delegates the load path to the data-directory switch', async () => {
    // Loading existing books is the settings screen's switch, not a second
    // implementation of it: it already validates, migrates and rebinds.
    const { el, client } = await mount();
    await toFirstMove(el);

    await typeInto(el, 'Data directory', '~/Documents/nigel');
    button(el, 'Load them')?.click();
    await settle(el);

    expect(client.calls).toContain('setDataDir');
    expect(client.calls.some((c) => c.startsWith('setup:'))).toBe(false);
    expect(reloads).toBe(1);
  });

  it('asks for a path rather than posting an empty one', async () => {
    const { el, client } = await mount();
    await toFirstMove(el);

    button(el, 'Load them')?.click();
    await settle(el);

    expect(el.shadowRoot?.textContent).toContain('I need a directory to look in.');
    expect(client.calls).not.toContain('setDataDir');
  });

  it('surfaces a refused setup and stays where it is', async () => {
    const { el, client } = await mount();
    client.setupError = new ApiError({
      code: 'conflict',
      rawCode: 'conflict',
      message: 'These books are already set up.',
      status: 409,
      details: { reason: 'already_initialized' },
    });
    await toFirstMove(el);

    button(el, 'Start fresh')?.click();
    await settle(el);

    expect(el.shadowRoot?.textContent).toContain('These books are already set up.');
    expect(el.shadowRoot?.textContent).toContain('How shall we start?');
  });

  it("surfaces the server's own sentence when a directory has no books", async () => {
    const { el, client } = await mount();
    client.settingsError = new ApiError({
      code: 'bad_request',
      rawCode: 'bad_request',
      message: 'No database found at /nope/nigel.db.',
      status: 400,
    });
    await toFirstMove(el);

    await typeInto(el, 'Data directory', '/nope');
    button(el, 'Load them')?.click();
    await settle(el);

    expect(el.shadowRoot?.textContent).toContain('No database found at /nope/nigel.db.');
    expect(reloads).toBe(0);
  });

  it('refuses to step back while a request is in flight', async () => {
    // A step change mid-request would land the failure sentence under the
    // identity step, where it reads as a complaint about the name.
    const { el, client } = await mount();
    vi.spyOn(client, 'setup').mockImplementation(() => new Promise(() => {}));
    await toFirstMove(el);

    button(el, 'Start fresh')?.click();
    await el.updateComplete;

    expect(button(el, 'Back')?.hasAttribute('disabled')).toBe(true);
  });

  it('goes back a step without losing what was typed', async () => {
    const { el } = await mount();
    await toIdentity(el, 'business');
    await typeInto(el, 'Your name', 'Marta');
    await typeInto(el, 'Business name', 'Cedar Systems');
    button(el, 'Carry on')?.click();
    await el.updateComplete;

    button(el, 'Back')?.click();
    await el.updateComplete;

    expect(field(el, 'Your name')!.value).toBe('Marta');
  });
});
