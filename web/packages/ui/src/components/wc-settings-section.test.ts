import { describe, it, expect, afterEach } from 'vitest';
import './wc-settings-section.js';
import type { WcSettingsSection } from './wc-settings-section.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-settings-section.preview.js';

async function mount(props: Partial<WcSettingsSection>): Promise<WcSettingsSection> {
  const el = document.createElement('wc-settings-section');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-settings-section', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('names the section by its heading', async () => {
    const el = await mount({ heading: 'Location', description: 'Where the books live.' });
    const section = el.shadowRoot?.querySelector('section');
    const heading = el.shadowRoot?.getElementById(section?.getAttribute('aria-labelledby') ?? '');
    expect(heading?.textContent).toBe('Location');
    expect(el.shadowRoot?.querySelector('.description')?.textContent).toBe(
      'Where the books live.',
    );
  });

  it('keeps an empty status region until there is something to announce', async () => {
    const el = await mount({ heading: 'Letterhead' });
    const status = el.shadowRoot?.querySelector('[role="status"]');
    expect(status?.textContent).toBe('');

    el.saved = true;
    await el.updateComplete;
    expect(el.shadowRoot?.querySelector('[role="status"]')).toBe(status);
    expect(status?.textContent).toBe('✓ Saved');
  });
});

describePreviewA11y(preview);
