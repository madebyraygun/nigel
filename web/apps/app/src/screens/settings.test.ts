import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import './settings.js';
import type { NigelSettingsScreen } from './settings.js';
import { ApiError, appLocked } from '../api/index.js';
import { initializeAppStore, resetAppStore } from '../state/app-store.js';
import { FakeApiClient } from '../__mocks__/fake-api-client.js';

/**
 * The settings screen driven entirely by FakeApiClient — no network, no server,
 * and every assertion is about which api method the screen chose to call.
 */
let reloads = 0;

async function mount(client = new FakeApiClient()): Promise<{
  el: NigelSettingsScreen;
  client: FakeApiClient;
}> {
  reloads = 0;
  const store = initializeAppStore(client, { reload: () => (reloads += 1) });
  await store.refreshStatus();
  // The store's own bookkeeping call is not what these tests are about.
  client.calls.length = 0;

  const el = document.createElement('nigel-settings-screen');
  el.client = client;
  document.body.appendChild(el);
  await el.updateComplete;
  await new Promise((r) => setTimeout(r, 0));
  await el.updateComplete;
  return { el, client };
}

const input = (el: NigelSettingsScreen, index: number) =>
  el.shadowRoot?.querySelectorAll('wa-input')[index] as HTMLElement & { value: string };

/** Fields move as settings are added; the label is the stable handle. */
const inputLabelled = (el: NigelSettingsScreen, label: string) =>
  [...(el.shadowRoot?.querySelectorAll('wa-input') ?? [])].find(
    (i) => i.getAttribute('label') === label,
  ) as HTMLElement & { value: string };

/**
 * Panels move as settings are added; the heading is the stable handle. The
 * action slot is named explicitly because a panel may hold other buttons — the
 * letterhead's Remove-logo control sits above its Save.
 */
function buttonIn(el: NigelSettingsScreen, heading: string) {
  const found = [...(el.shadowRoot?.querySelectorAll('wc-panel') ?? [])].find(
    (p) => p.getAttribute('heading') === heading,
  );
  return found?.querySelector('wa-button[slot="actions"]') as HTMLElement | undefined;
}

/** The panel itself, for assertions about what one panel is showing. */
function panel(el: NigelSettingsScreen, heading: string) {
  return [...(el.shadowRoot?.querySelectorAll('wc-panel') ?? [])].find(
    (p) => p.getAttribute('heading') === heading,
  );
}

async function settle(el: NigelSettingsScreen): Promise<void> {
  await new Promise((r) => setTimeout(r, 0));
  await el.updateComplete;
}

