import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import './nigel-app.js';
import type { NigelApp } from './nigel-app.js';
import { appLocked, appUnauthorized, ApiError } from '../api/index.js';
import { resetAppStore } from '../state/app-store.js';
import {
  FakeApiClient,
  LOCKED_STATUS,
  UNINITIALIZED_STATUS,
} from '../__mocks__/fake-api-client.js';

/**
 * The whole app driven by FakeApiClient — which is itself the point: if the
 * root container can run on an ApiClient that never opens a socket, the seam
 * really is swappable for Tauri or a remote server.
 */
async function mount(client = new FakeApiClient()): Promise<NigelApp> {
  const el = document.createElement('nigel-app');
  el.client = client;
  document.body.appendChild(el);
  await el.updateComplete;
  // One more turn for the status promise to settle and re-render.
  await new Promise((r) => setTimeout(r, 0));
  await el.updateComplete;
  return el;
}

const shell = (el: NigelApp) => el.shadowRoot?.querySelector('wc-app-shell');
const sidebar = (el: NigelApp) => el.shadowRoot?.querySelector('wc-nav-sidebar');

/** jsdom's matchMedia answers false to everything, which is a wide viewport. */
function matchMediaReturning(matches: boolean): void {
  window.matchMedia = ((query: string) => ({
    matches,
    media: query,
    addEventListener: () => {},
    removeEventListener: () => {},
  })) as unknown as typeof window.matchMedia;
}

const phoneScreen = () => matchMediaReturning(true);
const widescreen = () => matchMediaReturning(false);

