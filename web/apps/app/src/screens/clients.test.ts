import { describe, it, expect, afterEach, vi } from 'vitest';
import './clients.js';
import type { NigelClientsScreen } from './clients.js';
import type {
  WcClientForm,
  WcManagerDialog,
  WcManagerLayout,
  WcManagerTable,
} from '@nigel/ui';
import { conflictError, FakeApiClient } from '../__mocks__/fake-api-client.js';
import type { Client } from '../api/types.js';
import type { ScreenId } from './registry.js';

const CLIENTS: Client[] = [
  {
    id: 1,
    name: 'Acme Co',
    email: 'ap@acme.test',
    billingAddress: '1 Main St',
    notes: null,
    archivedAt: null,
  },
  {
    id: 2,
    name: 'Globex',
    email: null,
    billingAddress: null,
    notes: null,
    archivedAt: null,
  },
];

function client(clients: Client[] = CLIENTS): FakeApiClient {
  const fake = new FakeApiClient();
  fake.clients = clients.map((row) => ({ ...row }));
  return fake;
}

async function settle(el: NigelClientsScreen): Promise<void> {
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
}

interface Mounted {
  el: NigelClientsScreen;
  fake: FakeApiClient;
  routes: { screen: ScreenId; params: string }[];
}

async function mount(
  fake: FakeApiClient = client(),
  query = '',
): Promise<Mounted> {
  const routes: { screen: ScreenId; params: string }[] = [];
  const el = document.createElement('nigel-clients-screen');
  el.client = fake;
  el.params = new URLSearchParams(query);
  el.navigate = (screen, params) =>
    routes.push({ screen, params: params?.toString() ?? '' });
  document.body.appendChild(el);
  await settle(el);
  return { el, fake, routes };
}

function layout(el: NigelClientsScreen): WcManagerLayout {
  const found = el.shadowRoot?.querySelector<WcManagerLayout>('wc-manager-layout');
  if (!found) throw new Error('no layout on screen');
  return found;
}

function table(el: NigelClientsScreen): WcManagerTable {
  const found = el.shadowRoot?.querySelector<WcManagerTable>('wc-manager-table');
  if (!found) throw new Error('no table on screen');
  return found;
}

function rowLabels(el: NigelClientsScreen): string[] {
  return [...(table(el).shadowRoot?.querySelectorAll('tr[data-row] td:first-child') ?? [])]
    .map((cell) => cell.textContent?.trim().split('\n')[0].trim() ?? '');
}

function dialog(el: NigelClientsScreen): WcManagerDialog | null {
  return el.shadowRoot?.querySelector<WcManagerDialog>('wc-manager-dialog') ?? null;
}

function form(el: NigelClientsScreen): WcClientForm {
  const found = dialog(el)?.querySelector<WcClientForm>('wc-client-form');
  if (!found) throw new Error('no client form on screen');
  return found;
}

async function type(
  el: NigelClientsScreen,
  hook: string,
  value: string,
): Promise<void> {
  const field = form(el).shadowRoot?.querySelector<HTMLInputElement>(hook);
  if (!field) throw new Error(`no ${hook} in the form`);
  field.value = value;
  field.dispatchEvent(new Event('input'));
  await settle(el);
}

async function openAdd(el: NigelClientsScreen): Promise<void> {
  layout(el).dispatchEvent(new CustomEvent('nc-manager-add'));
  await settle(el);
}

async function rowAction(
  el: NigelClientsScreen,
  action: string,
  id: number,
): Promise<void> {
  table(el).dispatchEvent(
    new CustomEvent('nc-manager-action', {
      detail: { action, id },
      bubbles: true,
      composed: true,
    }),
  );
  await settle(el);
}

async function save(el: NigelClientsScreen): Promise<void> {
  dialog(el)?.dispatchEvent(new CustomEvent('nc-manager-save'));
  await settle(el);
}

