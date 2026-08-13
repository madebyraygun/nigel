import { describe, it, expect, afterEach } from 'vitest';
import './wc-mode-switcher.js';
import { WcModeSwitcher, type NcColorModeChangeDetail } from './wc-mode-switcher.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { describeControlsAdoption, styleText } from '../../preview/controls-suite.js';
import { describePrintHiding } from '../../preview/print-suite.js';
import preview from './wc-mode-switcher.preview.js';

async function mount(props: Partial<WcModeSwitcher> = {}): Promise<WcModeSwitcher> {
  const el = document.createElement('wc-mode-switcher') as WcModeSwitcher;
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function radios(el: WcModeSwitcher): Element[] {
  return [...(el.shadowRoot?.querySelectorAll('wa-radio') ?? [])];
}

describe('wc-mode-switcher', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders three radios labelled Light, Dark and System', async () => {
    const el = await mount();
    expect(radios(el).map((r) => r.getAttribute('value'))).toEqual(['light', 'dark', 'system']);
    expect(radios(el).map((r) => r.textContent?.trim())).toEqual(['Light', 'Dark', 'System']);
  });

  it('labels the group, so the radios are not three loose choices', async () => {
    const el = await mount();
    const group = el.shadowRoot?.querySelector('wa-radio-group');
    expect(group?.getAttribute('label')).toBe('Appearance');
  });

  it.each(['light', 'dark', 'system'] as const)('marks %s as the current mode', async (mode) => {
    const el = await mount({ mode });
    expect(el.shadowRoot?.querySelector('wa-radio-group')?.getAttribute('value')).toBe(mode);
  });

  it('emits nc-color-mode-change with the chosen mode', async () => {
    const el = await mount({ mode: 'system' });
    const seen: NcColorModeChangeDetail[] = [];
    el.addEventListener('nc-color-mode-change', (e) =>
      seen.push((e as CustomEvent<NcColorModeChangeDetail>).detail),
    );

    const group = el.shadowRoot?.querySelector('wa-radio-group') as HTMLElement & {
      value: string;
    };
    group.value = 'dark';
    group.dispatchEvent(new Event('change', { bubbles: true, composed: true }));

    expect(seen).toEqual([{ mode: 'dark' }]);
  });

  it('escapes its own shadow root, so the app can listen on the element', async () => {
    const el = await mount();
    let caught: Event | undefined;
    document.body.addEventListener('nc-color-mode-change', (e) => (caught = e));

    const group = el.shadowRoot?.querySelector('wa-radio-group') as HTMLElement & {
      value: string;
    };
    group.value = 'light';
    group.dispatchEvent(new Event('change', { bubbles: true, composed: true }));

    expect(caught).toBeDefined();
  });

  it('ignores a value that is not a mode', async () => {
    const el = await mount({ mode: 'system' });
    const seen: NcColorModeChangeDetail[] = [];
    el.addEventListener('nc-color-mode-change', (e) =>
      seen.push((e as CustomEvent<NcColorModeChangeDetail>).detail),
    );

    const group = el.shadowRoot?.querySelector('wa-radio-group') as HTMLElement & {
      value: string;
    };
    group.value = 'sepia';
    group.dispatchEvent(new Event('change', { bubbles: true, composed: true }));

    expect(seen).toEqual([]);
  });

  it('stays controlled — the property does not move on its own', async () => {
    // A component that wrote its own state would let the preview harness
    // change the real app's mode, and would give the container two sources of
    // truth for one preference.
    const el = await mount({ mode: 'system' });
    const group = el.shadowRoot?.querySelector('wa-radio-group') as HTMLElement & {
      value: string;
    };
    group.value = 'dark';
    group.dispatchEvent(new Event('change', { bubbles: true, composed: true }));
    await el.updateComplete;

    expect(el.mode).toBe('system');
  });

  it('never touches storage itself', () => {
    // The whole persistence story lives in the app container. Asserted on the
    // source because a spy would only prove this one path.
    expect(styleText(WcModeSwitcher)).not.toContain('localStorage');
    expect(WcModeSwitcher.prototype.render.toString()).not.toContain('localStorage');
  });

  it.each([
    ['light', 'dark', 'currently dark'],
    ['light', 'light', 'currently light'],
  ])('says what System resolves to (%s/%s)', async (_mode, resolved, expected) => {
    const el = await mount({ mode: 'system', resolved: resolved as 'light' | 'dark' });
    expect(el.shadowRoot?.textContent).toContain(expected);
  });

  it('drops the hint when the choice is explicit', async () => {
    // "System — currently dark" beside a selected Dark is noise: the user just
    // said what they wanted and the answer is on the radio.
    const el = await mount({ mode: 'dark', resolved: 'dark' });
    expect(el.shadowRoot?.textContent).not.toContain('currently');
  });
});

describePreviewA11y(preview);

describeControlsAdoption(WcModeSwitcher, ':focus-visible');

describePrintHiding(WcModeSwitcher, ':host');
