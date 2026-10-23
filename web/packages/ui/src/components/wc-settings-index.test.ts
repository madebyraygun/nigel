import { describe, it, expect, afterEach } from 'vitest';
import './wc-settings-index.js';
import type { WcSettingsIndex, NcSettingsSectionDetail } from './wc-settings-index.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-settings-index.preview.js';

async function mount(active = 'security'): Promise<WcSettingsIndex> {
  const el = document.createElement('wc-settings-index');
  el.groups = [
    {
      label: 'This book',
      items: [
        { id: 'letterhead', label: 'Letterhead' },
        { id: 'security', label: 'Security' },
      ],
    },
    { label: 'This device', items: [{ id: 'appearance', label: 'Appearance' }] },
  ];
  el.active = active;
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

const buttons = (el: WcSettingsIndex) => [
  ...(el.shadowRoot?.querySelectorAll<HTMLButtonElement>('button') ?? []),
];

describe('wc-settings-index', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('draws the page heading over the groups', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('h1')?.textContent).toBe('Settings');
    const labels = [...(el.shadowRoot?.querySelectorAll('.group-label') ?? [])].map(
      (l) => l.textContent,
    );
    expect(labels).toEqual(['This book', 'This device']);
  });

  it('marks only the active section as current', async () => {
    const el = await mount('security');
    expect(buttons(el).map((b) => b.getAttribute('aria-current'))).toEqual([
      'false',
      'true',
      'false',
    ]);
  });

  it('asks for a section without choosing it itself', async () => {
    const el = await mount('letterhead');
    const seen: string[] = [];
    el.addEventListener('nc-settings-section', (e) =>
      seen.push((e as CustomEvent<NcSettingsSectionDetail>).detail.id),
    );
    buttons(el)[2].click();
    await el.updateComplete;
    expect(seen).toEqual(['appearance']);
    expect(el.active).toBe('letterhead');
  });
});

describePreviewA11y(preview);
