import { describe, it, expect, afterEach } from 'vitest';
import './wc-recipient-editor.js';
import {
  defaultRecipients,
  validateRecipients,
  type NcRecipientsChangeDetail,
  type RecipientContactOption,
  type RecipientEditorValue,
  WcRecipientEditor,
} from './wc-recipient-editor.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { describeControlsAdoption } from '../../preview/controls-suite.js';
import preview from './wc-recipient-editor.preview.js';

const CONTACTS: RecipientContactOption[] = [
  { name: 'Sam Example', email: 'sam@cedar.test', isBilling: false },
  { name: 'Pat Example', email: 'pat@cedar.test', isBilling: true },
  { name: null, email: 'legal@cedar.test', isBilling: false },
];

const SIGNED: RecipientEditorValue = {
  signer: { name: 'Pat Example', email: 'pat@cedar.test' },
  collaborators: [{ name: 'Sam Example', email: 'sam@cedar.test' }],
};

async function mount(props: Partial<WcRecipientEditor> = {}): Promise<WcRecipientEditor> {
  const el = document.createElement('wc-recipient-editor');
  Object.assign(el, { value: SIGNED, contacts: CONTACTS }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

async function emitted(
  el: WcRecipientEditor,
  interact: () => void,
): Promise<RecipientEditorValue | undefined> {
  const seen: RecipientEditorValue[] = [];
  el.addEventListener('nc-recipients-change', (event) =>
    seen.push((event as CustomEvent<NcRecipientsChangeDetail>).detail.value),
  );
  interact();
  await el.updateComplete;
  return seen.at(-1);
}

function all(el: WcRecipientEditor, hook: string): HTMLElement[] {
  return [...(el.shadowRoot?.querySelectorAll<HTMLElement>(hook) ?? [])];
}

describe('defaultRecipients', () => {
  it('makes the billing contact the signer and leaves collaborators empty', () => {
    expect(defaultRecipients(CONTACTS)).toEqual({
      signer: { name: 'Pat Example', email: 'pat@cedar.test' },
      collaborators: [],
    });
  });

  it('leaves the signer blank when no contact is the billing one', () => {
    expect(defaultRecipients([])).toEqual({
      signer: { name: '', email: '' },
      collaborators: [],
    });
  });
});

describe('validateRecipients', () => {
  it('accepts a named signer with distinct addresses', () => {
    expect(validateRecipients(SIGNED)).toEqual({});
  });

  it('refuses a signer without a name', () => {
    const errors = validateRecipients({ ...SIGNED, signer: { name: '  ', email: 'pat@cedar.test' } });
    expect(errors.signerName).toBeTruthy();
  });

  it('refuses a signer without an address', () => {
    const errors = validateRecipients({ ...SIGNED, signer: { name: 'Pat Example', email: '' } });
    expect(errors.signerEmail).toBeTruthy();
  });

  it('refuses a collaborator address with no @', () => {
    const errors = validateRecipients({
      ...SIGNED,
      collaborators: [...SIGNED.collaborators, { name: 'Legal', email: 'legal.cedar.test' }],
    });
    expect(Object.keys(errors.collaborators ?? {})).toEqual(['1']);
  });

  it('refuses the same address twice, ignoring case', () => {
    const errors = validateRecipients({
      ...SIGNED,
      collaborators: [{ name: 'Pat again', email: 'PAT@Cedar.test' }],
    });
    expect(errors.duplicate).toBeTruthy();
  });
});

describe('wc-recipient-editor', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders the signer and one row per collaborator', async () => {
    const el = await mount();
    expect(all(el, '[data-signer-name]')).toHaveLength(1);
    expect(all(el, '[data-signer-email]')).toHaveLength(1);
    expect(all(el, '[data-collaborator]')).toHaveLength(1);
  });

  it('offers only the contacts not already listed', async () => {
    const el = await mount();
    const labels = all(el, '[data-add-contact]').map((b) => b.textContent?.trim());
    expect(labels).toEqual(['Add legal@cedar.test']);
  });

  it('appends a picked contact as a collaborator', async () => {
    const el = await mount();
    const next = await emitted(el, () => all(el, '[data-add-contact]')[0].click());
    expect(next).toEqual({
      ...SIGNED,
      collaborators: [...SIGNED.collaborators, { name: '', email: 'legal@cedar.test' }],
    });
  });

  it('adds a blank collaborator to type in', async () => {
    const el = await mount();
    const next = await emitted(el, () => all(el, '[data-add-collaborator]')[0].click());
    expect(next?.collaborators).toEqual([...SIGNED.collaborators, { name: '', email: '' }]);
  });

  it('removes a collaborator', async () => {
    const el = await mount();
    const next = await emitted(el, () => all(el, '[data-remove-collaborator]')[0].click());
    expect(next).toEqual({ ...SIGNED, collaborators: [] });
  });

  it('never offers a control to remove the signer', async () => {
    const el = await mount({ value: { ...SIGNED, collaborators: [] } });
    expect(all(el, '[data-remove-collaborator]')).toHaveLength(0);
    const signer = el.shadowRoot?.querySelector('fieldset[data-signer]');
    expect(signer?.querySelector('wa-button')).toBeNull();
  });

  it('edits the signer without touching the collaborators', async () => {
    const el = await mount();
    const next = await emitted(el, () => {
      const input = all(el, '[data-signer-name]')[0] as HTMLInputElement;
      input.value = 'Pat Q. Example';
      input.dispatchEvent(new Event('input'));
    });
    expect(next).toEqual({ ...SIGNED, signer: { ...SIGNED.signer, name: 'Pat Q. Example' } });
  });

  it('edits one collaborator field', async () => {
    const el = await mount();
    const next = await emitted(el, () => {
      const input = all(el, '[data-collaborator-email]')[0] as HTMLInputElement;
      input.value = 'sam@juniper.test';
      input.dispatchEvent(new Event('input'));
    });
    expect(next?.collaborators).toEqual([{ name: 'Sam Example', email: 'sam@juniper.test' }]);
  });

  it('shows each error beside what it is about', async () => {
    const el = await mount({
      errors: {
        signerName: 'The signer needs a name',
        collaborators: { 0: 'Enter an email address' },
        duplicate: 'An address appears twice',
      },
    });
    const messages = all(el, '.error').map((p) => p.textContent?.trim());
    expect(messages).toEqual([
      'The signer needs a name',
      'Enter an email address',
      'An address appears twice',
    ]);
  });

  it('disables every control', async () => {
    const el = await mount({ disabled: true });
    const controls = all(el, 'wa-input, wa-button');
    expect(controls.length).toBeGreaterThan(4);
    expect(controls.every((c) => c.hasAttribute('disabled'))).toBe(true);
  });
});

describePreviewA11y(preview);

describeControlsAdoption(WcRecipientEditor);
