import { describe, it, expect, afterEach } from 'vitest';
import './wc-save-bar.js';
import type { WcSaveBar } from './wc-save-bar.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-save-bar.preview.js';

async function mount(props: Partial<WcSaveBar> = {}): Promise<WcSaveBar> {
  const el = document.createElement('wc-save-bar');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

const button = (el: WcSaveBar, label: string) =>
  [...(el.shadowRoot?.querySelectorAll('wa-button') ?? [])].find((b) =>
    b.textContent?.includes(label),
  ) as HTMLElement;

describe('wc-save-bar', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('says what is unsaved', async () => {
    const el = await mount({ message: 'Unsaved changes to the letterhead' });
    expect(el.shadowRoot?.querySelector('[role="status"]')?.textContent).toBe(
      'Unsaved changes to the letterhead',
    );
  });

  it('asks its owner to discard or save', async () => {
    const el = await mount();
    const seen: string[] = [];
    el.addEventListener('nc-save-bar-discard', () => seen.push('discard'));
    el.addEventListener('nc-save-bar-save', () => seen.push('save'));

    button(el, 'Discard').click();
    button(el, 'Save').click();

    expect(seen).toEqual(['discard', 'save']);
  });

  it('holds both buttons while a save is in flight', async () => {
    const el = await mount({ busy: true });
    expect(button(el, 'Discard').hasAttribute('disabled')).toBe(true);
    expect(button(el, 'Saving').hasAttribute('disabled')).toBe(true);
  });
});

describePreviewA11y(preview);
