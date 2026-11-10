import { describe, it, expect, afterEach } from 'vitest';
import './wc-sync-settings.js';
import type {
  NcSyncMapDetail,
  NcSyncRunDetail,
  NcSyncSetupDetail,
  SyncLinkedAccount,
  SyncOutcome,
  WcSyncSettings,
} from './wc-sync-settings.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-sync-settings.preview.js';

const ACCOUNTS: SyncLinkedAccount[] = [
  {
    key: 'harborvale.example/chk1',
    orgName: 'Harbor & Vale Bank',
    name: 'Checking',
    currency: 'USD',
    account: 'Operating',
    lastSynced: '2026-11-08',
  },
  {
    key: 'harborvale.example/sav1',
    orgName: 'Harbor & Vale Bank',
    name: 'Savings',
    currency: 'USD',
    account: null,
    lastSynced: null,
  },
];

async function mount(props: Partial<WcSyncSettings> = {}): Promise<WcSyncSettings> {
  const el = document.createElement('wc-sync-settings');
  Object.assign(el, { targets: ['Operating', 'Savings'] }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function events<T>(el: HTMLElement, name: string): T[] {
  const seen: T[] = [];
  el.addEventListener(name, (event) => seen.push((event as CustomEvent<T>).detail));
  return seen;
}

const q = <T extends Element = HTMLElement>(el: WcSyncSettings, selector: string) =>
  el.shadowRoot?.querySelector(selector) as T | null;

describe('wc-sync-settings', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('asks for a setup token in a password field when not connected', async () => {
    const el = await mount();
    const field = q<HTMLElement & { type: string }>(el, '[data-token]');
    expect(field?.getAttribute('type')).toBe('password');
    expect(q(el, '.title')?.textContent).toBe('Not connected');
    expect(q(el, '[data-sync]')).toBeNull();
    expect(q(el, '.accounts')).toBeNull();
  });

  it('emits the trimmed token on connect, and nothing for an empty field', async () => {
    const el = await mount();
    const seen = events<NcSyncSetupDetail>(el, 'nc-sync-setup');
    const form = q(el, 'form.token');
    form?.dispatchEvent(new Event('submit', { cancelable: true }));
    expect(seen).toEqual([]);

    const field = q<HTMLElement & { value: string }>(el, '[data-token]');
    if (field) field.value = '  dG9rZW4=  ';
    form?.dispatchEvent(new Event('submit', { cancelable: true }));
    expect(seen).toEqual([{ token: 'dG9rZW4=' }]);
  });

  it('empties the token field once connected', async () => {
    const el = await mount();
    const field = q<HTMLElement & { value: string }>(el, '[data-token]');
    if (field) field.value = 'secret-token';
    el.configured = true;
    el.accounts = ACCOUNTS;
    await el.updateComplete;
    expect(field?.value).toBe('');
    expect(q(el, '[data-token]')).toBeNull();
  });

  it('closes the token form and empties it once a replacement token connects', async () => {
    const el = await mount({ configured: true, accounts: ACCOUNTS });
    q(el, '[aria-expanded]')?.click();
    await el.updateComplete;
    const field = q<HTMLElement & { value: string }>(el, '[data-token]');
    expect(field).not.toBeNull();
    if (field) field.value = 'spent-token';
    el.connections = 1;
    await el.updateComplete;
    expect(field?.value).toBe('');
    expect(q(el, '[data-token]')).toBeNull();
  });

  it('lists each bank account with what it syncs into', async () => {
    const el = await mount({ configured: true, accounts: ACCOUNTS });
    const selects = [...(el.shadowRoot?.querySelectorAll('wa-select') ?? [])];
    expect(selects.map((s) => s.getAttribute('value'))).toEqual(['Operating', '']);
    expect(q(el, '.sub')?.textContent).toContain('2 bank accounts, 1 syncing');
    expect(el.shadowRoot?.textContent).toContain('last synced Nov 8, 2026');
  });

  it('maps an account, or unmaps it when Not synced is chosen', async () => {
    const el = await mount({ configured: true, accounts: ACCOUNTS });
    const seen = events<NcSyncMapDetail>(el, 'nc-sync-map');
    const [first, second] = [...(el.shadowRoot?.querySelectorAll('wa-select') ?? [])] as (
      | (HTMLElement & { value: string })
      | undefined
    )[];
    if (second) second.value = 'Savings';
    second?.dispatchEvent(new Event('change'));
    if (first) first.value = '';
    first?.dispatchEvent(new Event('change'));
    expect(seen).toEqual([
      { key: 'harborvale.example/sav1', account: 'Savings' },
      { key: 'harborvale.example/chk1', account: null },
    ]);
  });

  it('previews and syncs on request', async () => {
    const el = await mount({ configured: true, accounts: ACCOUNTS });
    const runs = events<NcSyncRunDetail>(el, 'nc-sync-run');
    const refreshes = events<unknown>(el, 'nc-sync-refresh');
    q(el, '[data-preview]')?.click();
    q(el, '[data-sync]')?.click();
    q(el, '[data-refresh]')?.click();
    expect(runs).toEqual([{ dryRun: true }, { dryRun: false }]);
    expect(refreshes).toHaveLength(1);
  });

  it('cannot sync with nothing linked', async () => {
    const el = await mount({
      configured: true,
      accounts: ACCOUNTS.map((a) => ({ ...a, account: null })),
    });
    expect(q(el, '[data-sync]')?.hasAttribute('disabled')).toBe(true);
    expect(q(el, '[data-preview]')?.hasAttribute('disabled')).toBe(true);
    expect(q(el, '[data-refresh]')?.hasAttribute('disabled')).toBe(false);
  });

  it('holds every control while something is in flight', async () => {
    const el = await mount({ configured: true, accounts: ACCOUNTS, busy: 'sync' });
    const runs = events<NcSyncRunDetail>(el, 'nc-sync-run');
    q(el, '[data-sync]')?.click();
    expect(runs).toEqual([]);
    expect(q(el, '[data-sync]')?.hasAttribute('loading')).toBe(true);
    for (const select of el.shadowRoot?.querySelectorAll('wa-select') ?? []) {
      expect(select.hasAttribute('disabled')).toBe(true);
    }
  });

  it('reports a sync account by account', async () => {
    const outcome: SyncOutcome = {
      dryRun: false,
      accounts: [
        {
          key: 'harborvale.example/chk1',
          label: 'Harbor & Vale Bank — Checking',
          account: 'Operating',
          imported: 3,
          skipped: 2,
          malformed: 1,
          balance: { balance: 1250, asOf: '2026-11-08' },
        },
      ],
      unmapped: ['Harbor & Vale Bank — Savings'],
      warnings: ['Please sign in again.'],
      categorized: 2,
      stillFlagged: 1,
    };
    const el = await mount({ configured: true, accounts: ACCOUNTS, outcome });
    const text = q(el, '[data-outcome]')?.textContent?.replace(/\s+/g, ' ') ?? '';
    expect(text).toContain('Brought in 3 transactions');
    expect(text).toContain('3 new, 2 already here, 1 unreadable');
    expect(text).toContain('as of Nov 8, 2026');
    expect(text).toContain('Skipped, not linked yet: Harbor & Vale Bank — Savings');
    expect(text).toContain('Your bridge says: Please sign in again.');
    expect(text).toContain('1 transaction still needs a look');
    expect(q(el, '[data-outcome] wc-money')).toBeTruthy();
  });

  it('says a preview changed nothing', async () => {
    const el = await mount({
      configured: true,
      accounts: ACCOUNTS,
      outcome: {
        dryRun: true,
        accounts: [],
        unmapped: [],
        warnings: [],
        categorized: null,
        stillFlagged: null,
      },
    });
    const text = q(el, '[data-outcome]')?.textContent ?? '';
    expect(text).toContain('Nothing new to bring in');
    expect(text).toContain('Nothing has changed yet');
  });

  it('shows an error as an alert', async () => {
    const el = await mount({ error: 'SimpleFIN refused the setup token.' });
    expect(q(el, '[role="alert"]')?.textContent).toBe('SimpleFIN refused the setup token.');
  });
});

describePreviewA11y(preview);
