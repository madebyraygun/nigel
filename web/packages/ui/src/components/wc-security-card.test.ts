import { describe, it, expect, afterEach } from 'vitest';
import './wc-security-card.js';
import type { WcSecurityCard, NcSecurityPanelDetail } from './wc-security-card.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-security-card.preview.js';

async function mount(encrypted: boolean): Promise<WcSecurityCard> {
  const el = document.createElement('wc-security-card');
  el.encrypted = encrypted;
  for (const slot of encrypted ? ['change', 'remove'] : ['set']) {
    const child = document.createElement('div');
    child.slot = slot;
    child.textContent = `${slot} form`;
    el.appendChild(child);
  }
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

const rowButton = (el: WcSecurityCard, index: number) =>
  el.shadowRoot?.querySelectorAll('.row wa-button')[index] as HTMLElement;

const panel = (el: WcSecurityCard, which: string) =>
  el.shadowRoot?.getElementById(`${which}-panel`) as HTMLElement;

describe('wc-security-card', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('warns about a plaintext database and offers to encrypt it', async () => {
    const el = await mount(false);
    expect(el.shadowRoot?.querySelector('.title')?.textContent).toBe('Not encrypted');
    expect(el.shadowRoot?.querySelector('wc-icon-lock-open')).toBeTruthy();
    expect(el.shadowRoot?.querySelector('slot[name="set"]')).toBeTruthy();
    expect(el.shadowRoot?.querySelector('.row')).toBeNull();
  });

  it('offers change and remove on an encrypted database, both closed', async () => {
    const el = await mount(true);
    expect(el.shadowRoot?.querySelector('.title')?.textContent).toBe('Encrypted');
    expect(panel(el, 'change').hidden).toBe(true);
    expect(panel(el, 'remove').hidden).toBe(true);
    expect(rowButton(el, 0).textContent).toContain('Change…');
    expect(rowButton(el, 1).textContent).toContain('Remove…');
  });

  it('opens one panel at a time and says so', async () => {
    const el = await mount(true);
    const seen: Array<string | null> = [];
    el.addEventListener('nc-security-panel', (e) =>
      seen.push((e as CustomEvent<NcSecurityPanelDetail>).detail.panel),
    );

    rowButton(el, 0).click();
    await el.updateComplete;
    expect(panel(el, 'change').hidden).toBe(false);
    expect(rowButton(el, 0).getAttribute('aria-expanded')).toBe('true');
    expect(rowButton(el, 0).textContent).toContain('Cancel');

    rowButton(el, 1).click();
    await el.updateComplete;
    expect(panel(el, 'change').hidden).toBe(true);
    expect(panel(el, 'remove').hidden).toBe(false);

    rowButton(el, 1).click();
    await el.updateComplete;
    expect(panel(el, 'remove').hidden).toBe(true);
    expect(seen).toEqual(['change', 'remove', null]);
  });

  it('closes when its owner says so', async () => {
    const el = await mount(true);
    rowButton(el, 0).click();
    await el.updateComplete;
    el.panel = null;
    await el.updateComplete;
    expect(panel(el, 'change').hidden).toBe(true);
  });
});

describePreviewA11y(preview);
