import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import './dashboard.js';
import { NigelDashboardScreen as DashboardCtor } from './dashboard.js';
import type { NigelDashboardScreen } from './dashboard.js';
import { appLocked } from '../api/index.js';
import { initializeAppStore, resetAppStore } from '../state/app-store.js';
import { resetDashboardStore } from '../state/dashboard-store.js';
import { FakeApiClient, UNLOCKED_STATUS } from '../__mocks__/fake-api-client.js';
import type { WcBalancesCard, WcCashflowCard, WcStatStrip } from '@nigel/ui';
import type { BalanceReport, CashflowReport, PnlReport } from '../api/types.js';

const PNL: PnlReport = {
  income: [{ name: 'Consulting', total: 184320.5 }],
  expenses: [{ name: 'Rent', total: -24000 }],
  totalIncome: 184320.5,
  totalExpenses: -96540.18,
  net: 87780.32,
};

const BALANCE: BalanceReport = {
  accounts: [
    {
      name: 'BofA Checking',
      accountType: 'checking',
      class: 'asset',
      balance: 4928.01,
      naturalBalance: 4928.01,
    },
    {
      name: 'BofA Credit Card',
      accountType: 'credit_card',
      class: 'liability',
      balance: -318.49,
      naturalBalance: 318.49,
    },
  ],
  total: 4609.52,
  ytdNetIncome: 87780.32,
  uncategorizedTotal: 0,
  uncategorizedCount: 0,
};

const CASHFLOW: CashflowReport = {
  months: Array.from({ length: 12 }, (_, i) => ({
    month: `2026-${String(i + 1).padStart(2, '0')}`,
    inflows: 5000 + i,
    outflows: -3200 - i,
    net: 1800,
    runningBalance: 1800 * (i + 1),
  })),
};

function populated(): FakeApiClient {
  const client = new FakeApiClient();
  client.pnl = PNL;
  client.balance = BALANCE;
  client.cashflow = CASHFLOW;
  return client;
}

async function mount(client = populated()): Promise<{
  el: NigelDashboardScreen;
  client: FakeApiClient;
}> {
  const store = initializeAppStore(client, { reload: () => {} });
  await store.refreshStatus();
  client.calls.length = 0;

  const el = document.createElement('nigel-dashboard-screen');
  el.client = client;
  document.body.appendChild(el);
  await el.updateComplete;
  await new Promise((r) => setTimeout(r, 0));
  await el.updateComplete;
  return { el, client };
}

const shadow = (el: NigelDashboardScreen) => el.shadowRoot!;

function strip(el: NigelDashboardScreen): WcStatStrip | null {
  return shadow(el).querySelector<WcStatStrip>('wc-stat-strip');
}

describe('dashboard screen', () => {
  beforeEach(() => {
    resetAppStore();
    resetDashboardStore();
    appLocked.set(false);
  });

  afterEach(() => {
    document.body.innerHTML = '';
    resetDashboardStore();
    resetAppStore();
  });

  it('fetches all four figures when it mounts', async () => {
    const { client } = await mount();

    expect(client.calls.filter((c) => c.startsWith('getPnl')).length).toBe(1);
    expect(client.calls.filter((c) => c === 'getBalance').length).toBe(1);
    expect(client.calls.filter((c) => c.startsWith('getCashflow')).length).toBe(1);
    expect(client.calls.filter((c) => c === 'getFlagged').length).toBe(1);
  });

  it('shows year-to-date income, expenses and net', async () => {
    const { el } = await mount();
    const cells = strip(el)?.cells ?? [];
    expect(cells.map((c) => c.amount)).toEqual([184320.5, -96540.18, 87780.32]);
    expect(cells.map((c) => c.label)).toEqual([
      'Income, year to date',
      'Expenses, year to date',
      'Net',
    ]);
    expect(cells[2]?.hint).toBe('Cash basis');
  });

  it('lists the account balances with their total', async () => {
    const { el } = await mount();
    const list = shadow(el).querySelector<WcBalancesCard>('wc-balances-card');
    expect(list?.items.length).toBe(2);
    expect(list?.total).toBe(4609.52);
  });

  it('charts twelve months of cash flow', async () => {
    const { el } = await mount();
    const chart = shadow(el).querySelector<WcCashflowCard>('wc-cashflow-card');
    expect(chart?.buckets.length).toBe(12);
    expect(chart?.buckets[0]).toEqual({ label: 'Jan', income: 5000, expense: 3200 });
  });

  it('shows the update notice when the server reports a newer release', async () => {
    const client = populated();
    client.status = { ...UNLOCKED_STATUS, updateAvailable: '1.0.2' };
    const { el } = await mount(client);

    const notice = shadow(el).querySelector('wc-notice-bar');
    expect(notice).not.toBeNull();
    expect(notice?.getAttribute('message')).toContain('1.0.2');
  });

  it('shows no update notice when there is nothing newer', async () => {
    const { el } = await mount();
    expect(shadow(el).querySelector('wc-notice-bar')).toBeNull();
  });

  it('lets the update notice be dismissed for the session', async () => {
    const client = populated();
    client.status = { ...UNLOCKED_STATUS, updateAvailable: '1.0.2' };
    const { el } = await mount(client);

    shadow(el)
      .querySelector('wc-notice-bar')
      ?.dispatchEvent(new CustomEvent('nc-notice-dismiss', { bubbles: true }));
    await el.updateComplete;

    expect(shadow(el).querySelector('wc-notice-bar')).toBeNull();
  });

  it('points an empty database at the import screen', async () => {
    const client = new FakeApiClient();
    const { el } = await mount(client);

    const empty = shadow(el).querySelector('wc-empty-state');
    expect(empty).not.toBeNull();
    expect(strip(el)).toBeNull();
    expect(empty?.querySelector('a')?.getAttribute('href')).toBe('#/import');
  });

  it('surfaces a failed figure on its own card and leaves the rest', async () => {
    const client = populated();
    client.balanceError = new Error('balance is down');
    const { el } = await mount(client);

    expect(
      shadow(el).querySelector('wc-balances-card')?.getAttribute('error'),
    ).toContain('balance is down');
    // The other three still rendered.
    expect(strip(el)?.cells.length).toBe(3);
    expect(
      shadow(el).querySelector<WcCashflowCard>('wc-cashflow-card')?.buckets.length,
    ).toBe(12);
  });

  it('retries only the endpoint whose card asked', async () => {
    const client = populated();
    client.balanceError = new Error('down');
    const { el } = await mount(client);
    client.calls.length = 0;

    shadow(el)
      .querySelector('wc-balances-card')
      ?.dispatchEvent(new CustomEvent('nc-retry', { bubbles: true }));
    await new Promise((r) => setTimeout(r, 0));

    expect(client.calls).toEqual(['getBalance']);
  });
});

describe('nigel-dashboard-screen on a phone', () => {
  // jsdom has no layout engine, so the rules are read rather than measured.
  const text = [DashboardCtor.styles].flat().map(String).join('\n');

  it('never lets a card decide how wide the page is', () => {
    // A grid track's automatic minimum is its content, so one child that
    // cannot shrink — a thirteen-month chart, a long account name — widens the
    // whole page and the phone scrolls sideways. minmax(0, …) caps that at the
    // track, leaving the child to scroll inside its own panel.
    expect(text).toMatch(/@media \(max-width: 60rem\)[\s\S]*\.panels\s*{[^}]*grid-template-columns:\s*minmax\(0/);
  });
});
