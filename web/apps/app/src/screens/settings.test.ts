import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import './settings.js';
import type { NigelSettingsScreen } from './settings.js';
import { ApiError, appLocked } from '../api/index.js';
import { getAppStore, initializeAppStore, resetAppStore } from '../state/app-store.js';
import { FakeApiClient } from '../__mocks__/fake-api-client.js';
import { setUnsavedChanges, unsavedChanges } from '../state/unsaved.js';

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

/** A section, by the id the index scrolls to. */
const section = (el: NigelSettingsScreen, id: string) =>
  el.shadowRoot?.getElementById(id) as HTMLElement | null;

const saveBar = (el: NigelSettingsScreen) => el.shadowRoot?.querySelector('wc-save-bar');

function barButton(el: NigelSettingsScreen, label: string): HTMLElement {
  const bar = saveBar(el);
  if (!bar) throw new Error('no save bar');
  const found = [...(bar.shadowRoot?.querySelectorAll('wa-button') ?? [])].find((b) =>
    b.textContent?.includes(label),
  );
  if (!found) throw new Error(`no ${label} button`);
  return found as HTMLElement;
}

async function settle(el: NigelSettingsScreen): Promise<void> {
  await new Promise((r) => setTimeout(r, 0));
  await el.updateComplete;
}

