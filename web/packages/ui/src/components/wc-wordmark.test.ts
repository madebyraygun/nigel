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

/**
 * Specificity as one comparable number. `:host()` weighs as a pseudo-class
 * plus its argument, which is what makes `:host([animated]) .char` a (0,3,0)
 * that a bare `.char` (0,1,0) can never outrank.
 */
function specificity(selector: string): number {
  return (selector.match(/\.[\w-]+|\[[^\]]+\]|:[\w-]+/g) ?? []).length;
}

/** Whether a rule from the component's own sheet reaches the `.char` spans. */
function reachesChar(el: WcWordmark, selector: string): boolean {
  const host = selector.match(/^:host\((.+)\)\s+(.*)$/);
  if (host) return el.matches(host[1]) && host[2] === '.char';
  return selector === '.char';
}

/**
 * The `animation` a `.char` actually ends up with.
 *
 * jsdom neither cascades across a shadow boundary nor evaluates a media
 * query, so `getComputedStyle` answers nothing here and the cascade is walked
 * over the component's own rules instead.
 */
function winningAnimation(el: WcWordmark, systemReducedMotion = false): string {
  const probe = document.createElement('style');
  probe.textContent = el.shadowRoot?.querySelector('style')?.textContent ?? '';
  document.head.appendChild(probe);

  const rules: CSSStyleRule[] = [];
  for (const rule of [...(probe.sheet?.cssRules ?? [])]) {
    const media = rule as CSSMediaRule;
    if (media.conditionText !== undefined) {
      if (systemReducedMotion && media.conditionText.includes('prefers-reduced-motion: reduce')) {
        rules.push(...([...media.cssRules] as CSSStyleRule[]));
      }
    } else if ((rule as CSSStyleRule).selectorText !== undefined) {
      rules.push(rule as CSSStyleRule);
    }
  }
  probe.remove();

  let winner = '';
  let best = -1;
  rules.forEach((rule, order) => {
    if (!rule.style.animation || !reachesChar(el, rule.selectorText)) return;
    const weight = specificity(rule.selectorText) * 1000 + order;
    if (weight >= best) {
      best = weight;
      winner = rule.style.animation;
    }
  });
  return winner;
}

describe('wc-wordmark', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders one span per character of the art', async () => {
    const el = await mount();
    const width = Math.max(...WORDMARK_ART.map((line) => line.length));
    expect(chars(el)).toHaveLength(WORDMARK_ART.length * width);
  });

  it('gives every line one width, so a centred surface cannot shift the tail', async () => {
    // The descender rows are a third the width of the wordmark. Left short,
    // an inherited `text-align: center` slides them right and the `g` reads
    // as a `q` — which is what the setup screen was doing.
    const el = await mount();
    const lines = [...(el.shadowRoot?.querySelectorAll('.line') ?? [])];
    const widths = new Set(lines.map((line) => line.querySelectorAll('.char').length));

    expect(lines).toHaveLength(WORDMARK_ART.length);
    expect([...widths]).toEqual([Math.max(...WORDMARK_ART.map((line) => line.length))]);
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
    expect(winningAnimation(el)).toBe('none');
  });

  it('stands the animation down for a system that asked for less motion', async () => {
    // The media query is the safety net for a host that never learned about
    // the preference, and it only helps if it outweighs the rule it is there
    // to overrule.
    const el = await mount({ animated: true });
    expect(winningAnimation(el)).not.toBe('none');
    expect(winningAnimation(el, true)).toBe('none');
  });
});

describePreviewA11y(preview);
