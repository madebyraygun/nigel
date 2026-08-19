import { describe, it, expect, afterEach, vi } from 'vitest';
import './import.js';
import type { NigelImportScreen } from './import.js';
import {
  EMPTY_IMPORT_FORM,
  GENERIC_FORMAT_CHOICE,
  type ImportFormValue,
  type NcImportChangeDetail,
  type WcCountGrid,
  type WcDropzone,
  type WcImportForm,
  type WcSampleTable,
  unsupportedFileMessage,
} from '@nigel/ui';

import { ApiError, type DragDropEvent, type StagedUpload } from '../api/index.js';
import { UPLOAD_NOT_FOUND } from '../api/types.js';
import type { Account, ImporterFormat } from '../api/types.js';
import {
  EMPTY_IMPORT_CONFIRMATION,
  EMPTY_IMPORT_PREVIEW,
  FakeApiClient,
} from '../__mocks__/fake-api-client.js';

/**
 * 423 and 401 are deliberately untested here. The shell gates both before a
 * screen element is ever constructed (`boot` never reaches `ready` while
 * locked), so a lock test on this screen would assert behaviour that cannot
 * happen and would quietly pass forever.
 */

const ACCOUNTS: Account[] = [
  {
    id: 1,
    name: 'BofA Checking',
    accountType: 'checking',
    institution: 'BofA',
    lastFour: '1234',
  },
  {
    id: 2,
    name: 'BofA Credit Card',
    accountType: 'credit_card',
    institution: 'BofA',
    lastFour: '9876',
  },
];

const FORMATS: ImporterFormat[] = [
  { key: 'bofa_checking', name: 'Bank of America Checking', accountTypes: ['checking'] },
];

function statement(name = 'april-2025.csv', size = 8214): File {
  const file = new File(['date,description,amount\n'], name);
  Object.defineProperty(file, 'size', { value: size });
  return file;
}

function client(): FakeApiClient {
  const fake = new FakeApiClient();
  fake.accounts = ACCOUNTS;
  fake.importFormats = FORMATS;
  fake.importPreview = {
    ...EMPTY_IMPORT_PREVIEW,
    imported: 42,
    skipped: 3,
    malformed: 1,
    format: 'bofa_checking',
    sample: [
      { date: '2025-04-01', description: 'ACME CORP', amount: 3000 },
      { date: '2025-04-03', description: 'ADOBE', amount: -59.99 },
    ],
  };
  fake.importConfirmation = {
    ...EMPTY_IMPORT_CONFIRMATION,
    imported: 42,
    skipped: 3,
    malformed: 1,
    format: 'bofa_checking',
    importId: 7,
    categorized: 38,
    stillFlagged: 6,
    snapshot: '/tmp/nigel/snapshots/pre-import-20250401-120000.db',
  };
  return fake;
}

async function settle(el: NigelImportScreen): Promise<void> {
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
}

async function mount(
  fake: FakeApiClient = client(),
): Promise<{ el: NigelImportScreen; fake: FakeApiClient }> {
  const el = document.createElement('nigel-import-screen');
  el.client = fake;
  document.body.appendChild(el);
  await settle(el);
  return { el, fake };
}

function dropzone(el: NigelImportScreen): WcDropzone {
  const found = el.shadowRoot?.querySelector<WcDropzone>('wc-dropzone');
  if (!found) throw new Error('no dropzone on screen');
  return found;
}

function form(el: NigelImportScreen): WcImportForm {
  const found = el.shadowRoot?.querySelector<WcImportForm>('wc-import-form');
  if (!found) throw new Error('no import form on screen');
  return found;
}

/** The screen's buttons, by their visible text. */
function button(el: NigelImportScreen, text: string): HTMLButtonElement | null {
  const buttons = [...(el.shadowRoot?.querySelectorAll('button') ?? [])];
  return (
    (buttons.find((b) => b.textContent?.trim().startsWith(text)) as HTMLButtonElement) ??
    null
  );
}

