import { describe, it, expect, afterEach, vi } from 'vitest';
import './wc-choice-card.js';
import type { WcChoiceCard } from './wc-choice-card.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-choice-card.preview.js';

async function mount(props: Partial<WcChoiceCard> = {}): Promise<WcChoiceCard> {
  const el = document.createElement('wc-choice-card');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-choice-card', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('draws an interactive card as one button that clicks through to the host', async () => {
    const el = await mount({ interactive: true, heading: 'A business' });
    const spy = vi.fn();
    el.addEventListener('click', spy);
    el.shadowRoot?.querySelector('button')?.click();
    expect(spy).toHaveBeenCalledOnce();
  });

  it('is a plain container otherwise', async () => {
    const el = await mount({ heading: 'Start from scratch' });
    expect(el.shadowRoot?.querySelector('button')).toBeNull();
    expect(el.shadowRoot?.querySelector('.title')?.textContent).toBe('Start from scratch');
  });

  it('omits the description line when there is none', async () => {
    const el = await mount({ heading: 'Only a title' });
    expect(el.shadowRoot?.querySelector('.description')).toBeNull();
  });
});

describePreviewA11y(preview);