async function answerConfirm(answer: boolean): Promise<void> {
  const ui = await import('@nigel/ui');
  vi.spyOn(ui, 'confirmDialog').mockResolvedValue(answer);
}

describe('nigel-clients-screen', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  describe('contacts', () => {
    it('fetches the detail before the edit dialog can be filled in', async () => {
      const { el, fake } = await mount();
      await rowAction(el, 'edit', 1);

      // The list row is a bare `Client`; the contacts come from one request on
      // a deliberate click.
      expect(fake.calls).toContain('getClient:1');
      expect(form(el).value.contacts).toEqual([
        { email: 'ap@acme.test', name: '', title: '' },
      ]);
    });

    it('never offers a form the detail could not fill in', async () => {
      // An editable form here would carry an empty contacts baseline, and
      // saving it would whole-list-replace — delete — every address the client
      // actually has.
      const fake = client();
      fake.clientError = new Error('boom');
      const { el } = await mount(fake);
      await rowAction(el, 'edit', 1);

      expect(dialog(el)).not.toBeNull();
      expect(dialog(el)?.error).toBeTruthy();
      expect(dialog(el)?.querySelector('wc-client-form')).toBeNull();
      expect(dialog(el)?.querySelector('[data-retry-editor]')).not.toBeNull();
      // The list behind it loaded fine.
      expect(layout(el).error).toBeNull();

      // And a save cannot be coaxed out of it.
      await save(el);
      expect(fake.calls.some((c) => c.startsWith('updateClient:'))).toBe(false);
    });

    it('retries the detail and then offers the form', async () => {
      const fake = client();
      fake.clientError = new Error('boom');
      const { el } = await mount(fake);
      await rowAction(el, 'edit', 1);

      fake.clientError = null;
      dialog(el)?.querySelector<HTMLElement>('[data-retry-editor]')?.click();
      await settle(el);

      expect(dialog(el)?.querySelector('wc-client-form')).not.toBeNull();
      expect(form(el).value.contacts).toEqual([
        { email: 'ap@acme.test', name: '', title: '' },
      ]);
    });

    it('sends the whole contact list when it changes', async () => {
      const { el, fake } = await mount();
      await rowAction(el, 'edit', 1);

      const contacts = form(el).shadowRoot?.querySelector<HTMLElement>('[data-add-contact]');
      contacts?.click();
      await settle(el);
      const email = form(el).shadowRoot?.querySelectorAll<HTMLInputElement>(
        '[data-contact-email]',
      )[1];
      email!.value = 'dana@acme.test';
      email!.dispatchEvent(new Event('input'));
      await settle(el);
      await save(el);

      const call = fake.calls.find((c) => c.startsWith('updateClient:1:'));
      expect(call).toBeDefined();
      const body = JSON.parse(call!.slice('updateClient:1:'.length));
      expect(body.contacts).toEqual([
        { email: 'ap@acme.test', name: null, title: null, isBilling: true },
        { email: 'dana@acme.test', name: null, title: null, isBilling: false },
      ]);
      // `email` and `contacts` in one body is a 400, so the screen never sends
      // both.
      expect(body.email).toBeUndefined();
    });

    it('closes rather than patching when the contact list did not move', async () => {
      const { el, fake } = await mount();
      await rowAction(el, 'edit', 1);
      await save(el);

      expect(fake.calls.some((c) => c.startsWith('updateClient:'))).toBe(false);
      expect(dialog(el)).toBeNull();
    });

    it('refuses a blank row before the server sees it', async () => {
      const { el, fake } = await mount();
      await rowAction(el, 'edit', 1);
      form(el).shadowRoot?.querySelector<HTMLElement>('[data-add-contact]')?.click();
      await settle(el);
      await save(el);

      expect(fake.calls.some((c) => c.startsWith('updateClient:'))).toBe(false);
      expect(form(el).errors.contacts?.[1]).toBe('An email address is required');
    });
  });

  describe('archiving', () => {
    const ARCHIVED: Client[] = [
      ...CLIENTS,
      {
        id: 3,
        name: 'Umbrella Corp',
        email: 'ap@umbrella.test',
        billingAddress: null,
        notes: null,
        archivedAt: '2026-03-01',
      },
    ];

    it('asks for the active list by default and the whole one when told', async () => {
      const { fake } = await mount(client(ARCHIVED));
      expect(fake.calls).toContain('getClients');
      expect(fake.calls).not.toContain('getClients:all');

      const withArchived = await mount(client(ARCHIVED), 'archived=1');
      expect(withArchived.fake.calls).toContain('getClients:all');
      expect(rowLabels(withArchived.el)).toContain('Umbrella Corp');
    });

    it('marks an archived row with a badge rather than a column', async () => {
      const { el } = await mount(client(ARCHIVED), 'archived=1');
      const badges = table(el)
        .shadowRoot?.querySelectorAll('wc-row-badge');
      expect(badges?.length).toBe(1);
      expect(badges?.[0].getAttribute('label')).toBe('Archived');
      // No fourth column: the badge rides in the name cell.
      expect(table(el).shadowRoot?.querySelectorAll('thead th').length).toBe(4);
    });

    it('writes the filter into the route rather than into state', async () => {
      const { el, routes } = await mount();
      const toggle = el.shadowRoot?.querySelector<HTMLElement>(
        '[data-archived-toggle]',
      );
      toggle?.dispatchEvent(new Event('change', { bubbles: true }));
      await settle(el);
      expect(routes).toEqual([{ screen: 'clients', params: 'archived=1' }]);
    });

    it('offers Archive on an active row and Unarchive on an archived one', async () => {
      const { el } = await mount(client(ARCHIVED), 'archived=1');
      const labels = [...(table(el).shadowRoot?.querySelectorAll('tr[data-row]') ?? [])].map(
        (row) =>
          [...row.querySelectorAll('wa-button')].map((b) => b.getAttribute('data-action')),
      );
      expect(labels[0]).toContain('archive');
      expect(labels[2]).toContain('unarchive');
    });

    it('archives without a confirmation and refetches the list', async () => {
      const { el, fake } = await mount(client(ARCHIVED));
      await rowAction(el, 'archive', 2);

      expect(fake.calls).toContain('archiveClient:2');
      // Refetched, not spliced: the row leaves the unfiltered list.
      expect(fake.calls.filter((call) => call === 'getClients')).toHaveLength(2);
      expect(rowLabels(el)).not.toContain('Globex');
    });

    it('unarchives from the filtered list', async () => {
      const { el, fake } = await mount(client(ARCHIVED), 'archived=1');
      await rowAction(el, 'unarchive', 3);
      expect(fake.calls).toContain('unarchiveClient:3');
    });

    it('reports a failed archive in the layout rather than silently', async () => {
      const fake = client(ARCHIVED);
      fake.archiveClientError = conflictError('client_archived', {
        message: 'nope',
      });
      const { el } = await mount(fake);
      await rowAction(el, 'archive', 2);
      expect(layout(el).error).toBeTruthy();
    });
  });

  it('lists the clients, with an em dash for the ones missing a field', async () => {
    const { el } = await mount();
    expect(table(el).rows.map((row) => row.cells)).toEqual([
      ['Acme Co', 'ap@acme.test', '1 Main St'],
      ['Globex', null, null],
    ]);
    expect(layout(el).count).toBe(2);
  });

  it('shows the empty state when there are none', async () => {
    const { el } = await mount(client([]));
    expect(layout(el).empty).toBe(true);
  });

  it('creates a client and then refetches the list', async () => {
    const { el, fake } = await mount();
    await openAdd(el);
    await type(el, '[data-name]', 'Initech');
    await save(el);

    expect(fake.calls).toContain('createClient:Initech');
    // The refetch is the point: no optimistic splice, because the list is
    // sorted by name and the server is the authority on where a new row lands.
    expect(fake.calls.filter((call) => call === 'getClients')).toHaveLength(2);
    expect(dialog(el)).toBeNull();
  });

  it('sends an empty optional field as null rather than an empty string', async () => {
    const { el, fake } = await mount();
    await openAdd(el);
    await type(el, '[data-name]', 'Initech');
    await save(el);

    const created = fake.clients.find((row) => row.name === 'Initech');
    expect(created?.email).toBeNull();
    expect(created?.billingAddress).toBeNull();
  });

  it('prefills the edit form and sends only what changed', async () => {
    const { el, fake } = await mount();
    await rowAction(el, 'edit', 1);
    expect(form(el).value.name).toBe('Acme Co');
    expect(form(el).value.contacts).toEqual([
      { email: 'ap@acme.test', name: '', title: '' },
    ]);

    await type(el, '[data-address]', '2 Elm St');
    await save(el);

    expect(fake.calls).toContain('updateClient:1:{"billingAddress":"2 Elm St"}');
  });

  it('closes rather than sending an all-absent patch', async () => {
    // An empty PATCH is a 400: a save with nothing changed is a close.
    const { el, fake } = await mount();
    await rowAction(el, 'edit', 1);
    await save(el);

    expect(fake.calls.some((call) => call.startsWith('updateClient'))).toBe(false);
    expect(dialog(el)).toBeNull();
  });

  it('renders a duplicate name in the dialog, beside the field', async () => {
    const fake = client();
    fake.takenClientNames.add('Globex');
    const { el } = await mount(fake);

    await openAdd(el);
    await type(el, '[data-name]', 'Globex');
    await save(el);

    expect(dialog(el)?.error).toBe('A client named “Globex” already exists.');
    expect(layout(el).error).toBeNull();
  });

  it('deletes behind a confirmation, and does nothing when it is declined', async () => {
    await answerConfirm(false);
    const { el, fake } = await mount();
    await rowAction(el, 'delete', 2);
    expect(fake.calls.some((call) => call.startsWith('deleteClient'))).toBe(false);

    await answerConfirm(true);
    await rowAction(el, 'delete', 2);
    expect(fake.calls).toContain('deleteClient:2');
    expect(fake.calls.filter((call) => call === 'getClients')).toHaveLength(2);
  });

  it('renders a blocked delete in the layout with the count and a way through', async () => {
    // `confirmDialog()` has resolved and removed itself by the time the request
    // fails, so there is no dialog left for the refusal to appear in.
    await answerConfirm(true);
    const fake = client();
    fake.clientInvoiceCounts[1] = 7;
    const { el, routes } = await mount(fake);

    await rowAction(el, 'delete', 1);

    expect(layout(el).error).toContain('7 invoices bill this client');
    expect(layout(el).errorActionLabel).toBe('Show those invoices');

    layout(el).dispatchEvent(new CustomEvent('nc-manager-error-action'));
    expect(routes).toEqual([{ screen: 'invoices', params: 'clientId=1' }]);
  });

  it('offers the invoices of any client from its row', async () => {
    const { el, routes } = await mount();
    await rowAction(el, 'invoices', 2);
    expect(routes).toEqual([{ screen: 'invoices', params: 'clientId=2' }]);
  });

  it('offers a retry when the list itself would not load', async () => {
    const fake = client();
    fake.clientsError = conflictError('nope', { message: 'Could not read the clients' });
    const { el } = await mount(fake);

    expect(layout(el).error).toBe('Could not read the clients');
    expect(layout(el).errorActionLabel).toBe('Try again');
  });

  it('refuses to save a client with no name', async () => {
    const { el, fake } = await mount();
    await openAdd(el);
    await save(el);

    expect(form(el).errors.name).toBe('Name is required');
    expect(fake.calls.some((call) => call.startsWith('createClient'))).toBe(false);
  });
});
