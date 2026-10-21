import { describe, it, expect, afterEach } from 'vitest';
import './wc-appearance-picker.js';
import type { WcAppearancePicker } from './wc-appearance-picker.js';
import type { NcColorModeChangeDetail } from './wc-mode-switcher.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-appearance-picker.preview.js';

async function mount(mode: WcAppearancePicker['mode'] = 'system'): Promise<WcAppearancePicker> {
  const el = document.createElement('wc-appearance-picker');
  el.mode = mode;
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

const radios = (el: WcAppearancePicker) => [
  ...(el.shadowRoot?.querySelectorAll<HTMLInputElement>('input[type="radio"]') ?? []),
];

describe('wc-appearance-picker', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    document.documentElement.classList.remove('light-mode', 'dark-mode');
  });

  it('offers System, Light and Dark as one radio group', async () => {
    const el = await mount();
    expect(radios(el).map((r) => r.value)).toEqual(['system', 'light', 'dark']);
    expect(new Set(radios(el).map((r) => r.name)).size).toBe(1);
    expect(el.shadowRoot?.querySelector('legend')?.textContent).toBe('Appearance');
  });

  it('checks and highlights the current mode', async () => {
    const el = await mount('dark');
    expect(radios(el).map((r) => r.checked)).toEqual([false, false, true]);
    const selected = el.shadowRoot?.querySelector('.card.selected');
    expect(selected?.textContent).toContain('Dark');
  });

  it('splits the System preview between the two modes', async () => {
    const el = await mount();
    const system = el.shadowRoot?.querySelectorAll('.window')[0];
    expect(system?.querySelectorAll('.scene.light')).toHaveLength(1);
    expect(system?.querySelectorAll('.scene.dark.split')).toHaveLength(1);
  });

  it('asks for a mode and leaves the page alone', async () => {
    const el = await mount('system');
    const seen: string[] = [];
    el.addEventListener('nc-color-mode-change', (e) =>
      seen.push((e as CustomEvent<NcColorModeChangeDetail>).detail.mode),
    );
    const light = radios(el)[1];
    light.checked = true;
    light.dispatchEvent(new Event('change'));
    expect(seen).toEqual(['light']);
    expect(document.documentElement.classList.contains('light-mode')).toBe(false);
  });
});

describePreviewA11y(preview);