function panelHeadings(el: NigelImportScreen): string[] {
  return [...(el.shadowRoot?.querySelectorAll('wc-panel') ?? [])].map(
    (panel) => panel.getAttribute('heading') ?? '',
  );
}

async function choose(el: NigelImportScreen, file = statement()): Promise<void> {
  dropzone(el).dispatchEvent(
    new CustomEvent('nc-file-select', {
      detail: { file },
      bubbles: true,
      composed: true,
    }),
  );
  await settle(el);
}

async function setForm(
  el: NigelImportScreen,
  patch: Partial<ImportFormValue>,
): Promise<void> {
  const current: ImportFormValue = form(el).value;
  form(el).dispatchEvent(
    new CustomEvent<NcImportChangeDetail>('nc-import-change', {
      detail: { value: { ...current, ...patch } },
      bubbles: true,
      composed: true,
    }),
  );
  await settle(el);
}

async function click(el: NigelImportScreen, text: string): Promise<void> {
  const target = button(el, text);
  if (!target) throw new Error(`no "${text}" button on screen`);
  target.click();
  await settle(el);
}

/** Choose a file, name an account, and preview — the common opening. */
async function toPreview(
  el: NigelImportScreen,
  patch: Partial<ImportFormValue> = {},
): Promise<void> {
  await choose(el);
  await setForm(el, { account: 'BofA Checking', ...patch });
  await click(el, 'Preview');
}

function bodyOf(call: string): Record<string, unknown> {
  return JSON.parse(call.slice(call.indexOf(':') + 1));
}

