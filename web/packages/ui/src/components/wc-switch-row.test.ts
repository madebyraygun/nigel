import { describe, it, expect, afterEach } from 'vitest';
import './wc-switch-row.js';
import type { WcSwitchRow, NcSwitchChangeDetail } from './wc-switch-row.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-switch-row.preview.js';

async function mount(props: Partial<WcSwitchRow> = {}): Promise<WcSwitchRow> {
  const el = document.createElement('wc-switch-row');
  Object.assign(el, { label: 'Check for updates automatically' }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

const control = (el: WcSwitchRow) =>
  el.shadowRoot?.querySelector('wa-switch') as HTMLElement & { checked: boolean };

function flip(el: WcSwitchRow, to: boolean): void {
  control(el).checked = to;
  control(el).dispatchEvent(new Event('change'));
}

describe('wc-switch-row', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('asks for the flipped value', async () => {
    const el = await mount({ checked: true });
    const seen: boolean[] = [];
    el.addEventListener('nc-switch-change', (e) =>
      seen.push((e as CustomEvent<NcSwitchChangeDetail>).detail.checked),
    );
    flip(el, false);
    expect(seen).toEqual([false]);
  });

  it('stays where its owner has it until the owner moves it', async () => {
    // A write that failed leaves `checked` alone, and the switch with it.
    const el = await mount({ checked: true });
    flip(el, false);
    await el.updateComplete;
    expect(control(el).checked).toBe(true);

    el.checked = false;
    await el.updateComplete;
    expect(control(el).checked).toBe(false);
  });

  it('names the switch by its label', async () => {
    const el = await mount();
    expect(control(el).textContent).toContain('Check for updates automatically');
  });
});

describePreviewA11y(preview);
