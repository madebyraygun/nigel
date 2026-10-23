import { describe, it, expect, afterEach } from 'vitest';
import './wc-folder-path.js';
import type { WcFolderPath } from './wc-folder-path.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-folder-path.preview.js';

async function mount(withAction: boolean): Promise<WcFolderPath> {
  const el = document.createElement('wc-folder-path');
  el.path = '/tmp/acme-consulting';
  if (withAction) el.appendChild(document.createElement('button'));
  document.body.appendChild(el);
  await el.updateComplete;
  await new Promise((r) => setTimeout(r, 0));
  await el.updateComplete;
  return el;
}

describe('wc-folder-path', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('shows the path as code', async () => {
    const el = await mount(false);
    expect(el.shadowRoot?.querySelector('code')?.textContent).toBe('/tmp/acme-consulting');
  });

  it('draws the action row only when there is an action', async () => {
    expect((await mount(false)).shadowRoot?.querySelector('.actions.empty')).toBeTruthy();
    expect((await mount(true)).shadowRoot?.querySelector('.actions.empty')).toBeNull();
  });
});

describePreviewA11y(preview);
