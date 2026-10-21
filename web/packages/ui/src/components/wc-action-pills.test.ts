import { describe, it, expect, afterEach } from 'vitest';
import './wc-action-pills.js';
import type { ActionPill, NcActionDetail, WcActionPills } from './wc-action-pills.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-action-pills.preview.js';

const ACTIONS: ActionPill[] = [
  { key: 'send', label: 'Send…', tone: 'primary' },
  { key: 'edit', label: 'Edit', disabled: true, reason: 'A paid invoice cannot be edited.' },
  { key: 'void', label: 'Void…', tone: 'danger', disabled: true },
  { key: 'new', label: 'New invoice', icon: 'plus', hint: 'Start a draft' },
];

async function mount(actions = ACTIONS): Promise<WcActionPills> {
  const el = document.createElement('wc-action-pills');
  el.actions = actions;
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function button(el: WcActionPills, key: string): HTMLButtonElement {
  const found = el.shadowRoot?.querySelector<HTMLButtonElement>(`[data-action="${key}"]`);
  if (!found) throw new Error(`no ${key}`);
  return found;
}

describe('wc-action-pills', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('draws one button per action, in order, with its tone', async () => {
    const el = await mount();
    const buttons = [...(el.shadowRoot?.querySelectorAll('button') ?? [])];
    expect(buttons.map((b) => b.textContent?.trim())).toEqual([
      'Send…',
      'Edit',
      'Void…',
      'New invoice',
    ]);
    expect(buttons.map((b) => b.dataset.tone)).toEqual(['primary', 'default', 'danger', 'default']);
  });

  it('reports a click as nc-action with the key', async () => {
    const el = await mount();
    const seen: string[] = [];
    el.addEventListener('nc-action', (event) =>
      seen.push((event as CustomEvent<NcActionDetail>).detail.key),
    );
    button(el, 'send').click();
    expect(seen).toEqual(['send']);
  });

  it('says why a disabled action is off, and ties the sentence to the button', async () => {
    const el = await mount();
    const edit = button(el, 'edit');
    expect(edit.disabled).toBe(true);
    const id = edit.getAttribute('aria-describedby');
    expect(id).toBeTruthy();
    expect(el.shadowRoot?.getElementById(id ?? '')?.textContent?.trim()).toBe(
      'A paid invoice cannot be edited.',
    );
    expect(edit.title).toBe('A paid invoice cannot be edited.');
  });

  it('says a shared reason once and points every action it explains at it', async () => {
    const reason = 'A void invoice cannot be sent, paid, edited or voided again.';
    const el = await mount([
      { key: 'send', label: 'Send…', disabled: true, reason },
      { key: 'pay', label: 'Record payment…', disabled: true, reason },
      { key: 'duplicate', label: 'Duplicate' },
    ]);
    const notes = [...(el.shadowRoot?.querySelectorAll('[data-note]') ?? [])];
    expect(notes).toHaveLength(1);
    expect(notes[0].getAttribute('data-note')).toBe('send pay');
    expect(button(el, 'send').getAttribute('aria-describedby')).toBe(notes[0].id);
    expect(button(el, 'pay').getAttribute('aria-describedby')).toBe(notes[0].id);
  });

  it('prints nothing for a disabled action with no reason', async () => {
    const el = await mount();
    expect(button(el, 'void').hasAttribute('aria-describedby')).toBe(false);
    expect(el.shadowRoot?.querySelectorAll('[data-note]')).toHaveLength(1);
  });

  it('offers an enabled action’s hint as its tooltip', async () => {
    const el = await mount();
    expect(button(el, 'new').title).toBe('Start a draft');
    expect(button(el, 'new').querySelector('wc-icon-plus')).toBeTruthy();
  });

  it('prints no notes list when nothing is explained', async () => {
    const el = await mount([{ key: 'send', label: 'Send…' }]);
    expect(el.shadowRoot?.querySelector('.notes')).toBeNull();
  });
});

describePreviewA11y(preview);
