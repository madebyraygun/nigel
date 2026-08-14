import { describe, it, expect } from 'vitest';
import { nigelTheme } from '../src/themes/nigel.js';
import { NIGEL_PALETTE, NIGEL_PALETTE_INK } from '../src/tokens/gradient.js';

const text = nigelTheme.cssText;

describe('nigelTheme', () => {
  it('exposes a Lit CSSResult with the composed token sheet', () => {
    expect(nigelTheme).toBeDefined();
    expect(typeof text).toBe('string');
    expect(text.length).toBeGreaterThan(0);
  });

  it.each([
    '--wa-color-bg',
    '--wa-color-surface',
    '--wa-color-surface-alt',
    '--wa-color-border',
    '--wa-color-border-soft',
    '--wa-color-text',
    '--wa-color-muted',
    '--wa-color-brand',
    '--wa-color-brand-hover',
    '--wa-color-on-brand',
    '--wa-color-focus',
    '--wa-color-danger',
    '--wa-color-success',
    '--wa-color-warning',
    '--wa-color-info',
    '--wa-font-family-sans',
    '--wa-font-family-mono',
    '--wa-font-size-base',
    '--wa-line-height',
    '--wa-space-m',
    '--wa-radius-md',
    '--wa-shadow-sm',
  ])('defines the Web Awesome token %s', (token) => {
    expect(text).toContain(`${token}:`);
  });

  it.each([
    '--nc-color-income',
    '--nc-color-expense',
    '--nc-color-flagged',
    '--nc-color-selected-bg',
    '--nc-grad-brand',
    '--nc-grad-brand-soft',
    '--nc-font-money',
    '--nc-icon-size',
    '--nc-sidebar-width',
    '--nc-sidebar-collapsed-width',
    '--nc-header-height',
    '--nc-transition-fast',
    '--nc-transition-base',
    '--nc-glow-brand',
    '--nc-glow-neutral',
    '--nc-glow-danger',
    '--nc-glow-success',
    '--nc-glow-warning',
  ])('defines the nigel token %s', (token) => {
    expect(text).toContain(`${token}:`);
  });

  it('makes the bundled mono the primary face', () => {
    expect(text).toMatch(/--wa-font-family-sans:\s*'IBM Plex Mono'/);
  });

  it('keeps a system mono behind it, so a missing face still aligns columns', () => {
    // The fallback is deliberately mono rather than the old sans stack: if the
    // bundled face fails, money columns should still line up.
    expect(text).toMatch(/--wa-font-family-sans:[^;]*ui-monospace/);
    expect(text).toMatch(/--wa-font-family-sans:[^;]*monospace;/);
  });

  it('pins color-scheme when a mode is forced, so native widgets follow', () => {
    // :root declares `color-scheme: light dark`, which lets the UA pick
    // scrollbars, date pickers and form-control defaults from the OS. An
    // explicit choice has to pin it, or the app is light with dark scrollbars.
    expect(text).toMatch(/:root\.light-mode\s*{[^}]*color-scheme:\s*light/);
    expect(text).toMatch(/:root\.dark-mode\s*{[^}]*color-scheme:\s*dark/);
  });

  it('supports system dark mode and both explicit overrides', () => {
    expect(text).toMatch(/prefers-color-scheme:\s*dark/);
    expect(text).toContain('.dark-mode');
    expect(text).toContain('.light-mode');
  });

  it('honours a reduced-motion preference', () => {
    expect(text).toMatch(/prefers-reduced-motion:\s*reduce/);
  });

  it('zeroes the duration a button transitions its glow over when motion is reduced', () => {
    // The glow rides wa-button's own base-part transition, whose duration is
    // --wa-transition-fast. That is the whole of prefers-reduced-motion for it.
    expect(text).toMatch(/--wa-transition-fast:\s*var\(--nc-duration-fast\)/);
    const reduced = text.slice(text.indexOf('prefers-reduced-motion'));
    expect(reduced).toMatch(/--nc-duration-fast:\s*0ms/);
  });

  it('orders light tokens before dark overrides before print', () => {
    // Specificity alone does not settle this: the dark block and the light
    // block both target :root, so the later one wins. Order is the contract.
    const light = text.indexOf('--wa-color-bg: #f3f2f7');
    const dark = text.indexOf('.dark-mode');
    const print = text.indexOf('@media print');
    expect(light).toBeGreaterThan(-1);
    expect(light).toBeLessThan(dark);
    expect(dark).toBeLessThan(print);
  });
});

/**
 * The button glow is the brand ramp, mixed down to a halo. Light mode draws
 * the ink stops and dark mode the pastels — `--nc-grad-brand-text`'s split,
 * for its reason: a pastel glow is invisible on a near-white surface and the
 * ink hues are a smudge on a dark one.
 */
describe('the button glow', () => {
  const LAVENDER = 5;
  const MAGENTA = 6;

  /** Every value declared for a token, light declaration first. */
  function values(name: string): string[] {
    return [...text.matchAll(new RegExp(`${name}:([^;]*);`, 'g'))].map((m) => m[1]);
  }

  it('declares the light value once and the dark one in both override blocks', () => {
    // Dark tokens are interpolated twice — the media query and .dark-mode.
    expect(values('--nc-glow-brand')).toHaveLength(3);
    expect(values('--nc-glow-neutral')).toHaveLength(3);
  });

  it.each(['--nc-glow-brand', '--nc-glow-neutral'])(
    'mixes %s from the ink ramp in light mode and the pastels in dark',
    (name) => {
      const [lightValue, ...darkValues] = values(name);
      expect(lightValue).toContain(NIGEL_PALETTE_INK[LAVENDER]);
      for (const value of darkValues) {
        expect(value).toContain(NIGEL_PALETTE[LAVENDER]);
      }
    },
  );

  it('spreads the magenta end of the ramp under the brand halo', () => {
    const [lightValue, ...darkValues] = values('--nc-glow-brand');
    expect(lightValue).toContain(NIGEL_PALETTE_INK[MAGENTA]);
    for (const value of darkValues) {
      expect(value).toContain(NIGEL_PALETTE[MAGENTA]);
    }
  });

  /**
   * A semantic button is already drawn in its own colour, and that colour has
   * a dark override of its own — so one declaration reading the token serves
   * both modes, where the ramp (which is not a token) needs two.
   */
  it.each(['danger', 'success', 'warning'])(
    'mixes the %s halo from the colour the button is drawn in',
    (name) => {
      const declared = values(`--nc-glow-${name}`);
      expect(declared).toHaveLength(1);
      expect(declared[0]).toContain(`var(--wa-color-${name})`);
    },
  );

  it('stays a glow rather than a neon edge', () => {
    // Subtlety is the whole brief, and it lives in these percentages.
    const mixes = [
      '--nc-glow-brand',
      '--nc-glow-neutral',
      '--nc-glow-danger',
      '--nc-glow-success',
      '--nc-glow-warning',
    ]
      .flatMap(values)
      .flatMap((value) => [...value.matchAll(/(\d+)%/g)])
      .map((m) => Number(m[1]));

    expect(mixes.length).toBeGreaterThan(0);
    for (const mix of mixes) {
      expect(mix).toBeLessThanOrEqual(40);
    }
  });
});
