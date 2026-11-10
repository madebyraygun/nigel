import { html } from 'lit';
import './wc-sync-settings.js';
import type { Preview } from '../../preview/types.js';
import type {
  SyncBusy,
  SyncLinkedAccount,
  SyncOutcome,
} from './wc-sync-settings.js';

const width = 'width:min(720px, calc(100vw - 32px));';

const TARGETS = ['Operating', 'Savings', 'Company Card'];

const ACCOUNTS: SyncLinkedAccount[] = [
  {
    key: 'harborvale.example/chk1',
    orgName: 'Harbor & Vale Bank',
    name: 'Checking',
    currency: 'USD',
    account: 'Operating',
    lastSynced: '2026-11-08',
    suppliesBalance: true,
  },
  {
    key: 'harborvale.example/sav1',
    orgName: 'Harbor & Vale Bank',
    name: 'Savings',
    currency: 'USD',
    account: 'Savings',
    lastSynced: null,
    suppliesBalance: true,
  },
  {
    key: 'cedar.example/card9',
    orgName: 'Cedar Systems Credit Union',
    name: 'Rewards Card',
    currency: 'USD',
    account: null,
    lastSynced: null,
    suppliesBalance: false,
  },
];

/** A card program: the parent carries payments and the balance, each cardholder their charges. */
const CARD_PROGRAM: SyncLinkedAccount[] = [
  {
    key: 'cedar.example/program',
    orgName: 'Cedar Systems Credit Union',
    name: 'Business Card',
    currency: 'USD',
    account: 'Company Card',
    lastSynced: '2026-11-08',
    suppliesBalance: true,
  },
  {
    key: 'cedar.example/holder1',
    orgName: 'Cedar Systems Credit Union',
    name: 'Cardholder 1',
    currency: 'USD',
    account: 'Company Card',
    lastSynced: '2026-11-08',
    suppliesBalance: false,
  },
];

const outcome = (dryRun: boolean): SyncOutcome => ({
  dryRun,
  accounts: [
    {
      key: 'harborvale.example/chk1',
      label: 'Harbor & Vale Bank — Checking',
      account: 'Operating',
      imported: 4,
      skipped: 2,
      malformed: 1,
      balance: { balance: 4820.15, asOf: '2026-11-08' },
    },
    {
      key: 'harborvale.example/sav1',
      label: 'Harbor & Vale Bank — Savings',
      account: 'Savings',
      imported: 0,
      skipped: 0,
      malformed: 0,
      balance: { balance: 12000, asOf: '2026-11-08' },
    },
  ],
  unmapped: ['Cedar Systems Credit Union — Rewards Card'],
  warnings: ['Cedar Systems Credit Union needs you to sign in again at the bridge.'],
  categorized: dryRun ? null : 3,
  stillFlagged: dryRun ? null : 1,
});

const sync = (
  options: {
    configured?: boolean;
    accounts?: SyncLinkedAccount[];
    busy?: SyncBusy;
    outcome?: SyncOutcome | null;
    error?: string;
    notices?: string[];
  } = {},
) => html`<div style=${width}>
  <wc-sync-settings
    ?configured=${options.configured ?? true}
    .accounts=${options.accounts ?? ACCOUNTS}
    .targets=${TARGETS}
    .busy=${options.busy ?? null}
    .outcome=${options.outcome ?? null}
    .notices=${options.notices ?? []}
    error=${options.error ?? ''}
  ></wc-sync-settings>
</div>`;

const preview: Preview = {
  id: 'wc-sync-settings',
  title: 'Bank Sync',
  group: 'Forms',
  description:
    'Connect a SimpleFIN Bridge with a setup token, link each bank account to a Nigel account, and preview or run a sync.',
  layout: 'stack',
  states: [
    { name: 'not configured', render: () => sync({ configured: false, accounts: [] }) },
    {
      name: 'configuring',
      render: () => sync({ configured: false, accounts: [], busy: 'setup' }),
    },
    {
      name: 'connected, nothing shared yet',
      render: () =>
        sync({
          accounts: [],
          notices: ['Connected, but the bridge’s accounts could not be read yet.'],
        }),
    },
    { name: 'configured', render: () => sync() },
    {
      name: 'several feeds into one account',
      render: () => sync({ accounts: CARD_PROGRAM }),
    },
    { name: 'syncing', render: () => sync({ busy: 'sync' }) },
    { name: 'preview results', render: () => sync({ outcome: outcome(true) }) },
    { name: 'sync results', render: () => sync({ outcome: outcome(false) }) },
    {
      name: 'error',
      render: () =>
        sync({
          error:
            'SimpleFIN refused the stored credentials: access was revoked or has expired.',
        }),
    },
  ],
};

export default preview;
