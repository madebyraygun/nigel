import { describe, it, expect, afterEach } from 'vitest';
import './wc-settings-scope.js';
import type { WcSettingsScope } from './wc-settings-scope.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-settings-scope.preview.js';

async function mount(props: Partial<WcSettingsScope>): Promise<WcSettingsScope> {
  const el = document.createElement('wc-settings-scope');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-settings-scope', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it("shows the book's monogram and name", async () => {
    const el = await mount({ kind: 'book', label: 'This book', heading: 'Acme Consulting LLC' });
    const monogram = el.shadowRoot?.querySelector('wc-monogram');
    expect(monogram?.getAttribute('name')).toBe('Acme Consulting LLC');
    expect(el.shadowRoot?.querySelector('h2')?.textContent).toContain('Acme Consulting LLC');
  });

  it('shows a monitor tile for the device', async () => {
    const el = await mount({ kind: 'device', label: 'This device', heading: 'Every book' });
    expect(el.shadowRoot?.querySelector('wc-monogram')).toBeNull();
    expect(el.shadowRoot?.querySelector('wc-icon-monitor')).toBeTruthy();
  });
});

describePreviewA11y(preview);