describe('nigel-import-screen', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('loads accounts, formats and profiles on enter', async () => {
    const { fake } = await mount();
    expect(fake.calls).toEqual(['getAccounts', 'getImportFormats', 'getCsvProfiles']);
  });

  it('walks select, preview and confirm', async () => {
    const { el, fake } = await mount();

    await toPreview(el);

    expect(fake.calls).toContain('uploadImport:april-2025.csv');
    expect(panelHeadings(el)).toContain('Preview');
    const sample = el.shadowRoot?.querySelector<WcSampleTable>('wc-sample-table');
    expect(sample?.rows).toHaveLength(2);

    await click(el, 'Import 42 transactions');

    expect(panelHeadings(el)).toEqual(['Import complete']);
    const counts = el.shadowRoot?.querySelector<WcCountGrid>('wc-count-grid');
    expect(counts?.items.map((item) => [item.label, item.value])).toEqual([
      ['Imported', 42],
      ['Duplicates', 3],
      ['Malformed', 1],
      ['Categorized', 38],
      ['Still flagged', 6],
    ]);
  });

  it('calls upload, preview and confirm in that order', async () => {
    const { el, fake } = await mount();
    await toPreview(el);
    await click(el, 'Import 42');

    const names = fake.calls
      .map((call) => call.split(':')[0])
      .filter((name) =>
        ['uploadImport', 'previewImport', 'confirmImport'].includes(name),
      );
    expect(names).toEqual(['uploadImport', 'previewImport', 'confirmImport']);
  });

  it('shows the snapshot path and a link to the flagged review', async () => {
    const { el } = await mount();
    await toPreview(el);
    await click(el, 'Import 42');

    expect(el.shadowRoot?.querySelector('.snapshot')?.textContent).toContain(
      'pre-import-20250401-120000.db',
    );
    const link = el.shadowRoot?.querySelector('a[href="#/review"]');
    expect(link?.textContent).toContain('Review 6 flagged');
  });

  it('offers no review link when nothing is flagged', async () => {
    const fake = client();
    fake.importConfirmation = { ...fake.importConfirmation, stillFlagged: 0 };
    const { el } = await mount(fake);
    await toPreview(el);
    await click(el, 'Import 42');

    expect(el.shadowRoot?.querySelector('a[href="#/review"]')).toBeNull();
  });

  it('blocks the confirm for a duplicate file', async () => {
    const fake = client();
    fake.importPreview = { ...EMPTY_IMPORT_PREVIEW, duplicateFile: true };
    const { el } = await mount(fake);

    await toPreview(el);

    expect(panelHeadings(el)).toContain('Already imported');
    expect(el.shadowRoot?.querySelector('wc-notice-bar')?.getAttribute('variant')).toBe(
      'warning',
    );
    expect(button(el, 'Import')).toBeNull();
    expect(fake.calls.some((call) => call.startsWith('confirmImport'))).toBe(false);
  });

  it('blocks the confirm when the preview would import nothing', async () => {
    // Confirming would still record the file's checksum, after which a
    // corrected format or mapping could never import it.
    const fake = client();
    fake.importPreview = {
      ...EMPTY_IMPORT_PREVIEW,
      imported: 0,
      skipped: 4,
      format: 'bofa_checking',
    };
    const { el } = await mount(fake);

    await toPreview(el);

    const confirm = button(el, 'Import 0');
    expect(confirm?.disabled).toBe(true);
    expect(
      el.shadowRoot?.querySelector('wc-notice-bar')?.getAttribute('message'),
    ).toContain('nothing here to import');

    confirm?.click();
    await settle(el);
    expect(fake.calls.some((call) => call.startsWith('confirmImport'))).toBe(false);
  });

  it('sends the mapping and the profile name for a generic CSV', async () => {
    const { el, fake } = await mount();

    await toPreview(el, {
      format: GENERIC_FORMAT_CHOICE,
      mapping: { dateCol: 3, descCol: 1, amountCol: 4, dateFormat: '%d/%m/%Y' },
      saveProfile: 'chase',
    });
    await click(el, 'Import 42');

    const confirm = fake.calls.find((call) => call.startsWith('confirmImport'));
    const body = bodyOf(confirm!);
    expect(body.mapping).toEqual({
      dateCol: 3,
      descCol: 1,
      amountCol: 4,
      dateFormat: '%d/%m/%Y',
    });
    expect(body.saveProfile).toBe('chase');
    expect(body).not.toHaveProperty('format');
  });

  it('re-reads the profile list after saving one', async () => {
    const { el, fake } = await mount();
    await toPreview(el, {
      format: GENERIC_FORMAT_CHOICE,
      saveProfile: 'chase',
    });
    await click(el, 'Import 42');

    expect(fake.csvProfiles.map((p) => p.name)).toContain('chase');
    expect(fake.calls.filter((call) => call === 'getCsvProfiles')).toHaveLength(2);
  });

  it('sends neither format nor mapping when detecting', async () => {
    const { el, fake } = await mount();
    await toPreview(el);

    const body = bodyOf(fake.calls.find((c) => c.startsWith('previewImport'))!);
    expect(body).not.toHaveProperty('format');
    expect(body).not.toHaveProperty('mapping');
    expect(body.account).toBe('BofA Checking');
  });

  it('sends an explicit format alone', async () => {
    const { el, fake } = await mount();
    await toPreview(el, { format: 'bofa_checking' });

    const body = bodyOf(fake.calls.find((c) => c.startsWith('previewImport'))!);
    expect(body.format).toBe('bofa_checking');
    expect(body).not.toHaveProperty('mapping');
  });

  it('surfaces a bad mapping under the mapping form and allows another try', async () => {
    const fake = client();
    fake.previewErrorOnce = new ApiError({
      code: 'bad_request',
      rawCode: 'bad_request',
      message: 'Column 9 is past the end of every row.',
      status: 400,
    });
    const { el } = await mount(fake);

    await toPreview(el, { format: GENERIC_FORMAT_CHOICE });

    expect(form(el).mappingError).toContain('Column 9');
    expect(panelHeadings(el)).not.toContain('Preview');
    // The file is still chosen, so correcting the columns is the only work left.
    expect(dropzone(el).filename).toBe('april-2025.csv');

    await click(el, 'Preview');
    expect(panelHeadings(el)).toContain('Preview');
    // The upload was cached: the retry cost one request, not two.
    expect(fake.calls.filter((c) => c.startsWith('uploadImport'))).toHaveLength(1);
  });

  it('reuses the upload when only the mapping changed', async () => {
    const { el, fake } = await mount();
    await toPreview(el, { format: GENERIC_FORMAT_CHOICE });
    await setForm(el, { mapping: { dateCol: 2, descCol: 0, amountCol: 3, dateFormat: '%Y-%m-%d' } });
    await click(el, 'Preview');

    expect(fake.calls.filter((c) => c.startsWith('uploadImport'))).toHaveLength(1);
    expect(fake.calls.filter((c) => c.startsWith('previewImport'))).toHaveLength(2);
  });

  it('re-uploads once when the upload has expired', async () => {
    const fake = client();
    fake.previewErrorOnce = new ApiError({
      code: 'not_found',
      rawCode: 'not_found',
      message: 'gone',
      status: 404,
      details: { reason: UPLOAD_NOT_FOUND },
    });
    const { el } = await mount(fake);

    await toPreview(el);

    // Recovered without saying anything: the file never left the browser.
    expect(panelHeadings(el)).toContain('Preview');
    expect(fake.calls.filter((c) => c.startsWith('uploadImport'))).toHaveLength(2);
    expect(dropzone(el).error).toBe('');
  });

  it('gives up after one re-upload and says the upload expired', async () => {
    const fake = client();
    fake.previewError = new ApiError({
      code: 'not_found',
      rawCode: 'not_found',
      message: 'gone',
      status: 404,
      details: { reason: UPLOAD_NOT_FOUND },
    });
    const { el } = await mount(fake);

    await toPreview(el);

    expect(dropzone(el).error).toContain('expired');
    expect(fake.calls.filter((c) => c.startsWith('uploadImport'))).toHaveLength(2);
  });

  it('puts an oversize rejection from the server under the dropzone', async () => {
    const fake = client();
    fake.uploadError = new ApiError({
      code: 'payload_too_large',
      rawCode: 'payload_too_large',
      message: 'That file is over the 25 MB limit.',
      status: 413,
    });
    const { el } = await mount(fake);

    await toPreview(el);

    expect(dropzone(el).error).toContain('25 MB');
  });

  it('never uploads a file the dropzone already refused', async () => {
    const { el, fake } = await mount();
    await setForm(el, { account: 'BofA Checking' });

    dropzone(el).dispatchEvent(
      new CustomEvent('nc-file-error', {
        detail: { message: 'nigel reads .csv, .xlsx, .xls statements.' },
        bubbles: true,
        composed: true,
      }),
    );
    await settle(el);

    expect(dropzone(el).error).toContain('.csv');
    expect(fake.calls.some((call) => call.startsWith('uploadImport'))).toBe(false);
    expect(button(el, 'Preview')?.disabled).toBe(true);
  });

  it('puts a missing cargo feature under the format select', async () => {
    const fake = client();
    fake.previewError = new ApiError({
      code: 'feature_disabled',
      rawCode: 'feature_disabled',
      message: 'This build has no Gusto payroll support.',
      status: 501,
    });
    const { el } = await mount(fake);

    await toPreview(el, { format: 'gusto_payroll' });

    expect(form(el).formatError).toContain('Gusto');
  });

  it('puts an unknown account under the account select and toasts it', async () => {
    const fake = client();
    fake.previewError = new ApiError({
      code: 'not_found',
      rawCode: 'not_found',
      message: 'No account named that.',
      status: 404,
    });
    const { el } = await mount(fake);
    const toasted = vi.fn();
    el.addEventListener('nc-toast', toasted);

    await toPreview(el);

    expect(form(el).accountError).toContain('No account');
    expect(toasted).toHaveBeenCalled();
  });

  it('resets for a second import without a reload', async () => {
    const { el, fake } = await mount();
    await toPreview(el);
    await click(el, 'Import 42');

    await click(el, 'Import another');

    expect(panelHeadings(el)).toEqual(['Import a statement']);
    expect(dropzone(el).filename).toBe('');
    // The account survives: a second statement for the same account is the
    // ordinary next thing to do.
    expect(form(el).value.account).toBe('BofA Checking');

    await choose(el, statement('may-2025.csv'));
    await click(el, 'Preview');
    await click(el, 'Import 42');

    expect(panelHeadings(el)).toEqual(['Import complete']);
    const uploads = fake.calls.filter((c) => c.startsWith('uploadImport'));
    expect(uploads).toEqual(['uploadImport:april-2025.csv', 'uploadImport:may-2025.csv']);

    // The second confirm used a fresh upload, not the one the first consumed.
    const confirms = fake.calls.filter((c) => c.startsWith('confirmImport'));
    expect(bodyOf(confirms[0]).uploadId).not.toBe(bodyOf(confirms[1]).uploadId);
  });

  it('drops a stale preview when the file changes', async () => {
    const { el } = await mount();
    await toPreview(el);
    expect(panelHeadings(el)).toContain('Preview');

    await choose(el, statement('may-2025.csv'));

    expect(panelHeadings(el)).not.toContain('Preview');
  });

  it('drops a stale preview when the format changes', async () => {
    const { el } = await mount();
    await toPreview(el);

    await setForm(el, { format: 'bofa_checking' });

    // The old preview described a different reading of the same bytes.
    expect(panelHeadings(el)).not.toContain('Preview');
  });

  it('preselects the only account there is', async () => {
    const fake = client();
    fake.accounts = [ACCOUNTS[0]];
    const { el } = await mount(fake);

    expect(form(el).value.account).toBe('BofA Checking');
  });

  it('leaves the account unchosen when there is more than one', async () => {
    const { el } = await mount();
    expect(form(el).value.account).toBe('');
    expect(button(el, 'Preview')?.disabled).toBe(true);
  });

  it('keeps edits made while the account list was still loading', async () => {
    const fake = client();
    fake.accounts = [ACCOUNTS[0]];
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const getAccounts = fake.getAccounts.bind(fake);
    fake.getAccounts = async () => {
      await gate;
      return getAccounts();
    };

    const el = document.createElement('nigel-import-screen');
    el.client = fake;
    document.body.appendChild(el);
    await el.updateComplete;

    await setForm(el, { format: GENERIC_FORMAT_CHOICE, saveProfile: 'chase' });
    release();
    await settle(el);

    // The lists arrive after the screen does; the preselect is one field, not
    // a replacement for whatever was typed while they were in flight.
    expect(form(el).value.format).toBe(GENERIC_FORMAT_CHOICE);
    expect(form(el).value.saveProfile).toBe('chase');
    expect(form(el).value.account).toBe('BofA Checking');
  });

  it('offers no cancel on the screen a finished import resets to', async () => {
    const { el } = await mount();
    await toPreview(el);
    await click(el, 'Import 42');
    await click(el, 'Import another');

    // The account the reset deliberately kept is the new baseline, not work
    // waiting to be abandoned.
    expect(form(el).value.account).toBe('BofA Checking');
    expect(button(el, 'Cancel')).toBeNull();

    await choose(el);
    expect(button(el, 'Cancel')).not.toBeNull();
  });

  it('cancels back to what the reset kept, not to an empty form', async () => {
    const { el } = await mount();
    await toPreview(el);
    await click(el, 'Import 42');
    await click(el, 'Import another');
    await choose(el, statement('may-2025.csv'));
    await setForm(el, { format: 'bofa_checking' });

    await click(el, 'Cancel');

    expect(dropzone(el).filename).toBe('');
    expect(form(el).value.account).toBe('BofA Checking');
    expect(form(el).value.format).toBe('');
    expect(button(el, 'Cancel')).toBeNull();
  });

  it('offers no cancel until there is an import to abandon', async () => {
    const { el } = await mount();
    expect(button(el, 'Cancel')).toBeNull();

    await choose(el);
    expect(button(el, 'Cancel')).not.toBeNull();
  });

  it('offers a cancel for a form touched before any file is chosen', async () => {
    const { el } = await mount();
    await setForm(el, { account: 'BofA Checking' });

    expect(button(el, 'Cancel')).not.toBeNull();
  });

  it('offers a cancel for a file the dropzone refused', async () => {
    const { el } = await mount();
    dropzone(el).dispatchEvent(
      new CustomEvent('nc-file-error', {
        detail: { message: 'nigel reads .csv, .xlsx, .xls statements.' },
        bubbles: true,
        composed: true,
      }),
    );
    await settle(el);

    expect(button(el, 'Cancel')).not.toBeNull();
  });

  it('cancels a chosen file before any preview', async () => {
    const { el, fake } = await mount();
    await choose(el);
    await setForm(el, { account: 'BofA Checking', format: 'bofa_checking' });

    await click(el, 'Cancel');

    expect(dropzone(el).filename).toBe('');
    expect(form(el).value).toEqual({ ...EMPTY_IMPORT_FORM });
    expect(button(el, 'Cancel')).toBeNull();
    expect(button(el, 'Preview')?.disabled).toBe(true);
    expect(fake.calls).toEqual(['getAccounts', 'getImportFormats', 'getCsvProfiles']);
  });

  it('cancels a previewed import and returns the screen to its initial state', async () => {
    const { el } = await mount();
    await toPreview(el);
    expect(panelHeadings(el)).toContain('Preview');

    await click(el, 'Cancel');

    expect(panelHeadings(el)).toEqual(['Import a statement']);
    expect(dropzone(el).filename).toBe('');
    expect(dropzone(el).error).toBe('');
    // The wrong account is one of the two things a cancel corrects, so unlike
    // the reset a finished import offers, this one clears it.
    expect(form(el).value.account).toBe('');
    expect(button(el, 'Cancel')).toBeNull();
  });

  it('cancels out of the duplicate-file dead end', async () => {
    const fake = client();
    fake.importPreview = { ...EMPTY_IMPORT_PREVIEW, duplicateFile: true };
    const { el } = await mount(fake);
    await toPreview(el);
    expect(panelHeadings(el)).toContain('Already imported');

    await click(el, 'Cancel');

    expect(panelHeadings(el)).toEqual(['Import a statement']);
    expect(dropzone(el).filename).toBe('');
  });

  it('cancels back to the preselected account rather than to none', async () => {
    const fake = client();
    fake.accounts = [ACCOUNTS[0]];
    const { el } = await mount(fake);
    await choose(el);
    await click(el, 'Preview');

    await click(el, 'Cancel');

    // One account is not a choice, so the initial state has it filled in.
    expect(form(el).value.account).toBe('BofA Checking');
    expect(button(el, 'Cancel')).toBeNull();
  });

  it('clears the error a failed preview left behind', async () => {
    const fake = client();
    fake.previewError = new ApiError({
      code: 'bad_request',
      rawCode: 'bad_request',
      message: 'Column 9 is past the end of every row.',
      status: 400,
    });
    const { el } = await mount(fake);
    await toPreview(el, { format: GENERIC_FORMAT_CHOICE });
    expect(form(el).mappingError).toContain('Column 9');

    await click(el, 'Cancel');

    expect(form(el).mappingError).toBe('');
  });

  it('leaves the spooled upload to the purge and never names it again', async () => {
    const { el, fake } = await mount();
    await toPreview(el);
    const abandoned = [...fake.liveUploads];
    expect(abandoned).toHaveLength(1);
    const before = [...fake.calls];

    await click(el, 'Cancel');

    // Cancel says nothing to the server: an upload is a file on disk with an
    // mtime, and the hourly sweep is what collects it.
    expect(fake.calls).toEqual(before);

    // A second attempt at the same file uploads afresh rather than reaching
    // for the id the cancelled import was holding.
    await toPreview(el);
    await click(el, 'Import 42');

    expect(fake.calls.filter((c) => c.startsWith('uploadImport'))).toHaveLength(2);
    const named = fake.calls
      .filter((c) => c.startsWith('previewImport') || c.startsWith('confirmImport'))
      .map((call) => bodyOf(call).uploadId);
    expect(named.slice(1)).not.toContain(abandoned[0]);
  });

  it('refuses to cancel out from under a request in flight', async () => {
    const fake = client();
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const upload = fake.uploadImport.bind(fake);
    fake.uploadImport = async (file: File) => {
      await gate;
      return upload(file);
    };

    const { el } = await mount(fake);
    await choose(el);
    await setForm(el, { account: 'BofA Checking' });

    button(el, 'Preview')?.click();
    await el.updateComplete;

    // The api client sends nothing it can call back, so an upload cannot be
    // recalled — only waited out.
    expect(button(el, 'Cancel')?.disabled).toBe(true);

    release();
    await settle(el);

    expect(panelHeadings(el)).toContain('Preview');
    expect(button(el, 'Cancel')?.disabled).toBe(false);
  });

  it('still renders the form when the profile list fails', async () => {
    const fake = client();
    fake.csvProfilesError = new ApiError({
      code: 'internal',
      rawCode: 'internal',
      message: 'profiles are down',
      status: 500,
    });
    const { el } = await mount(fake);

    // One failed list is no reason to withhold the other two.
    expect(form(el).accounts).toHaveLength(2);
    expect(form(el).formats).toHaveLength(1);
    expect(el.shadowRoot?.querySelector('.load-error')?.textContent).toContain(
      'profiles are down',
    );
  });
});