describe('settings screen', () => {
  beforeEach(() => {
    resetAppStore();
    appLocked.set(false);
  });

  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('loads the application settings when it mounts', async () => {
    const { client } = await mount();
    expect(client.calls).toContain('getAppSettings');
  });

  it('shows the active data directory', async () => {
    const { el } = await mount();
    expect(el.shadowRoot?.textContent).toContain('/tmp/nigel');
  });

  it('labels the name field from the books profile', async () => {
    const { el } = await mount();
    expect(input(el, 0).getAttribute('label')).toBe('Business name');

    const client = new FakeApiClient();
    client.status = { ...client.status, profile: 'personal' };
    const { el: personal } = await mount(client);
    expect(input(personal, 0).getAttribute('label')).toBe('Household name');
  });

  describe('the letterhead', () => {
    const textarea = (el: NigelSettingsScreen, index: number) =>
      el.shadowRoot?.querySelectorAll('wa-textarea')[index] as HTMLElement & {
        value: string;
      };

    it('seeds all five fields once and does not clobber typing', async () => {
      const client = new FakeApiClient();
      client.company = {
        name: 'Bluepeak LLC',
        address: 'P.O. Box 1234',
        phone: '619.555.0123',
        logo: '',
        paymentInstructions: 'Wells Fargo',
      };
      const { el } = await mount(client);

      expect(client.calls).toContain('getCompany');
      expect(input(el, 0).value).toBe('Bluepeak LLC');
      expect(input(el, 1).value).toBe('619.555.0123');
      expect(textarea(el, 0).value).toBe('P.O. Box 1234');
      expect(textarea(el, 1).value).toBe('Wells Fargo');

      // A store refresh must not re-seed the form under someone typing.
      input(el, 0).value = 'Bluepeak Studio';
      input(el, 0).dispatchEvent(new Event('input'));
      await el.updateComplete;
      await client.getStatus();
      await el.updateComplete;
      expect(input(el, 0).value).toBe('Bluepeak Studio');
    });

    it('saves the whole letterhead in one request', async () => {
      const { el, client } = await mount();
      input(el, 0).value = 'Bluepeak LLC';
      input(el, 0).dispatchEvent(new Event('input'));
      textarea(el, 0).value = 'P.O. Box 1234';
      textarea(el, 0).dispatchEvent(new Event('input'));
      textarea(el, 1).value = 'Wells Fargo';
      textarea(el, 1).dispatchEvent(new Event('input'));
      await el.updateComplete;

      buttonIn(el, 'Letterhead')?.click();
      await settle(el);

      expect(client.calls.filter((c) => c === 'setCompany')).toHaveLength(1);
      expect(client.company.name).toBe('Bluepeak LLC');
      expect(client.company.address).toBe('P.O. Box 1234');
      expect(client.company.paymentInstructions).toBe('Wells Fargo');
      // The name is shown from status, so a save that skipped the refresh would
      // leave a stale sidebar behind it.
      expect(client.calls.lastIndexOf('getStatus')).toBeGreaterThan(
        client.calls.indexOf('setCompany'),
      );
      expect(client.status.companyName).toBe('Bluepeak LLC');
    });

    it('surfaces the server refusal for a bad logo without clearing the form', async () => {
      const client = new FakeApiClient();
      const { el } = await mount(client);
      input(el, 0).value = 'Bluepeak LLC';
      input(el, 0).dispatchEvent(new Event('input'));
      await el.updateComplete;

      client.companyError = new ApiError({
        code: 'bad_request',
        rawCode: 'bad_request',
        message: 'A logo of type image/svg+xml cannot be used.',
        status: 400,
      });
      buttonIn(el, 'Letterhead')?.click();
      await settle(el);

      expect(el.shadowRoot?.textContent).toContain('image/svg+xml');
      expect(input(el, 0).value).toBe('Bluepeak LLC');
      expect(client.company.name).toBe('Test Consultancy');
    });

    it('reads a chosen file as a data URI before sending', async () => {
      const { el, client } = await mount();
      const file = new File([new Uint8Array([1, 2, 3])], 'logo.png', {
        type: 'image/png',
      });
      const picker = el.shadowRoot?.querySelector(
        'input[type="file"]',
      ) as HTMLInputElement;
      expect(picker, 'the logo picker is on the screen').toBeTruthy();
      Object.defineProperty(picker, 'files', { value: [file], configurable: true });
      picker.dispatchEvent(new Event('change'));
      // FileReader resolves on its own task; the save must see the result.
      await vi.waitFor(() => {
        expect(el.shadowRoot?.querySelector('.logo-preview')).toBeTruthy();
      });
      await settle(el);

      buttonIn(el, 'Letterhead')?.click();
      await settle(el);

      expect(client.company.logo).toMatch(/^data:/);
    });

    it('offers a retry when the letterhead cannot be loaded', async () => {
      const client = new FakeApiClient();
      client.companyLoadError = new ApiError({
        code: 'internal',
        rawCode: 'internal',
        message: 'the database is unreadable',
        status: 500,
      });
      const { el } = await mount(client);

      // A failed load is its own state, never a form the data could not fill.
      const letterhead = panel(el, 'Letterhead');
      expect(letterhead?.querySelector('wc-spinner')).toBeFalsy();
      expect(letterhead?.querySelector('wa-input')).toBeFalsy();
      expect(letterhead?.textContent).toContain('the database is unreadable');
      expect(buttonIn(el, 'Letterhead')?.textContent).toContain('Retry');

      client.companyLoadError = null;
      buttonIn(el, 'Letterhead')?.click();
      await settle(el);

      expect(client.calls.filter((c) => c === 'getCompany')).toHaveLength(2);
      expect(input(el, 0).value).toBe('Test Consultancy');
    });

    it('refuses an oversized logo before it reaches the server', async () => {
      const { el, client } = await mount();
      const oversized = new File([new Uint8Array(200 * 1024)], 'huge.png', {
        type: 'image/png',
      });
      const picker = el.shadowRoot?.querySelector(
        'input[type="file"]',
      ) as HTMLInputElement;
      Object.defineProperty(picker, 'files', { value: [oversized], configurable: true });
      picker.dispatchEvent(new Event('change'));
      await settle(el);

      // Named, so it is a refusal about the file that was chosen and not the
      // standing note about the cap.
      expect(el.shadowRoot?.textContent).toContain('huge.png');
      expect(el.shadowRoot?.querySelector('.logo-preview')).toBeFalsy();

      buttonIn(el, 'Letterhead')?.click();
      await settle(el);
      expect(client.company.logo).toBe('');
    });

    it('reports a failed save without changing what is shown', async () => {
      const client = new FakeApiClient();
      const { el } = await mount(client);
      client.companyError = new ApiError({
        code: 'internal',
        rawCode: 'internal',
        message: 'disk is full',
        status: 500,
      });

      buttonIn(el, 'Letterhead')?.click();
      await settle(el);

      expect(client.status.companyName).toBe('Test Consultancy');
    });
  });

  describe('update check', () => {
    it('writes the new value', async () => {
      const { el, client } = await mount();
      const toggle = el.shadowRoot?.querySelector('wa-switch') as HTMLElement & {
        checked: boolean;
      };
      toggle.checked = false;
      toggle.dispatchEvent(new Event('change'));
      await settle(el);

      expect(client.calls).toContain('updateAppSettings');
      expect(client.appSettings.updateCheck).toBe(false);
    });

    it('puts the switch back when the write fails', async () => {
      const client = new FakeApiClient();
      const { el } = await mount(client);
      expect(client.appSettings.updateCheck).toBe(true);
      client.settingsError = new ApiError({
        code: 'internal',
        rawCode: 'internal',
        message: 'nope',
        status: 500,
      });

      const toggle = el.shadowRoot?.querySelector('wa-switch') as HTMLElement & {
        checked: boolean;
      };
      toggle.checked = false;
      toggle.dispatchEvent(new Event('change'));
      await settle(el);

      // A control whose write failed must not keep claiming it succeeded.
      // Asserted on the property, not the attribute: the property is what the
      // user moved, and re-rendering the old value does not move it back.
      expect(toggle.checked).toBe(true);
      expect(client.appSettings.updateCheck).toBe(true);
    });
  });

  describe('data directory', () => {
    it('asks before switching, and does nothing when refused', async () => {
      const ui = await import('@nigel/ui');
      vi.spyOn(ui, 'confirmDialog').mockResolvedValue(false);

      const { el, client } = await mount();
      const field = inputLabelled(el, 'Switch to');
      field.value = '/tmp/other';
      field.dispatchEvent(new Event('input'));
      await el.updateComplete;

      buttonIn(el, 'Data directory')?.click();
      await settle(el);

      expect(client.calls).not.toContain('setDataDir');
    });

    it('switches once confirmed', async () => {
      const ui = await import('@nigel/ui');
      vi.spyOn(ui, 'confirmDialog').mockResolvedValue(true);

      const { el, client } = await mount();
      const field = inputLabelled(el, 'Switch to');
      field.value = '/tmp/other';
      field.dispatchEvent(new Event('input'));
      await el.updateComplete;

      buttonIn(el, 'Data directory')?.click();
      await settle(el);

      expect(client.calls).toContain('setDataDir');
      expect(reloads).toBe(1);
    });

    it('does nothing when the path is blank', async () => {
      const { el, client } = await mount();
      buttonIn(el, 'Data directory')?.click();
      await settle(el);
      expect(client.calls).not.toContain('setDataDir');
    });
  });

  describe('password', () => {
    function submitPassword(
      el: NigelSettingsScreen,
      detail: Record<string, unknown>,
      formIndex = 0,
    ) {
      const forms = el.shadowRoot?.querySelectorAll('wc-password-form');
      forms?.[formIndex]?.dispatchEvent(
        new CustomEvent('nc-password-submit', {
          detail,
          bubbles: true,
          composed: true,
        }),
      );
    }

    it('offers set on a plaintext database', async () => {
      const { el } = await mount();
      const form = el.shadowRoot?.querySelector('wc-password-form');
      expect(form?.getAttribute('mode')).toBe('set');
    });

    it('encrypts and then re-reads the encryption state', async () => {
      const { el, client } = await mount();
      submitPassword(el, { mode: 'set', newPassword: 'hunter2' });
      await settle(el);

      expect(client.calls).toContain('setPassword');
      expect(client.calls.lastIndexOf('getStatus')).toBeGreaterThan(
        client.calls.indexOf('setPassword'),
      );
    });

    it('offers change and remove on an encrypted database', async () => {
      const client = new FakeApiClient();
      client.status = { ...client.status, encrypted: true };
      const { el } = await mount(client);

      const modes = [...(el.shadowRoot?.querySelectorAll('wc-password-form') ?? [])].map(
        (f) => f.getAttribute('mode'),
      );
      expect(modes).toEqual(['change', 'remove']);
    });

    it('confirms before removing the password', async () => {
      const ui = await import('@nigel/ui');
      const confirm = vi.spyOn(ui, 'confirmDialog').mockResolvedValue(false);

      const client = new FakeApiClient();
      client.status = { ...client.status, encrypted: true };
      const { el } = await mount(client);

      submitPassword(el, { mode: 'remove', currentPassword: 'hunter2' }, 1);
      await settle(el);

      expect(confirm).toHaveBeenCalled();
      expect(client.calls).not.toContain('removePassword');
    });

    it('asks for the removal in the destructive voice', async () => {
      const ui = await import('@nigel/ui');
      const confirm = vi.spyOn(ui, 'confirmDialog').mockResolvedValue(true);

      const client = new FakeApiClient();
      client.status = { ...client.status, encrypted: true };
      const { el } = await mount(client);

      submitPassword(el, { mode: 'remove', currentPassword: 'hunter2' }, 1);
      await settle(el);

      expect(confirm).toHaveBeenCalledWith(
        expect.objectContaining({ variant: 'danger', confirmLabel: 'Remove password' }),
      );
      expect(client.calls).toContain('removePassword');
    });

    it('changing needs no confirmation', async () => {
      const ui = await import('@nigel/ui');
      const confirm = vi.spyOn(ui, 'confirmDialog');

      const client = new FakeApiClient();
      client.status = { ...client.status, encrypted: true };
      const { el } = await mount(client);

      submitPassword(el, {
        mode: 'change',
        currentPassword: 'hunter2',
        newPassword: 'hunter3',
      });
      await settle(el);

      expect(confirm).not.toHaveBeenCalled();
      expect(client.calls).toContain('changePassword');
    });

    it('files a failure against the operation that failed', async () => {
      const ui = await import('@nigel/ui');
      vi.spyOn(ui, 'confirmDialog').mockResolvedValue(true);

      const client = new FakeApiClient();
      client.status = { ...client.status, encrypted: true };
      const { el } = await mount(client);
      client.settingsError = new ApiError({
        code: 'invalid_password',
        rawCode: 'invalid_password',
        message: 'Wrong password.',
        status: 401,
        details: { attemptsRemaining: 2, retryAfterMs: 0 },
      });

      submitPassword(el, { mode: 'remove', currentPassword: 'nope' }, 1);
      await settle(el);

      // The change form is first on screen and collects a field by the same
      // name; a remove failure rendered there would be about the wrong one.
      const errors = [...(el.shadowRoot?.querySelectorAll('wc-password-form') ?? [])].map(
        (f) => f.getAttribute('error'),
      );
      expect(errors).toEqual(['', 'Wrong password.']);
    });

    it('surfaces a wrong current password on the form', async () => {
      const client = new FakeApiClient();
      client.status = { ...client.status, encrypted: true };
      const { el } = await mount(client);
      client.settingsError = new ApiError({
        code: 'invalid_password',
        rawCode: 'invalid_password',
        message: 'Wrong password.',
        status: 401,
        details: { attemptsRemaining: 2, retryAfterMs: 0 },
      });

      submitPassword(el, {
        mode: 'change',
        currentPassword: 'nope',
        newPassword: 'new one',
      });
      await settle(el);

      expect(
        el.shadowRoot?.querySelector('wc-password-form')?.getAttribute('error'),
      ).toBe('Wrong password.');
    });
  });

  describe('appearance', () => {
    const switcher = (el: NigelSettingsScreen) =>
      el.shadowRoot?.querySelector('wc-mode-switcher') as HTMLElement & {
        mode: string;
        resolved: string;
      };

    function choose(el: NigelSettingsScreen, mode: string) {
      switcher(el).dispatchEvent(
        new CustomEvent('nc-color-mode-change', {
          detail: { mode },
          bubbles: true,
          composed: true,
        }),
      );
    }

    beforeEach(() => {
      localStorage.clear();
      document.documentElement.classList.remove('light-mode', 'dark-mode');
    });

    it('offers the switcher', async () => {
      const { el } = await mount();
      expect(switcher(el)).toBeTruthy();
    });

    it('shows the stored mode as the current one', async () => {
      localStorage.setItem('nigel.color-mode', 'dark');
      const { el } = await mount();
      expect(switcher(el).mode).toBe('dark');
    });

    it('defaults to system with nothing stored', async () => {
      const { el } = await mount();
      expect(switcher(el).mode).toBe('system');
    });

    it('persists a choice and applies it', async () => {
      const { el } = await mount();
      choose(el, 'dark');
      await el.updateComplete;

      expect(localStorage.getItem('nigel.color-mode')).toBe('dark');
      expect(document.documentElement.classList.contains('dark-mode')).toBe(true);
      expect(switcher(el).mode).toBe('dark');
    });

    it('removes both classes for system, leaving the media query in charge', async () => {
      const { el } = await mount();
      choose(el, 'dark');
      await el.updateComplete;
      choose(el, 'system');
      await el.updateComplete;

      expect(localStorage.getItem('nigel.color-mode')).toBe('system');
      expect(document.documentElement.classList.contains('dark-mode')).toBe(false);
      expect(document.documentElement.classList.contains('light-mode')).toBe(false);
    });

    it('sends nothing to the server — the preference is per-browser', async () => {
      // Every /api/settings/* route is behind the locked guard, so a
      // server-stored mode could not be honoured on the unlock screen.
      const { el, client } = await mount();
      client.calls.length = 0;
      choose(el, 'light');
      await settle(el);

      expect(client.calls).toEqual([]);
    });
  });
});
