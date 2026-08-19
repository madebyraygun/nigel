import { describe, it, expect, afterEach } from 'vitest';
import './wc-wordmark.js';
import { WORDMARK_ART, type WcWordmark } from './wc-wordmark.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-wordmark.preview.js';

async function mount(props: Partial<WcWordmark> = {}): Promise<WcWordmark> {
  const el = document.createElement('wc-wordmark');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

const chars = (el: WcWordmark) => [...(el.shadowRoot?.querySelectorAll('.char') ?? [])];

describe('wc-wordmark', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders one span per character of the art', async () => {
    const el = await mount();
    const expected = WORDMARK_ART.reduce((total, line) => total + line.length, 0);
    expect(chars(el)).toHaveLength(expected);
  });

  it('names itself for a screen reader instead of reading the ascii aloud', async () => {
    const el = await mount();
    const art = el.shadowRoot?.querySelector('.art');
    expect(art?.getAttribute('role')).toBe('img');
    expect(art?.getAttribute('aria-label')).toBe('Nigel');
  });

  it('takes a custom accessible name', async () => {
    const el = await mount({ label: 'Nigel — bookkeeping' });
    expect(el.shadowRoot?.querySelector('.art')?.getAttribute('aria-label')).toBe(
      'Nigel — bookkeeping',
    );
  });

  it('staggers each character so one ramp sweeps across the whole mark', async () => {
    const el = await mount({ animated: true });
    const delays = chars(el).map((c) => (c as HTMLElement).style.animationDelay);
    expect(new Set(delays).size).toBeGreaterThan(1);
    expect(delays.every((d) => d.startsWith('-') && d.endsWith('s'))).toBe(true);
  });

  it('shows everything at a full reveal', async () => {
    const el = await mount({ reveal: 1 });
    const hidden = chars(el).filter((c) => c.classList.contains('hidden'));
    expect(hidden).toHaveLength(0);
  });

  it('shows nothing at a zero reveal, and keeps the space it will take', async () => {
    // Hidden rather than absent: a wordmark that grows into place shoves the
    // form underneath it down mid-animation.
    const el = await mount({ reveal: 0 });
    const drawn = chars(el).filter(
      (c) => !c.classList.contains('hidden') && c.textContent?.trim(),
    );
    expect(drawn).toHaveLength(0);
    expect(chars(el).length).toBeGreaterThan(0);
  });

  it('reveals more characters as the reveal advances', async () => {
    const el = await mount({ reveal: 0.5 });
    const half = chars(el).filter((c) => !c.classList.contains('hidden')).length;
    el.reveal = 0.9;
    await el.updateComplete;
    const most = chars(el).filter((c) => !c.classList.contains('hidden')).length;
    expect(most).toBeGreaterThan(half);
  });

  it('reveals in a stable order as the fraction climbs', async () => {
    // The order is shuffled once per instance; a reshuffle per render would
    // make characters blink out again as the reveal advances.
    const el = await mount({ reveal: 0.4 });
    const shown = chars(el).map((c) => !c.classList.contains('hidden'));
    el.reveal = 0.8;
    await el.updateComplete;
    const later = chars(el).map((c) => !c.classList.contains('hidden'));
    shown.forEach((was, i) => {
      if (was) expect(later[i], `character ${i} went back into hiding`).toBe(true);
    });
  });

  it('does not animate when motion is unwelcome', async () => {
    const el = await mount({ animated: true, reducedMotion: true });
    expect(el.hasAttribute('reduced-motion')).toBe(true);
  });
});

describePreviewA11y(preview);