/**
 * Point the fake client at a native source, and hand the test the two things
 * only the shell can do: answer the dialog, and drop a file on the window.
 */
function nativeSource(fake: FakeApiClient) {
  const staged: StagedUpload[] = [];
  let nextId = 1;
  let picked: string | null = null;
  let handler: ((event: DragDropEvent) => void) | null = null;
  let subscribes = 0;
  let unsubscribes = 0;

  const stage = (path: string): StagedUpload => {
    const uploadId = `staged-${nextId++}`;
    fake.liveUploads.add(uploadId);
    const upload = {
      uploadId,
      filename: path.slice(path.lastIndexOf('/') + 1),
      size: 8214,
      path,
    };
    staged.push(upload);
    return upload;
  };

  fake.importSourceValue = {
    kind: 'native',
    pick: async () => (picked === null ? null : stage(picked)),
    stagePath: async (path) => stage(path),
    onDragDrop: (fn) => {
      subscribes += 1;
      handler = fn;
      return () => {
        unsubscribes += 1;
        handler = null;
      };
    },
  };

  return {
    /** What the next dialog answers; null is a cancel, which is the default. */
    willPick: (path: string | null) => {
      picked = path;
    },
    staged,
    emit: (event: DragDropEvent) => handler?.(event),
    counts: () => ({ subscribes, unsubscribes }),
    isSubscribed: () => handler !== null,
  };
}

