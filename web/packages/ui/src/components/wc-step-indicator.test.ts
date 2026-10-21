import { describe, it, expect, afterEach } from 'vitest';
import './wc-step-indicator.js';
import type { WcStepIndicator } from './wc-step-indicator.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-step-indicator.preview.js';

async function mount(props: Partial<WcStepIndicator> = {}): Promise<WcStepIndicator> {
  const el = document.createElement('wc-step-indicator');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

const pills = (el: WcStepIndicator) => [...(el.shadowRoot?.querySelectorAll('.pill') ?? [])];

describe('wc-step-indicator', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('marks completed and current pills, and widens only the current one', async () => {
    const el = await mount({ current: 2 });
    expect(pills(el).map((p) => p.className)).toEqual([
      'pill done',
      'pill done',
      'pill done current',
      'pill',
    ]);
  });

  it('is announced as a single position', async () => {
    const el = await mount({ current: 1, total: 4 });
    expect(el.shadowRoot?.querySelector('[role="img"]')?.getAttribute('aria-label')).toBe(
      'Step 2 of 4',
    );
  });
});

describePreviewA11y(preview);
