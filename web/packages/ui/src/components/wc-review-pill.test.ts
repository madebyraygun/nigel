import { describe, it, expect, afterEach } from 'vitest';
import './wc-review-pill.js';
import type { WcReviewPill } from './wc-review-pill.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-review-pill.preview.js';

async function mount(props: Partial<WcReviewPill> = {}): Promise<WcReviewPill> {
  const el = document.createElement('wc-review-pill');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-review-pill', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('shows the count and the words after it', async () => {
    const el = await mount({ count: 84 });
    expect(el.shadowRoot?.querySelector('.count')?.textContent).toBe('84');
    expect(el.shadowRoot?.querySelector('a')?.textContent).toContain('to review');
  });

  it('links to the review screen', async () => {
    const el = await mount({ count: 3 });
    expect(el.shadowRoot?.querySelector('a')?.getAttribute('href')).toBe('#/review');
  });
});

describePreviewA11y(preview);