async function type(el: NigelSettingsScreen, field: HTMLElement & { value: string }, value: string) {
  field.value = value;
  field.dispatchEvent(new Event('input'));
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
    vi.useRealTimers();
    setUnsavedChanges(null);
  });

  it('loads the application settings when it mounts', async () => {
    const { client } = await mount();
    expect(client.calls).toContain('getAppSettings');
  });

  it('groups the sections under This book and This device', async () => {
    const { el } = await mount();
    const index = el.shadowRoot?.querySelector('wc-settings-index');
    expect(index?.groups.map((g) => g.label)).toEqual(['This book', 'This device']);
    expect(index?.groups.flatMap((g) => g.items.map((i) => i.id))).toEqual([
      'letterhead',
      'security',
      'location',
      'appearance',
      'updates',
    ]);
    for (const id of ['letterhead', 'security', 'location', 'appearance', 'updates']) {
      expect(section(el, id), id).toBeTruthy();
    }
    expect(index?.active).toBe('letterhead');
  });

  it('scrolls to a section picked from the index and marks it', async () => {
    const { el } = await mount();
    const target = section(el, 'appearance');
    const scroll = vi.fn();
    if (target) target.scrollIntoView = scroll;

    el.shadowRoot
      ?.querySelector('wc-settings-index')
      ?.dispatchEvent(
        new CustomEvent('nc-settings-section', { detail: { id: 'appearance' }, bubbles: true }),
      );
    await el.updateComplete;

    expect(scroll).toHaveBeenCalledWith(expect.objectContaining({ block: 'start' }));
    expect(el.shadowRoot?.querySelector('wc-settings-index')?.active).toBe('appearance');
  });

  it('names the open book in the This book header', async () => {
    const { el } = await mount();
    const scope = el.shadowRoot?.querySelector('wc-settings-scope[kind="book"]');
    expect(scope?.getAttribute('heading')).toBe('Test Consultancy');
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

    const logoField = (el: NigelSettingsScreen) =>
      el.shadowRoot?.querySelector('wc-logo-field') as HTMLElement & { logo: string };

    function chooseLogo(el: NigelSettingsScreen, file: File): void {
      logoField(el).dispatchEvent(
        new CustomEvent('nc-logo-file', { detail: { file }, bubbles: true, composed: true }),
      );
    }

    it('tells the user the logo limits in plain words', async () => {
      const { el } = await mount();
      const field = el.shadowRoot?.querySelector('wc-logo-field') as HTMLElement & {
        help: string;
        prompt: string;
      };
      expect(field.prompt).toContain('PNG or JPEG');
      expect(field.help).toBe('Up to 128 KB. Shown at the top of your invoices.');
    });

    it('seeds all five fields once and does not clobber typing', async () => {
      const client = new FakeApiClient();
      client.company = {
        name: 'Juniper Labs',
        address: '1 Juniper Way',
        phone: '555-0100',
        logo: '',
        paymentInstructions: 'ACH to Juniper Labs',
      };
      const { el } = await mount(client);

      expect(client.calls).toContain('getCompany');
      expect(input(el, 0).value).toBe('Juniper Labs');
      expect(input(el, 1).value).toBe('555-0100');
      expect(textarea(el, 0).value).toBe('1 Juniper Way');
      expect(textarea(el, 1).value).toBe('ACH to Juniper Labs');

      // A store refresh must not re-seed the form under someone typing.
      await type(el, input(el, 0), 'Juniper Studio');
      await getAppStore().refreshStatus();
      await el.updateComplete;
      expect(input(el, 0).value).toBe('Juniper Studio');
    });

    it('shows the save bar only while something differs from what is stored', async () => {
      const { el } = await mount();
      expect(saveBar(el)).toBeFalsy();

      await type(el, input(el, 1), '555-0199');
      expect(saveBar(el)?.getAttribute('message')).toBe('Unsaved changes to the letterhead');

      // Typed back to the stored value: nothing is unsaved any more.
      await type(el, input(el, 1), '');
      expect(saveBar(el)).toBeFalsy();
    });

    it('discards back to the stored values', async () => {
      const { el, client } = await mount();
      await type(el, input(el, 0), 'Cedar Systems');
      await type(el, textarea(el, 0), '9 Cedar Court');

      barButton(el, 'Discard').click();
      await settle(el);

      expect(input(el, 0).value).toBe('Test Consultancy');
      expect(textarea(el, 0).value).toBe('');
      expect(saveBar(el)).toBeFalsy();
      expect(client.calls).not.toContain('setCompany');
    });

    it('saves the whole letterhead in one request and refreshes the name everywhere', async () => {
      const { el, client } = await mount();
      await type(el, input(el, 0), 'Cedar Systems');
      await type(el, textarea(el, 0), '9 Cedar Court');
      await type(el, textarea(el, 1), 'ACH to Cedar Systems');

      barButton(el, 'Save').click();
      await settle(el);

      expect(client.calls.filter((c) => c === 'setCompany')).toHaveLength(1);
      expect(client.company.name).toBe('Cedar Systems');
      expect(client.company.address).toBe('9 Cedar Court');
      expect(client.company.paymentInstructions).toBe('ACH to Cedar Systems');
      // The breadcrumb, the status line and the window title read the name
      // from status, so a save that skipped the refresh would leave them stale.
      expect(client.calls.lastIndexOf('getStatus')).toBeGreaterThan(
        client.calls.indexOf('setCompany'),
      );
      expect(getAppStore().companyName.get()).toBe('Cedar Systems');
      expect(
        el.shadowRoot?.querySelector('wc-settings-scope[kind="book"]')?.getAttribute('heading'),
      ).toBe('Cedar Systems');
      expect(saveBar(el)).toBeFalsy();
    });

    it('says Saved beside the title for a moment after a save', async () => {
      const { el } = await mount();
      await type(el, input(el, 1), '555-0142');
      vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });

      barButton(el, 'Save').click();
      await vi.waitFor(() => expect(section(el, 'letterhead')?.hasAttribute('saved')).toBe(true));

      vi.advanceTimersByTime(2300);
      await el.updateComplete;
      expect(section(el, 'letterhead')?.hasAttribute('saved')).toBe(false);
    });

    it('tells the app about the draft while it differs, so leaving asks first', async () => {
      const { el } = await mount();
      expect(unsavedChanges()).toBeNull();

      await type(el, input(el, 1), '555-0199');
      expect(unsavedChanges()?.heading).toBe('Discard unsaved letterhead changes?');

      barButton(el, 'Discard').click();
      await settle(el);
      expect(unsavedChanges()).toBeNull();
    });

    it('drops its claim when it goes away', async () => {
      const { el } = await mount();
      await type(el, input(el, 1), '555-0199');
      el.remove();
      expect(unsavedChanges()).toBeNull();
    });

    it('has the browser ask before the window closes on a draft', async () => {
      const { el } = await mount();
      const clean = new Event('beforeunload', { cancelable: true });
      window.dispatchEvent(clean);
      expect(clean.defaultPrevented).toBe(false);

      await type(el, input(el, 1), '555-0199');
      const dirty = new Event('beforeunload', { cancelable: true });
      window.dispatchEvent(dirty);
      expect(dirty.defaultPrevented).toBe(true);
    });

    it('puts focus on the first field when the save bar goes', async () => {
      const { el } = await mount();
      await type(el, input(el, 1), '555-0199');
      const focus = vi.spyOn(input(el, 0), 'focus');
      barButton(el, 'Discard').click();
      await settle(el);
      expect(focus).toHaveBeenCalled();

      await type(el, input(el, 1), '555-0199');
      const again = vi.spyOn(input(el, 0), 'focus');
      barButton(el, 'Save').click();
      await settle(el);
      await settle(el);
      expect(again).toHaveBeenCalled();
    });

    it('surfaces the server refusal for a bad logo without clearing the form', async () => {
      const client = new FakeApiClient();
      const { el } = await mount(client);
      await type(el, input(el, 0), 'Cedar Systems');

      client.companyError = new ApiError({
        code: 'bad_request',
        rawCode: 'bad_request',
        message: 'A logo of type image/svg+xml cannot be used.',
        status: 400,
      });
      barButton(el, 'Save').click();
      await settle(el);

      expect(el.shadowRoot?.textContent).toContain('image/svg+xml');
      expect(input(el, 0).value).toBe('Cedar Systems');
      expect(client.company.name).toBe('Test Consultancy');
      // Still unsaved, so the way to try again is still there.
      expect(saveBar(el)).toBeTruthy();
    });

    it('reads a chosen file as a data URI before sending', async () => {
      const { el, client } = await mount();
      chooseLogo(el, new File([new Uint8Array([1, 2, 3])], 'logo.png', { type: 'image/png' }));
      // FileReader resolves on its own task; the save must see the result.
      await vi.waitFor(() => {
        expect(logoField(el).logo).toMatch(/^data:/);
      });
      await settle(el);

      barButton(el, 'Save').click();
      await settle(el);

      expect(client.company.logo).toMatch(/^data:/);
    });

    it('clears the logo as an unsaved change', async () => {
      const client = new FakeApiClient();
      client.company = { ...client.company, logo: 'data:image/png;base64,AAAA' };
      const { el } = await mount(client);

      logoField(el).dispatchEvent(new CustomEvent('nc-logo-clear', { bubbles: true }));
      await el.updateComplete;

      expect(logoField(el).logo).toBe('');
      expect(saveBar(el)).toBeTruthy();
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
      const letterhead = section(el, 'letterhead');
      expect(letterhead?.querySelector('wc-spinner')).toBeFalsy();
      expect(letterhead?.querySelector('wa-input')).toBeFalsy();
      expect(letterhead?.textContent).toContain('the database is unreadable');
      const retry = letterhead?.querySelector('wa-button') as HTMLElement;
      expect(retry.textContent).toContain('Retry');

      client.companyLoadError = null;
      retry.click();
      await settle(el);

      expect(client.calls.filter((c) => c === 'getCompany')).toHaveLength(2);
      expect(input(el, 0).value).toBe('Test Consultancy');
    });

    it('refuses an oversized logo before it reaches the server', async () => {
      const { el, client } = await mount();
      chooseLogo(
        el,
        new File([new Uint8Array(200 * 1024)], 'huge.png', { type: 'image/png' }),
      );
      await settle(el);

      // Named, so it is a refusal about the file that was chosen and not the
      // standing note about the cap.
      expect(el.shadowRoot?.textContent).toContain('huge.png');
      expect(logoField(el).logo).toBe('');
      expect(saveBar(el)).toBeFalsy();
      expect(client.calls).not.toContain('setCompany');
    });

    it('reports a failed save without changing what is shown', async () => {
      const client = new FakeApiClient();
      const { el } = await mount(client);
      await type(el, input(el, 0), 'Globex');
      client.companyError = new ApiError({
        code: 'internal',
        rawCode: 'internal',
        message: 'disk is full',
        status: 500,
      });

      barButton(el, 'Save').click();
      await settle(el);

      expect(client.status.companyName).toBe('Test Consultancy');
      expect(section(el, 'letterhead')?.textContent).toContain('disk is full');
    });
  });

  describe('update check', () => {
    const row = (el: NigelSettingsScreen) =>
      el.shadowRoot?.querySelector('#updates wc-switch-row') as HTMLElement & {
        checked: boolean;
      };

    function flip(el: NigelSettingsScreen, checked: boolean): void {
      row(el).dispatchEvent(
        new CustomEvent('nc-switch-change', { detail: { checked }, bubbles: true }),
      );
    }

    it('writes the new value', async () => {
      const { el, client } = await mount();
      flip(el, false);
      await settle(el);

      expect(client.calls).toContain('updateAppSettings');
      expect(client.appSettings.updateCheck).toBe(false);
      expect(row(el).checked).toBe(false);
    });

    it('keeps the switch where the server has it when the write fails', async () => {
      const client = new FakeApiClient();
      const { el } = await mount(client);
      expect(client.appSettings.updateCheck).toBe(true);
      client.settingsError = new ApiError({
        code: 'internal',
        rawCode: 'internal',
        message: 'nope',
        status: 500,
      });

      flip(el, false);
      await settle(el);

      // A control whose write failed must not keep claiming it succeeded.
      expect(row(el).checked).toBe(true);
      expect(client.appSettings.updateCheck).toBe(true);
    });

    it('says which version is running', async () => {
      const { el } = await mount();
      expect(section(el, 'updates')?.textContent).toContain("You're on v0.0.0-test.");
    });

    it('is not offered in a store build', async () => {
      const client = new FakeApiClient();
      client.status = { ...client.status, distribution: 'store' };
      const { el } = await mount(client);

      expect(section(el, 'updates')).toBeNull();
      const ids = el.shadowRoot
        ?.querySelector('wc-settings-index')
        ?.groups.flatMap((g) => g.items.map((i) => i.id));
      expect(ids).not.toContain('updates');
    });
  });

  describe('location', () => {
    const switchButton = (el: NigelSettingsScreen) =>
      el.shadowRoot?.querySelector('wa-button[data-switch]') as HTMLElement;

    it('shows the active data directory', async () => {
      const { el } = await mount();
      expect(el.shadowRoot?.querySelector('wc-folder-path')?.getAttribute('path')).toBe(
        '/tmp/nigel',
      );
    });

    it('offers no reveal where the client cannot open a folder', async () => {
      const { el } = await mount();
      expect(section(el, 'location')?.querySelector('wc-folder-path wa-button')).toBeNull();
    });

    it("reveals the folder through the client's file manager", async () => {
      const client = new FakeApiClient();
      const run = vi.fn(async () => {});
      client.dataDirRevealValue = { label: 'Reveal in Finder', run };
      const { el } = await mount(client);

      const reveal = section(el, 'location')?.querySelector(
        'wc-folder-path wa-button',
      ) as HTMLElement;
      expect(reveal.textContent).toBe('Reveal in Finder');
      reveal.click();
      await settle(el);
      expect(run).toHaveBeenCalledTimes(1);
    });

    it('asks before switching, and does nothing when refused', async () => {
      const ui = await import('@nigel/ui');
      vi.spyOn(ui, 'confirmDialog').mockResolvedValue(false);

      const { el, client } = await mount();
      await type(el, inputLabelled(el, 'Switch to'), '/tmp/other');

      switchButton(el).click();
      await settle(el);

      expect(client.calls).not.toContain('setDataDir');
    });

    it('switches once confirmed', async () => {
      const ui = await import('@nigel/ui');
      vi.spyOn(ui, 'confirmDialog').mockResolvedValue(true);

      const { el, client } = await mount();
      await type(el, inputLabelled(el, 'Switch to'), '/tmp/other');

      switchButton(el).click();
      await settle(el);

      expect(client.calls).toContain('setDataDir');
      expect(reloads).toBe(1);
    });

    it('does nothing when the path is blank', async () => {
      const { el, client } = await mount();
      switchButton(el).click();
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

    const card = (el: NigelSettingsScreen) =>
      el.shadowRoot?.querySelector('wc-security-card') as HTMLElement & {
        panel: string | null;
        encrypted: boolean;
      };

    it('offers set on a plaintext database', async () => {
      const { el } = await mount();
      const form = el.shadowRoot?.querySelector('wc-password-form');
      expect(form?.getAttribute('mode')).toBe('set');
      expect(form?.getAttribute('slot')).toBe('set');
      expect(card(el).encrypted).toBe(false);
    });

    it('encrypts and then re-reads the encryption state', async () => {
      const { el, client } = await mount();
      submitPassword(el, { mode: 'set', newPassword: 'hunter2' });
      await settle(el);

      expect(client.calls).toContain('setPassword');
      expect(client.calls.lastIndexOf('getStatus')).toBeGreaterThan(
        client.calls.indexOf('setPassword'),
      );
      expect(card(el).encrypted).toBe(true);
    });

    it('offers change and remove on an encrypted database', async () => {
      const client = new FakeApiClient();
      client.status = { ...client.status, encrypted: true };
      const { el } = await mount(client);

      const forms = [...(el.shadowRoot?.querySelectorAll('wc-password-form') ?? [])];
      expect(forms.map((f) => f.getAttribute('mode'))).toEqual(['change', 'remove']);
      expect(forms.map((f) => f.getAttribute('slot'))).toEqual(['change', 'remove']);
    });

    it('closes the open panel once its operation succeeds', async () => {
      const client = new FakeApiClient();
      client.status = { ...client.status, encrypted: true };
      const { el } = await mount(client);

      card(el).dispatchEvent(
        new CustomEvent('nc-security-panel', { detail: { panel: 'change' }, bubbles: true }),
      );
      await el.updateComplete;
      expect(card(el).panel).toBe('change');

      submitPassword(el, { mode: 'change', currentPassword: 'a', newPassword: 'b' });
      await settle(el);
      expect(card(el).panel).toBeNull();
    });

    it('clears a failure when its panel closes', async () => {
      const client = new FakeApiClient();
      client.status = { ...client.status, encrypted: true };
      const { el } = await mount(client);
      client.settingsError = new Error('socket hang up');
      submitPassword(el, { mode: 'change', currentPassword: 'a', newPassword: 'b' });
      await settle(el);
      expect(el.shadowRoot?.querySelector('wc-password-form')?.getAttribute('error')).toBe(
        'Could not change the password.',
      );

      card(el).dispatchEvent(
        new CustomEvent('nc-security-panel', { detail: { panel: null }, bubbles: true }),
      );
      await el.updateComplete;
      const errors = [...(el.shadowRoot?.querySelectorAll('wc-password-form') ?? [])].map((f) =>
        f.getAttribute('error'),
      );
      expect(errors).toEqual(['', '']);
    });

    it('moves focus to the new Change… button once the database is encrypted', async () => {
      const { el } = await mount();
      const focus = vi.spyOn(card(el) as unknown as { focusFirstAction: () => void }, 'focusFirstAction');
      submitPassword(el, { mode: 'set', newPassword: 'hunter2' });
      await settle(el);
      await settle(el);
      expect(card(el).encrypted).toBe(true);
      expect(focus).toHaveBeenCalled();
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

      // The change form collects a field by the same name; a remove failure
      // rendered there would be about the wrong one.
      const errors = [...(el.shadowRoot?.querySelectorAll('wc-password-form') ?? [])].map(
        (f) => f.getAttribute('error'),
      );
      expect(errors).toEqual(['', 'Wrong password.']);
    });

    it('words a non-api failure for the operation that failed', async () => {
      const ui = await import('@nigel/ui');
      vi.spyOn(ui, 'confirmDialog').mockResolvedValue(true);

      const client = new FakeApiClient();
      client.status = { ...client.status, encrypted: true };
      const { el } = await mount(client);
      // Not an ApiError, so the screen supplies the sentence itself.
      client.settingsError = new Error('socket hang up');

      submitPassword(el, { mode: 'remove', currentPassword: 'hunter2' }, 1);
      await settle(el);

      const remove = el.shadowRoot?.querySelectorAll('wc-password-form')[1];
      expect(remove?.getAttribute('error')).toBe('Could not remove the password.');
    });

    it('keeps a failure visible when the operation it names leaves the screen', async () => {
      const ui = await import('@nigel/ui');
      vi.spyOn(ui, 'confirmDialog').mockResolvedValue(true);

      const client = new FakeApiClient();
      client.status = { ...client.status, encrypted: true };
      const { el } = await mount(client);
      client.settingsError = new Error('socket hang up');

      submitPassword(el, { mode: 'remove', currentPassword: 'hunter2' }, 1);
      await settle(el);

      // Another session decrypts these books; the next refreshStatus swaps the
      // two forms for the single Set form the remove failure cannot name.
      client.settingsError = null;
      client.status = { ...client.status, encrypted: false };
      await getAppStore().refreshStatus();
      await settle(el);

      const forms = [...(el.shadowRoot?.querySelectorAll('wc-password-form') ?? [])];
      expect(forms.map((f) => f.getAttribute('mode'))).toEqual(['set']);
      // Stale, but readable — vanishing would leave the failure unreported.
      expect(forms[0]?.getAttribute('error')).toBe('Could not remove the password.');
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
    const picker = (el: NigelSettingsScreen) =>
      el.shadowRoot?.querySelector('wc-appearance-picker') as HTMLElement & { mode: string };

    function choose(el: NigelSettingsScreen, mode: string) {
      picker(el).dispatchEvent(
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

    it('offers the picker', async () => {
      const { el } = await mount();
      expect(picker(el)).toBeTruthy();
    });

    it('shows the stored mode as the current one', async () => {
      localStorage.setItem('nigel.color-mode', 'dark');
      const { el } = await mount();
      expect(picker(el).mode).toBe('dark');
    });

    it('defaults to system with nothing stored', async () => {
      const { el } = await mount();
      expect(picker(el).mode).toBe('system');
    });

    it('persists a choice and applies it at once', async () => {
      const { el } = await mount();
      choose(el, 'dark');
      await el.updateComplete;

      expect(localStorage.getItem('nigel.color-mode')).toBe('dark');
      expect(document.documentElement.classList.contains('dark-mode')).toBe(true);
      expect(picker(el).mode).toBe('dark');
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

    it('still applies the choice when storage refuses it', async () => {
      const { el } = await mount();
      vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
        throw new Error('SecurityError');
      });
      choose(el, 'light');
      await el.updateComplete;

      expect(document.documentElement.classList.contains('light-mode')).toBe(true);
      expect(picker(el).mode).toBe('light');
    });

    it('sends nothing to the server — the preference is per-device', async () => {
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
