import { describe, it, expect, afterEach } from 'vitest';
import { PASTEL_COUNT } from '@nigel/theme';
import './wc-monogram.js';
import { WcMonogram, monogramInitials, monogramTone } from './wc-monogram.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { styleText } from '../../preview/controls-suite.js';
import preview from './wc-monogram.preview.js';

describe('monogramInitials', () => {
  it.each([
    ['Acme Consulting LLC', 'AC'],
    ['Cedar Systems', 'CS'],
    ['Harbor & Vale', 'HV'],
    ['Globex', 'GL'],
    ['Initech, Inc.', 'IN'],
    ['juniper labs', 'JL'],
    ['', ''],
    ['  ', ''],
  ])('%s is %s', (name, initials) => {
    expect(monogramInitials(name)).toBe(initials);
  });
});

describe('monogramTone', () => {
  it('lands on one of the pastels', () => {
    for (const seed of ['acme', 'cedar', 'juniper', 'harbor', 'globex', 'initech', '']) {
      const tone = monogramTone(seed);
      expect(tone).toBeGreaterThanOrEqual(1);
      expect(tone).toBeLessThanOrEqual(PASTEL_COUNT);
    }
  });

  it('gives a seed the same colour every time', () => {
    expect(monogramTone('/books/acme')).toBe(monogramTone('/books/acme'));
  });

  it('spreads the cast across the ramp', () => {
    const tones = new Set(
      ['Acme', 'Cedar Systems', 'Juniper Labs', 'Harbor & Vale', 'Globex', 'Initech'].map(
        monogramTone,
      ),
    );
    expect(tones.size).toBeGreaterThan(2);
  });
});

describe('wc-monogram', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  async function mount(props: Partial<WcMonogram>): Promise<WcMonogram> {
    const el = document.createElement('wc-monogram');
    Object.assign(el, props);
    document.body.appendChild(el);
    await el.updateComplete;
    return el;
  }

  it('prints the initials, hidden from assistive tech', async () => {
    const el = await mount({ name: 'Acme Consulting LLC' });
    const tile = el.shadowRoot?.querySelector('.tile');
    expect(tile?.textContent).toBe('AC');
    expect(tile?.getAttribute('aria-hidden')).toBe('true');
  });

  it('takes its colour from the seed, not the name', async () => {
    const el = await mount({ name: 'Acme Consulting LLC', seed: '/books/acme' });
    expect(el.shadowRoot?.querySelector('.tile')?.classList).toContain(
      `tone-${monogramTone('/books/acme')}`,
    );
  });

  it('draws its text in the on-gradient ink on every pastel', () => {
    const text = styleText(WcMonogram);
    expect(text).toMatch(/\.tile\s*{[^}]*color:\s*var\(--nc-color-on-gradient\)/);
    for (let tone = 2; tone <= PASTEL_COUNT; tone += 1) {
      expect(text).toContain(`.tone-${tone}`);
      expect(text).toContain(`var(--nc-color-pastel-${tone})`);
    }
  });
});

describePreviewA11y(preview);