describe('the import screen in native mode', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('puts the dropzone in native mode', async () => {
    const fake = client();
    nativeSource(fake);
    const { el } = await mount(fake);

    expect(dropzone(el).native).toBe(true);
  });

  it('stages what the native dialog returns and previews it', async () => {
    const fake = client();
    const native = nativeSource(fake);
    native.willPick('/home/books/cedar-april-2025.csv');
    const { el } = await mount(fake);

    dropzone(el).dispatchEvent(
      new CustomEvent('nc-pick-request', { bubbles: true, composed: true }),
    );
    await settle(el);

    expect(dropzone(el).filename).toBe('cedar-april-2025.csv');
    await setForm(el, { account: 'BofA Checking' });
    await click(el, 'Preview');

    expect(panelHeadings(el)).toContain('Preview');
    // Nothing was uploaded: the file never crossed the wire.
    expect(fake.calls.filter((c) => c.startsWith('uploadImport'))).toHaveLength(0);
    const previewCall = fake.calls.find((c) => c.startsWith('previewImport'));
    if (previewCall === undefined) throw new Error('no previewImport call');
    expect(bodyOf(previewCall).uploadId).toBe('staged-1');
  });

  it('leaves the screen alone when the dialog is cancelled', async () => {
    const fake = client();
    nativeSource(fake);
    const { el } = await mount(fake);

    dropzone(el).dispatchEvent(
      new CustomEvent('nc-pick-request', { bubbles: true, composed: true }),
    );
    await settle(el);

    expect(dropzone(el).filename).toBe('');
    expect(dropzone(el).error).toBe('');
  });

  it('highlights the dropzone while a file is over the window', async () => {
    const fake = client();
    const native = nativeSource(fake);
    const { el } = await mount(fake);

    native.emit({ type: 'over' });
    await settle(el);
    expect(dropzone(el).highlight).toBe(true);

    native.emit({ type: 'leave' });
    await settle(el);
    expect(dropzone(el).highlight).toBe(false);
  });

  it('stages the first usable path in a drop', async () => {
    const fake = client();
    const native = nativeSource(fake);
    const { el } = await mount(fake);

    native.emit({
      type: 'drop',
      paths: ['/home/books/receipt.pdf', '/home/books/juniper-may-2025.xlsx'],
    });
    await settle(el);

    expect(native.staged.map((s) => s.path)).toEqual([
      '/home/books/juniper-may-2025.xlsx',
    ]);
    expect(dropzone(el).filename).toBe('juniper-may-2025.xlsx');
    expect(dropzone(el).highlight).toBe(false);
  });

  it('says the same thing the dropzone would about a drop it cannot read', async () => {
    const fake = client();
    const native = nativeSource(fake);
    const { el } = await mount(fake);

    native.emit({ type: 'drop', paths: ['/home/books/receipt.pdf'] });
    await settle(el);

    expect(dropzone(el).error).toBe(unsupportedFileMessage());
    expect(native.staged).toHaveLength(0);
  });

  it('re-stages from the retained path when the spool has forgotten the file', async () => {
    const fake = client();
    const native = nativeSource(fake);
    fake.previewErrorOnce = new ApiError({
      code: 'not_found',
      rawCode: 'not_found',
      message: 'gone',
      status: 404,
      details: { reason: UPLOAD_NOT_FOUND },
    });
    const { el } = await mount(fake);

    native.emit({ type: 'drop', paths: ['/home/books/cedar-april-2025.csv'] });
    await settle(el);
    await setForm(el, { account: 'BofA Checking' });
    await click(el, 'Preview');

    // Recovered without saying anything: the file is still on disk.
    expect(panelHeadings(el)).toContain('Preview');
    expect(native.staged.map((s) => s.path)).toEqual([
      '/home/books/cedar-april-2025.csv',
      '/home/books/cedar-april-2025.csv',
    ]);
    expect(dropzone(el).error).toBe('');
  });

  it('unsubscribes from the window on disconnect and resubscribes on reconnect', async () => {
    const fake = client();
    const native = nativeSource(fake);
    const { el } = await mount(fake);

    expect(native.counts()).toEqual({ subscribes: 1, unsubscribes: 0 });

    el.remove();
    expect(native.counts()).toEqual({ subscribes: 1, unsubscribes: 1 });
    expect(native.isSubscribed()).toBe(false);

    document.body.appendChild(el);
    await settle(el);
    expect(native.counts()).toEqual({ subscribes: 2, unsubscribes: 1 });
  });

  it('leaves the dropzone alone when the client is a browser', async () => {
    const { el } = await mount();

    expect(dropzone(el).native).toBe(false);
    expect(dropzone(el).highlight).toBe(false);
  });
});
