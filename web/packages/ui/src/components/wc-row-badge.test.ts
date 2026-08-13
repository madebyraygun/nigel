import { describe, it, expect, afterEach } from 'vitest';
import './wc-row-badge.js';
import type { WcRowBadge } from './wc-row-badge.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-row-badge.preview.js';

async function mount(label: string): Promise<WcRowBadge> {
  const el = document.createElement('wc-row-badge');
  el.label = label;
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-row-badge', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders its label as a word', async () => {
    const el = await mount('Archived');
    expect(el.shadowRoot?.querySelector('.badge')?.textContent).toBe('Archived');
  });

  it('renders nothing at all for an empty label', async () => {
    // A row without the state must not gain an empty pill.
    const el = await mount('   ');
    expect(el.shadowRoot?.querySelector('.badge')).toBeNull();
  });
});

describePreviewA11y(preview);
