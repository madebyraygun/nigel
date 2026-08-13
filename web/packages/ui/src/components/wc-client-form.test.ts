import { describe, it, expect, afterEach } from 'vitest';
import './wc-client-form.js';
import {
  EMPTY_CLIENT_FORM,
  validateClientForm,
  type ClientFormValue,
  type NcClientFormChangeDetail,
  WcClientForm,
} from './wc-client-form.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { describeControlsAdoption } from '../../preview/controls-suite.js';
import preview from './wc-client-form.preview.js';

const filled: ClientFormValue = {
  name: 'Acme Co',
  contacts: [{ email: 'ap@acme.test', name: 'Ada Payne', title: 'AP' }],
  billingIndex: 0,
  billingAddress: '1 Main St',
  notes: '',
};

const two: ClientFormValue = {
  ...filled,
  contacts: [
    { email: 'ap@acme.test', name: 'Ada Payne', title: 'AP' },
    { email: 'dana@acme.test', name: 'Dana', title: '' },
  ],
};

async function mount(props: Partial<WcClientForm> = {}): Promise<WcClientForm> {
  const el = document.createElement('wc-client-form');
  Object.assign(el, { value: EMPTY_CLIENT_FORM }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

/** The value the form emitted, from one interaction. */
async function emitted(
  el: WcClientForm,
  interact: () => void,
): Promise<ClientFormValue | undefined> {
  const seen: ClientFormValue[] = [];
  el.addEventListener('nc-client-form-change', (event) =>
    seen.push((event as CustomEvent<NcClientFormChangeDetail>).detail.value),
  );
  interact();
  await el.updateComplete;
  return seen.at(-1);
}

function rows(el: WcClientForm): Element[] {
  return [...(el.shadowRoot?.querySelectorAll('[data-contact]') ?? [])];
}

function button(el: WcClientForm, hook: string, index = 0): HTMLElement {
  const found = el.shadowRoot?.querySelectorAll<HTMLElement>(hook)[index];
  if (!found) throw new Error(`no ${hook} at ${index}`);
  return found;
}

describe('validateClientForm', () => {
  it('requires a name', () => {
    expect(validateClientForm({ ...filled, name: '   ' }).name).toBe('Name is required');
    expect(validateClientForm(filled).name).toBeUndefined();
  });

  it('does not shape-check an address', () => {
    // Deliberate, and the reason is recorded on the component: no surface in
    // Nigel shape-checks an email, so a form that did would refuse what the
    // rest accepts. What this test owns is the form's own answer.
    expect(
      validateClientForm({
        ...filled,
        contacts: [{ email: 'not-an-email', name: '', title: '' }],
      }),
    ).toEqual({});
  });

  it('accepts a client with no contacts at all', () => {
    expect(validateClientForm({ ...filled, contacts: [] })).toEqual({});
  });

  it('refuses a blank row and a duplicate address, by row', () => {
    expect(
      validateClientForm({
        ...filled,
        contacts: [{ email: '  ', name: '', title: '' }],
      }).contacts,
    ).toEqual({ 0: 'An email address is required' });

    // Case-insensitively, so the form and the server refuse the same pair.
    expect(
      validateClientForm({
        ...filled,
        contacts: [
          { email: 'ap@acme.test', name: '', title: '' },
          { email: 'AP@ACME.TEST', name: '', title: '' },
        ],
      }).contacts,
    ).toEqual({ 1: 'This address is already on the list' });
  });
});

describe('wc-client-form', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('collects the client fields and a row per contact', async () => {
    const el = await mount({ value: two });
    for (const hook of ['[data-name]', '[data-address]', '[data-notes]']) {
      expect(el.shadowRoot?.querySelector(hook), hook).toBeTruthy();
    }
    expect(rows(el)).toHaveLength(2);
  });

  it('emits the whole value on every edit', async () => {
    const el = await mount({ value: filled });
    const next = await emitted(el, () => {
      const input = el.shadowRoot?.querySelector<HTMLInputElement>('[data-address]');
      input!.value = '2 Elm St';
      input!.dispatchEvent(new Event('input'));
    });
    expect(next).toEqual({ ...filled, billingAddress: '2 Elm St' });
  });

  it('edits one contact without touching the others', async () => {
    const el = await mount({ value: two });
    const next = await emitted(el, () => {
      const input = el.shadowRoot?.querySelectorAll<HTMLInputElement>('[data-contact-name]')[1];
      input!.value = 'Dana Chen';
      input!.dispatchEvent(new Event('input'));
    });
    expect(next?.contacts[1].name).toBe('Dana Chen');
    expect(next?.contacts[0]).toEqual(two.contacts[0]);
  });

  it('adds and removes rows', async () => {
    const el = await mount({ value: filled });
    const added = await emitted(el, () => button(el, '[data-add-contact]').click());
    expect(added?.contacts).toHaveLength(2);
    expect(added?.contacts[1]).toEqual({ email: '', name: '', title: '' });

    const shrunk = await mount({ value: two });
    const removed = await emitted(shrunk, () =>
      button(shrunk, '[data-remove-contact]', 1).click(),
    );
    expect(removed?.contacts).toHaveLength(1);
    expect(removed?.contacts[0].email).toBe('ap@acme.test');
  });

  it('keeps the billing row pointing at the same contact when the list moves', async () => {
    const el = await mount({ value: { ...two, billingIndex: 1 } });

    // Removing the row above it: the flag follows the contact, not the index.
    const removed = await emitted(el, () => button(el, '[data-remove-contact]', 0).click());
    expect(removed?.billingIndex).toBe(0);
    expect(removed?.contacts[0].email).toBe('dana@acme.test');

    const moved = await mount({ value: { ...two, billingIndex: 1 } });
    const reordered = await emitted(moved, () => button(moved, '[data-move-up]', 1).click());
    expect(reordered?.contacts[0].email).toBe('dana@acme.test');
    expect(reordered?.billingIndex).toBe(0);
  });

  it('chooses the billing recipient with a radio, never a drag', async () => {
    const el = await mount({ value: two });
    const radios = el.shadowRoot?.querySelectorAll<HTMLInputElement>('[data-billing]');
    expect(radios).toHaveLength(2);
    expect(radios?.[0].checked).toBe(true);

    const next = await emitted(el, () => {
      radios![1].checked = true;
      radios![1].dispatchEvent(new Event('change'));
    });
    expect(next?.billingIndex).toBe(1);
  });

  it('says what a missing email costs, and stops saying it once one is typed', async () => {
    const empty = await mount();
    expect(empty.shadowRoot?.querySelector('[data-email-hint]')?.textContent).toContain(
      'cannot be sent',
    );

    const filledIn = await mount({ value: filled });
    expect(filledIn.shadowRoot?.querySelector('[data-email-hint]')).toBeNull();
  });

  it('renders a field error beside its field', async () => {
    const el = await mount({ errors: { name: 'Name is required' } });
    expect(el.shadowRoot?.querySelector('.error')?.textContent?.trim()).toBe(
      'Name is required',
    );
  });

  it('renders a contact error beside its row', async () => {
    const el = await mount({
      value: two,
      errors: { contacts: { 1: 'This address is already on the list' } },
    });
    expect(el.shadowRoot?.querySelector('.error')?.textContent?.trim()).toBe(
      'This address is already on the list',
    );
  });

  it('disables every control while a save is in flight', async () => {
    const el = await mount({ value: two, disabled: true });
    const controls = [
      ...(el.shadowRoot?.querySelectorAll(
        '[data-name],[data-address],[data-notes],[data-contact-email],[data-contact-name],[data-contact-title],[data-add-contact],[data-remove-contact],[data-billing]',
      ) ?? []),
    ];
    expect(controls.length).toBeGreaterThan(8);
    expect(controls.every((control) => control.hasAttribute('disabled'))).toBe(true);
  });
});

describePreviewA11y(preview);

describeControlsAdoption(WcClientForm, ':focus-visible');