describe('nigel-app', () => {
  beforeEach(() => {
    resetAppStore();
    appLocked.set(false);
    appUnauthorized.set(false);
    window.location.hash = '';
  });

  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('shows a spinner until status arrives', async () => {
    const el = document.createElement('nigel-app');
    el.client = new FakeApiClient();
    document.body.appendChild(el);
    await el.updateComplete;

    expect(el.shadowRoot?.querySelector('wc-spinner')).toBeTruthy();
    expect(shell(el)).toBeNull();
  });

  it('renders the shell and sidebar once status arrives', async () => {
    const el = await mount();
    expect(shell(el)).toBeTruthy();
    expect(sidebar(el)).toBeTruthy();
  });

  it('seeds the hash so a reload lands on the same screen', async () => {
    await mount();
    expect(window.location.hash).toBe('#/dashboard');
  });

  it('renders the screen named by the hash', async () => {
    window.location.hash = '#/register';
    const el = await mount();
    expect(shell(el)?.getAttribute('screen-title')).toBe('Register');
  });

  it('swaps screens on hashchange', async () => {
    const el = await mount();
    expect(shell(el)?.getAttribute('screen-title')).toBe('Dashboard');

    window.location.hash = '#/reports';
    window.dispatchEvent(new HashChangeEvent('hashchange'));
    await el.updateComplete;

    expect(shell(el)?.getAttribute('screen-title')).toBe('Reports');
  });

  it('navigates by writing the hash, so back works', async () => {
    // The hash is the single writer of route state; a nav click only sets it.
    const el = await mount();
    sidebar(el)?.dispatchEvent(
      new CustomEvent('nc-navigate', {
        detail: { id: 'review' },
        bubbles: true,
        composed: true,
      }),
    );
    expect(window.location.hash).toBe('#/review');
  });

  it('falls back to the dashboard for an unknown hash', async () => {
    window.location.hash = '#/nonsense';
    const el = await mount();
    expect(shell(el)?.getAttribute('screen-title')).toBe('Dashboard');
  });

  it('titles the document with the screen and company', async () => {
    const el = await mount();
    expect(document.title).toBe('Dashboard · Test Consultancy');
    expect(el).toBeTruthy();
  });

  it('titles an open invoice innermost first', async () => {
    window.location.hash = '#/invoices?number=1254';
    await mount();
    expect(document.title).toBe('#1254 · Invoices · Test Consultancy');
  });

  describe('the top bar', () => {
    it('puts the book and the screen in the breadcrumb', async () => {
      window.location.hash = '#/register';
      const el = await mount();
      expect(shell(el)?.bookName).toBe('Test Consultancy');
      expect(shell(el)?.bookKey).toBe('/tmp/nigel');
      expect(shell(el)?.trail).toEqual([{ label: 'Register' }]);
    });

    it('adds the invoice level on an open invoice', async () => {
      window.location.hash = '#/invoices?number=1254';
      const el = await mount();
      expect(shell(el)?.trail).toEqual([
        { label: 'Invoices', id: 'invoices' },
        { label: '#1254' },
      ]);
    });

    it('goes back to the invoice list from the breadcrumb', async () => {
      window.location.hash = '#/invoices?number=1254';
      const el = await mount();
      const crumb = shell(el)?.shadowRoot?.querySelector('wc-breadcrumb');
      crumb?.shadowRoot?.querySelector<HTMLButtonElement>('[data-level="invoices"]')?.click();
      expect(window.location.hash).toBe('#/invoices');
    });

    it('lets the palette request out of the app for the palette to answer', async () => {
      const el = await mount();
      const scopes: string[] = [];
      const listen = (event: Event) =>
        scopes.push((event as CustomEvent<{ scope: string }>).detail.scope);
      document.addEventListener('nc-palette-open', listen);
      const crumb = shell(el)?.shadowRoot?.querySelector('wc-breadcrumb');
      crumb?.shadowRoot?.querySelector<HTMLButtonElement>('.book')?.click();
      const pill = shell(el)?.shadowRoot?.querySelector('wc-search-pill');
      pill?.shadowRoot?.querySelector<HTMLButtonElement>('button')?.click();
      document.removeEventListener('nc-palette-open', listen);
      expect(scopes).toEqual(['books', 'all']);
    });

    it('changes the scroll key with the screen, so main goes back to the top', async () => {
      const el = await mount();
      const before = shell(el)?.scrollKey;
      window.location.hash = '#/reports';
      window.dispatchEvent(new HashChangeEvent('hashchange'));
      await el.updateComplete;
      expect(shell(el)?.scrollKey).not.toBe(before);
    });

    it('reloads the screen, the status and the count on refresh', async () => {
      const client = new FakeApiClient();
      const el = await mount(client);
      const screenBefore = shell(el)?.querySelector('nigel-dashboard-screen');
      const statusCalls = client.calls.filter((c) => c === 'getStatus').length;
      const queueCalls = client.calls.filter((c) => c === 'getReviewQueue').length;

      shell(el)?.dispatchEvent(new CustomEvent('nc-refresh', { bubbles: true, composed: true }));
      await el.updateComplete;
      await new Promise((r) => setTimeout(r, 0));

      expect(client.calls.filter((c) => c === 'getStatus').length).toBe(statusCalls + 1);
      expect(client.calls.filter((c) => c === 'getReviewQueue').length).toBe(queueCalls + 1);
      const screenAfter = shell(el)?.querySelector('nigel-dashboard-screen');
      expect(screenAfter).not.toBe(screenBefore);
    });
  });

  describe('the dashboard title row', () => {
    const queued = (n: number) =>
      Array.from({ length: n }, (_, i) => ({
        id: i + 1,
        date: '2026-01-04',
        description: 'Acme',
        amount: -12,
        accountName: 'Harbor Checking',
      }));
    const pill = (el: NigelApp) =>
      el.shadowRoot?.querySelector<HTMLElement>('wc-review-pill');

    it('carries the review pill with the count, in the shell title row', async () => {
      const client = new FakeApiClient();
      client.reviewQueue = queued(2);
      const el = await mount(client);
      await vi.waitFor(() => expect(pill(el)).not.toBeNull());
      expect(pill(el)?.getAttribute('slot')).toBe('page-actions');
      expect((pill(el) as unknown as { count: number }).count).toBe(2);
    });

    it('has no pill when nothing needs review', async () => {
      const el = await mount();
      expect(pill(el)).toBeNull();
    });

    it('has no pill when the review count cannot be fetched', async () => {
      const client = new FakeApiClient();
      client.reviewQueue = queued(2);
      client.queueError = new Error('queue is down');
      const el = await mount(client);
      await vi.waitFor(() =>
        expect(client.calls).toContain('getReviewQueue'),
      );
      await el.updateComplete;
      expect(pill(el)).toBeNull();
    });

    it('drops the pill when a refresh finds the queue empty', async () => {
      const client = new FakeApiClient();
      client.reviewQueue = queued(1);
      const el = await mount(client);
      await vi.waitFor(() => expect(pill(el)).not.toBeNull());

      client.reviewQueue = [];
      shell(el)?.dispatchEvent(
        new CustomEvent('nc-refresh', { bubbles: true, composed: true }),
      );
      await vi.waitFor(() => expect(pill(el)).toBeNull());
    });

    it('has no pill on other screens', async () => {
      window.location.hash = '#/register';
      const client = new FakeApiClient();
      client.reviewQueue = queued(2);
      const el = await mount(client);
      expect(pill(el)).toBeNull();
    });
  });

  describe('the status line', () => {
    it('shows the folder, lock state and version from the status', async () => {
      const el = await mount();
      expect(shell(el)?.statusLine).toEqual({
        path: '/tmp/nigel',
        fullPath: '/tmp/nigel',
        encrypted: false,
        version: '0.0.0-test',
        distribution: undefined,
      });
    });

    it('is handed the review count the sidebar shows', async () => {
      const el = await mount();
      await vi.waitFor(() => {
        const count = sidebar(el)?.items.find((item) => item.id === 'review')?.count;
        expect(count).toBeTypeOf('number');
        expect(shell(el)?.reviewCount).toBe(count);
      });
    });
  });

  describe('hosted by the native shell', () => {
    beforeEach(() => {
      history.replaceState(null, '', '/?shell=native');
    });

    afterEach(() => {
      history.replaceState(null, '', '/');
    });

    it('renders no status line', async () => {
      const el = await mount();
      expect(shell(el)?.shadowRoot?.querySelector('wc-status-line')).toBeNull();
    });

    it('renders no sidebar and tells the shell there is none', async () => {
      const el = await mount();
      expect(sidebar(el)).toBeNull();
      expect(shell(el)?.hasAttribute('hide-sidebar')).toBe(true);
    });

    it('takes the flag from the hash route too, and keeps it from the screen', async () => {
      history.replaceState(null, '', '/');
      window.location.hash = '#/settings?shell=native';
      const el = await mount();
      expect(sidebar(el)).toBeNull();
      expect(shell(el)?.hasAttribute('hide-sidebar')).toBe(true);
      expect(shell(el)?.getAttribute('screen-title')).toBe('Settings');
    });

    it('keeps the breadcrumb top bar', async () => {
      const el = await mount();
      expect(shell(el)?.bookName).toBe('Test Consultancy');
      expect(shell(el)?.shadowRoot?.querySelector('header wc-breadcrumb')).toBeTruthy();
    });

    it('leaves the palette and its chord to the native shell', async () => {
      const el = await mount();
      const chord = new KeyboardEvent('keydown', {
        key: 'k',
        ctrlKey: true,
        bubbles: true,
        cancelable: true,
      });
      window.dispatchEvent(chord);
      await el.updateComplete;
      expect(chord.defaultPrevented).toBe(false);
      expect(el.shadowRoot?.querySelector('wc-command-palette')).toBeNull();
    });
  });

  describe('the palette', () => {
    const palette = (el: NigelApp) => el.shadowRoot!.querySelector('wc-command-palette')!;

    // jsdom reports no platform, which is not an Apple one: the chord is Ctrl+K.
    const chord = (init: KeyboardEventInit = { ctrlKey: true }) => {
      const event = new KeyboardEvent('keydown', {
        key: 'k',
        bubbles: true,
        composed: true,
        cancelable: true,
        ...init,
      });
      window.dispatchEvent(event);
      return event;
    };

    it('is shut until it is asked for', async () => {
      const el = await mount();
      expect(palette(el).open).toBe(false);
    });

    const onPlatform = (platform: string) =>
      Object.defineProperty(navigator, 'platform', { value: platform, configurable: true });

    afterEach(() => {
      onPlatform('');
    });

    it.each([
      ['Ctrl+K off Apple platforms', 'Linux x86_64', { ctrlKey: true }, { metaKey: true }],
      ['Cmd+K on Apple platforms', 'MacIntel', { metaKey: true }, { ctrlKey: true }],
    ])('toggles on %s, and only that chord', async (_name, platform, init, other) => {
      onPlatform(platform);
      const el = await mount();
      const wrong = chord(other);
      await el.updateComplete;
      expect(wrong.defaultPrevented).toBe(false);
      expect(palette(el).open).toBe(false);

      const event = chord(init);
      await el.updateComplete;
      expect(event.defaultPrevented).toBe(true);
      expect(palette(el).open).toBe(true);
      expect(palette(el).scope).toBe('all');
      expect(shell(el)?.hasAttribute('inert')).toBe(true);

      chord(init);
      await el.updateComplete;
      expect(palette(el).open).toBe(false);
      expect(shell(el)?.hasAttribute('inert')).toBe(false);
    });

    it('ignores a K with another modifier, or none, a held key and an IME composition', async () => {
      const el = await mount();
      chord({});
      chord({ ctrlKey: true, shiftKey: true });
      chord({ ctrlKey: true, repeat: true });
      chord({ ctrlKey: true, isComposing: true });
      await el.updateComplete;
      expect(palette(el).open).toBe(false);
    });

    it('stays shut over another dialog, but still closes itself', async () => {
      const el = await mount();
      const dialog = document.createElement('dialog');
      dialog.setAttribute('open', '');
      document.body.appendChild(dialog);
      const blocked = chord();
      await el.updateComplete;
      expect(blocked.defaultPrevented).toBe(false);
      expect(palette(el).open).toBe(false);
      dialog.remove();

      chord();
      await el.updateComplete;
      expect(palette(el).open).toBe(true);
      // Its own panel is a modal too; the chord still toggles it shut.
      chord();
      await el.updateComplete;
      expect(palette(el).open).toBe(false);
    });

    it('hands focus back to the control that asked, even one a click did not focus', async () => {
      const el = await mount();
      const pill = shell(el)?.shadowRoot?.querySelector('wc-search-pill');
      const button = pill?.shadowRoot?.querySelector<HTMLButtonElement>('button');
      // As WebKit does: the click opens the palette without focusing the button.
      button?.click();
      await el.updateComplete;
      expect(palette(el).opener).toBe(button);

      palette(el).dispatchEvent(new CustomEvent('nc-palette-close', { bubbles: true }));
      await el.updateComplete;
      await palette(el).updateComplete;
      expect(pill?.shadowRoot?.activeElement).toBe(button);
    });

    it.each([
      ['book', 'books'],
      ['hint', 'all'],
    ])('opens from the status line %s and hands focus back to it', async (part, scope) => {
      const el = await mount();
      const line = shell(el)?.shadowRoot?.querySelector('wc-status-line');
      await line?.updateComplete;
      const button = line?.shadowRoot?.querySelector<HTMLButtonElement>(`button.${part}`);
      expect(button).toBeTruthy();
      button?.click();
      await el.updateComplete;
      expect(palette(el).open).toBe(true);
      expect(palette(el).scope).toBe(scope);
      expect(palette(el).opener).toBe(button);

      palette(el).dispatchEvent(new CustomEvent('nc-palette-close', { bubbles: true }));
      await el.updateComplete;
      await palette(el).updateComplete;
      expect(line?.shadowRoot?.activeElement).toBe(button);
    });

    it('gives focus to the shell when the screen holding the opener is swapped out', async () => {
      const el = await mount();
      const field = document.createElement('input');
      shell(el)?.appendChild(field);
      field.focus();
      chord();
      await el.updateComplete;
      expect(palette(el).opener).toBe(field);

      palette(el).dispatchEvent(
        new CustomEvent('nc-palette-select', { detail: { kind: 'screen', id: 'reports' } }),
      );
      await el.updateComplete;
      field.remove();
      window.dispatchEvent(new HashChangeEvent('hashchange'));
      await el.updateComplete;
      await new Promise((r) => setTimeout(r, 0));
      expect(el.shadowRoot?.activeElement).toBe(shell(el));
    });

    it('opens from every screen', async () => {
      window.location.hash = '#/undo';
      const el = await mount();
      chord();
      await el.updateComplete;
      expect(palette(el).open).toBe(true);
    });

    it('opens scoped to books from the breadcrumb and unscoped from the pill', async () => {
      const el = await mount();
      const crumb = shell(el)?.shadowRoot?.querySelector('wc-breadcrumb');
      crumb?.shadowRoot?.querySelector<HTMLButtonElement>('.book')?.click();
      await el.updateComplete;
      expect(palette(el).open).toBe(true);
      expect(palette(el).scope).toBe('books');

      palette(el).dispatchEvent(new CustomEvent('nc-palette-close', { bubbles: true }));
      await el.updateComplete;
      const pill = shell(el)?.shadowRoot?.querySelector('wc-search-pill');
      pill?.shadowRoot?.querySelector<HTMLButtonElement>('button')?.click();
      await el.updateComplete;
      expect(palette(el).scope).toBe('all');
    });

    it('lists the open book from status and every screen in the sidebar', async () => {
      const el = await mount();
      expect(palette(el).books).toEqual([
        {
          id: '/tmp/nigel',
          name: 'Test Consultancy',
          kind: 'Business',
          path: '/tmp/nigel',
          encrypted: false,
          open: true,
        },
      ]);
      const labels = palette(el).screens.map((s) => s.label);
      expect(labels).toHaveLength(14);
      expect(labels).toContain('Undo last import');
    });

    it('navigates to a chosen screen and closes', async () => {
      const el = await mount();
      chord();
      await el.updateComplete;
      palette(el).dispatchEvent(
        new CustomEvent('nc-palette-select', { detail: { kind: 'screen', id: 'reports' } }),
      );
      await el.updateComplete;
      expect(window.location.hash).toBe('#/reports');
      expect(palette(el).open).toBe(false);
    });

    it('just closes when the open book is chosen', async () => {
      const el = await mount();
      chord();
      await el.updateComplete;
      palette(el).dispatchEvent(
        new CustomEvent('nc-palette-select', { detail: { kind: 'book', id: '/tmp/nigel' } }),
      );
      await el.updateComplete;
      expect(window.location.hash).toBe('#/dashboard');
      expect(palette(el).open).toBe(false);
    });

    it('stays shut while the database is locked', async () => {
      const client = new FakeApiClient();
      client.status = LOCKED_STATUS;
      const el = await mount(client);
      chord();
      await el.updateComplete;
      expect(el.shadowRoot?.querySelector('wc-command-palette')).toBeNull();
    });
  });

  describe('needs setup', () => {
    const freshClient = () => {
      const client = new FakeApiClient();
      client.status = UNINITIALIZED_STATUS;
      return client;
    };

    it('shows the setup gate instead of the app', async () => {
      const el = await mount(freshClient());
      expect(el.shadowRoot?.querySelector('nigel-setup-screen')).toBeTruthy();
    });

    it('renders no shell and no sidebar before the books exist', async () => {
      // The locked gate's reasoning, for the other empty state: with no shell
      // there is no screen element, so nothing can query a database that has
      // not been created yet.
      const el = await mount(freshClient());
      expect(shell(el)).toBeNull();
      expect(sidebar(el)).toBeNull();
    });

    it('fetches nothing but status before the books exist', async () => {
      // The whole acceptance criterion in one assertion.
      const client = freshClient();
      await mount(client);
      expect(client.calls).toEqual(['getStatus']);
    });
  });

  describe('locked', () => {
    const lockedClient = () => {
      const client = new FakeApiClient();
      client.status = LOCKED_STATUS;
      return client;
    };

    it('shows the unlock gate instead of the app', async () => {
      const el = await mount(lockedClient());
      expect(el.shadowRoot?.querySelector('nigel-unlock-screen')).toBeTruthy();
    });

    it('renders no shell and no sidebar while locked', async () => {
      // Structural, not cosmetic: with no shell there is no screen element, so
      // there is nothing that could fetch data before the password arrives.
      const el = await mount(lockedClient());
      expect(shell(el)).toBeNull();
      expect(sidebar(el)).toBeNull();
    });

    it('leaves the hash alone so unlocking returns the user where they were', async () => {
      window.location.hash = '#/reports';
      await mount(lockedClient());
      expect(window.location.hash).toBe('#/reports');
    });

    it('fetches nothing but status before the database is unlocked', async () => {
      // The whole acceptance criterion in one assertion: the only call the app
      // makes while locked is the one that told it it was locked.
      const client = lockedClient();
      await mount(client);
      expect(client.calls).toEqual(['getStatus']);
    });

    it('enters the app once the password lands', async () => {
      const client = lockedClient();
      const el = await mount(client);

      el.shadowRoot
        ?.querySelector('nigel-unlock-screen')
        ?.shadowRoot?.querySelector('wc-unlock-card')
        ?.dispatchEvent(
          new CustomEvent('nc-unlock', {
            detail: { password: 'hunter2' },
            bubbles: true,
            composed: true,
          }),
        );
      await new Promise((r) => setTimeout(r, 0));
      await el.updateComplete;

      // A prefix rather than the whole log: once the shell exists the screen
      // behind it starts loading its own data, and pinning the exact tail here
      // would fail for every screen that ever fetches anything. What this test
      // is about is the order of the first three — nothing before the unlock.
      expect(client.calls.slice(0, 3)).toEqual([
        'getStatus',
        'unlock:hunter2',
        'getStatus',
      ]);
      expect(shell(el)).toBeTruthy();
    });

    it('returns to the gate when any later call reports a locked database', async () => {
      const el = await mount();
      expect(shell(el)).toBeTruthy();

      appLocked.set(true);
      await el.updateComplete;

      expect(shell(el)).toBeNull();
      expect(el.shadowRoot?.querySelector('nigel-unlock-screen')).toBeTruthy();
    });
  });

  describe('banners', () => {
    it('explains a dead session, since the SPA cannot mint a token', async () => {
      const el = await mount();
      appUnauthorized.set(true);
      await el.updateComplete;

      const banner = el.shadowRoot?.querySelector('[slot="banner"]');
      expect(banner?.textContent).toContain('Session expired');
      expect(banner?.textContent).toContain('nigel serve');
    });

    it('offers a retry when status could not be loaded', async () => {
      const client = new FakeApiClient();
      client.statusError = new ApiError({
        code: 'internal',
        rawCode: 'internal',
        message: 'database is on fire',
        status: 500,
      });
      const el = await mount(client);

      const banner = el.shadowRoot?.querySelector('[slot="banner"]');
      expect(banner?.textContent).toContain('database is on fire');
      expect(banner?.querySelector('button')).toBeTruthy();
    });

    it('retries the status call when asked', async () => {
      const client = new FakeApiClient();
      client.statusError = new ApiError({
        code: 'internal',
        rawCode: 'internal',
        message: 'boom',
        status: 500,
      });
      const el = await mount(client);

      client.statusError = null;
      el.shadowRoot?.querySelector<HTMLButtonElement>('[slot="banner"] button')?.click();
      await new Promise((r) => setTimeout(r, 0));
      await el.updateComplete;

      expect(shell(el)?.getAttribute('screen-title')).toBe('Dashboard');
    });

    it('shows no banner when everything is fine', async () => {
      const el = await mount();
      expect(el.shadowRoot?.querySelector('[slot="banner"]')).toBeNull();
    });
  });

  /**
   * The one shortcut in the app, and the one nothing on screen advertises.
   * What is worth testing is mostly the refusals: a letter key bound at the
   * window is a bug in every context except the one it is meant for.
   */
  describe('the Snake easter egg', () => {
    const snake = (el: NigelApp) => el.shadowRoot?.querySelector('wc-snake');

    const typeS = (target: EventTarget = document.body, init: KeyboardEventInit = {}) =>
      target.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 's',
          bubbles: true,
          composed: true,
          cancelable: true,
          ...init,
        }),
      );

    it('is not on screen until it is asked for', async () => {
      const el = await mount();
      expect(snake(el)).toBeNull();
    });

    it('is nowhere in the visible UI', async () => {
      const el = await mount();
      const shellText = shell(el)?.textContent ?? '';
      const navText = sidebar(el)?.textContent ?? '';
      expect(`${shellText} ${navText}`.toLowerCase()).not.toContain('snake');
    });

    it('opens on s from the dashboard', async () => {
      const el = await mount();
      typeS();
      await el.updateComplete;

      expect(snake(el)).toBeTruthy();
      expect(snake(el)?.hasAttribute('fullscreen')).toBe(true);
    });

    it('makes the screen underneath inert while it is up', async () => {
      const el = await mount();
      expect(shell(el)?.hasAttribute('inert')).toBe(false);

      typeS();
      await el.updateComplete;
      expect(shell(el)?.hasAttribute('inert')).toBe(true);

      snake(el)?.dispatchEvent(
        new CustomEvent('nc-snake-exit', { bubbles: true, composed: true }),
      );
      await el.updateComplete;
      expect(shell(el)?.hasAttribute('inert')).toBe(false);
    });

    it('stays shut on any screen but the dashboard', async () => {
      const el = await mount();
      window.location.hash = '#/register';
      window.dispatchEvent(new HashChangeEvent('hashchange'));
      await el.updateComplete;

      typeS();
      await el.updateComplete;

      expect(snake(el)).toBeNull();
    });

    it('stays shut while the database is locked', async () => {
      const client = new FakeApiClient();
      client.status = LOCKED_STATUS;
      const el = await mount(client);

      typeS();
      await el.updateComplete;

      expect(snake(el)).toBeNull();
    });

    it('stays shut over the boot-failure screen', async () => {
      const client = new FakeApiClient();
      client.statusError = new ApiError({
        code: 'internal',
        rawCode: 'internal',
        message: 'database is on fire',
        status: 500,
      });
      const el = await mount(client);

      typeS();
      await el.updateComplete;

      expect(snake(el)).toBeNull();
    });

    /**
     * Every way out goes through the same close. A render branch that simply
     * stops rendering the overlay would leave the open flag and the captured
     * focus behind, and the next time that branch came back so would a game.
     */
    describe('closing', () => {
      it('closes when the route changes underneath it', async () => {
        const el = await mount();
        typeS();
        await el.updateComplete;
        expect(snake(el)).toBeTruthy();

        // The back button: the hash is the only writer of route state.
        window.location.hash = '#/register';
        window.dispatchEvent(new HashChangeEvent('hashchange'));
        await el.updateComplete;

        expect(snake(el)).toBeNull();
        expect(shell(el)?.hasAttribute('inert')).toBe(false);
      });

      it('closes when the database locks under it', async () => {
        const el = await mount();
        typeS();
        await el.updateComplete;
        expect(snake(el)).toBeTruthy();

        appLocked.set(true);
        await el.updateComplete;
        expect(snake(el)).toBeNull();

        // And unlocking returns the app, not a game nobody asked for twice.
        appLocked.set(false);
        await el.updateComplete;
        expect(snake(el)).toBeNull();
      });

      it('falls back to the shell when the element it captured is gone', async () => {
        const el = await mount();
        const link = document.createElement('a');
        link.href = '#/review';
        document.body.appendChild(link);
        link.focus();

        typeS(link);
        await el.updateComplete;

        // Whatever the route change unmounted, it took the link with it.
        link.remove();
        window.location.hash = '#/register';
        window.dispatchEvent(new HashChangeEvent('hashchange'));
        await el.updateComplete;
        await el.updateComplete;

        expect(el.shadowRoot?.activeElement).toBe(shell(el));
      });
    });

    it('stays shut when the keystroke belongs to a field', async () => {
      const el = await mount();
      const field = document.createElement('input');
      document.body.appendChild(field);
      field.focus();

      typeS(field);
      await el.updateComplete;

      expect(snake(el)).toBeNull();
      field.remove();
    });

    it('closes on the game asking to exit', async () => {
      const el = await mount();
      typeS();
      await el.updateComplete;

      snake(el)?.dispatchEvent(
        new CustomEvent('nc-snake-exit', { bubbles: true, composed: true }),
      );
      await el.updateComplete;

      expect(snake(el)).toBeNull();
    });

    it('gives the keyboard back to whatever had it', async () => {
      const el = await mount();
      const link = document.createElement('a');
      link.href = '#/review';
      document.body.appendChild(link);
      link.focus();
      expect(document.activeElement).toBe(link);

      typeS(link);
      await el.updateComplete;
      expect(document.activeElement).toBe(el);

      snake(el)?.dispatchEvent(
        new CustomEvent('nc-snake-exit', { bubbles: true, composed: true }),
      );
      await el.updateComplete;
      await el.updateComplete;

      expect(document.activeElement).toBe(link);
      link.remove();
    });
  });

  describe('the sidebar it owns', () => {
    // The sidebar is nigel-app's slotted child, so only nigel-app can pass
    // `collapsed` down; the shell asks by event and the answer comes back as a
    // property on both.
    afterEach(() => {
      widescreen();
    });

    it('shows the sidebar on a wide viewport', async () => {
      const el = await mount();
      expect(sidebar(el)?.hasAttribute('collapsed')).toBe(false);
      expect(shell(el)?.hasAttribute('sidebar-collapsed')).toBe(false);
    });

    it('starts with the sidebar away on a phone', async () => {
      // 232px of a 390px viewport, before a screen has drawn anything.
      phoneScreen();
      const el = await mount();
      expect(sidebar(el)?.hasAttribute('collapsed')).toBe(true);
      expect(shell(el)?.hasAttribute('sidebar-collapsed')).toBe(true);
    });

    it('puts the sidebar away when the shell asks', async () => {
      const el = await mount();

      shell(el)?.dispatchEvent(
        new CustomEvent('nc-sidebar-toggle', {
          detail: { collapsed: true },
          bubbles: true,
          composed: true,
        }),
      );
      await el.updateComplete;

      expect(sidebar(el)?.hasAttribute('collapsed')).toBe(true);
      expect(shell(el)?.hasAttribute('sidebar-collapsed')).toBe(true);
    });

    it('remembers the rail across a reload', async () => {
      localStorage.clear();
      const el = await mount();
      shell(el)?.dispatchEvent(
        new CustomEvent('nc-sidebar-toggle', {
          detail: { collapsed: true },
          bubbles: true,
          composed: true,
        }),
      );
      await el.updateComplete;
      el.remove();
      resetAppStore();

      const again = await mount();
      expect(sidebar(again)?.hasAttribute('collapsed')).toBe(true);
      localStorage.clear();
    });

    it('does not remember a drawer shut on a phone as a folded rail', async () => {
      localStorage.clear();
      phoneScreen();
      const el = await mount();
      shell(el)?.dispatchEvent(
        new CustomEvent('nc-sidebar-toggle', {
          detail: { collapsed: true },
          bubbles: true,
          composed: true,
        }),
      );
      await el.updateComplete;
      expect(localStorage.getItem('nigel.sidebar-collapsed')).toBeNull();
    });

    it('lets the sidebar hamburger fold it to the rail', async () => {
      const el = await mount();
      sidebar(el)
        ?.shadowRoot?.querySelector<HTMLButtonElement>('.hamburger')
        ?.click();
      await el.updateComplete;
      expect(sidebar(el)?.hasAttribute('collapsed')).toBe(true);
      localStorage.clear();
    });

    it('shows the review queue as the Review count', async () => {
      const client = new FakeApiClient();
      client.reviewQueue = [{ id: 1 }, { id: 2 }, { id: 3 }] as typeof client.reviewQueue;
      const el = await mount(client);
      await new Promise((r) => setTimeout(r, 0));
      await el.updateComplete;
      const review = sidebar(el)?.items.find((item) => item.id === 'review');
      expect(review?.count).toBe(3);
    });

    it('brings the sidebar back when the shell asks', async () => {
      phoneScreen();
      const el = await mount();

      shell(el)?.dispatchEvent(
        new CustomEvent('nc-sidebar-toggle', {
          detail: { collapsed: false },
          bubbles: true,
          composed: true,
        }),
      );
      await el.updateComplete;

      expect(sidebar(el)?.hasAttribute('collapsed')).toBe(false);
    });
  });
});
