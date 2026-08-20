import { describe, it, expect } from 'vitest';
import { nigelTheme } from '../src/themes/nigel.js';
import { NIGEL_PALETTE } from '../src/tokens/gradient.js';
import { declarationsOf } from './token-resolution.js';

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
    '--nc-grad-brand-size',
    '--nc-grad-brand-soft',
    '--nc-font-money',
    '--nc-icon-size',
    '--nc-sidebar-width',
    '--nc-sidebar-collapsed-width',
    '--nc-header-height',
    '--nc-transition-fast',
    '--nc-transition-base',
    '--nc-duration-slow',
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

  it('zeroes the durations a button animates over when motion is reduced', () => {
    // WA's own base-part transitions run at --wa-transition-fast; the hover
    // edge runs at --nc-duration-slow. Zeroing both is the whole of
    // prefers-reduced-motion for a button's chrome — the brand drift is an
    // animation rather than a transition and is turned off in controlsCss.
    expect(text).toMatch(/--wa-transition-fast:\s*var\(--nc-duration-fast\)/);
    const reduced = text.slice(text.indexOf('prefers-reduced-motion'));
    expect(reduced).toMatch(/--nc-duration-fast:\s*0ms/);
    expect(reduced).toMatch(/--nc-duration-slow:\s*0ms/);
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
 * The brand ramp as the button draws it: a periodic image plus the size that
 * makes one `background-position: 100%` shift exactly one period.
 *
 * The pair is the whole of the hover drift's seamlessness, and the pair is
 * also what keeps a button that never moves looking as it always did — so the
 * arithmetic is asserted rather than eyeballed.
 */
describe('the brand ramp', () => {
  /** Every value declared for a token, light declaration first. */
  function values(name: string): string[] {
    return [...text.matchAll(new RegExp(`${name}:([^;]*);`, 'g'))].map((m) => m[1]);
  }

  const image = values('--nc-grad-brand')[0];
  const stops = [...image.matchAll(/(#[0-9a-f]{6}) ([\d.]+)%/gi)].map((m) => ({
    color: m[1],
    at: Number(m[2]),
  }));
  const size = Number(values('--nc-grad-brand-size')[0].match(/([\d.]+)%/)![1]);

  it('repeats, which is what the drift scrolls through', () => {
    expect(image).toContain('repeating-linear-gradient');
  });

  it('carries the seven stops of the palette and then returns to the first', () => {
    // The wrap is what makes it periodic; magenta back to pink is a short hue
    // step, so it reads as part of the rainbow rather than as a join.
    expect(stops.map((s) => s.color)).toEqual([...NIGEL_PALETTE, NIGEL_PALETTE[0]]);
  });

  it('puts the ramp exactly across the element at rest, as the plain one did', () => {
    // A stop's position within the image, times the image's size, is where it
    // lands on the element. The last hue belongs at the right-hand edge.
    const magenta = stops[NIGEL_PALETTE.length - 1];
    expect((magenta.at * size) / 100).toBeCloseTo(100, 2);
    expect(stops[0].at).toBe(0);
  });

  it('shifts one whole period per iteration, so the loop has no seam', () => {
    // background-position: 100% offsets by image minus element. That has to
    // equal the period — the distance from a stop to its repeat.
    const period = stops[NIGEL_PALETTE.length].at * (size / 100);
    expect(size - 100).toBeCloseTo(period, 2);
  });

  it('is evenly spaced, so no hue is wider than its neighbours', () => {
    const steps = stops.slice(1).map((s, i) => s.at - stops[i].at);
    for (const step of steps) {
      expect(step).toBeCloseTo(steps[0], 3);
    }
  });

  it('declares a periodic wordmark ramp in both modes', () => {
    const declared = declarationsOf('--nc-grad-brand-text-cycle');
    expect(declared.length).toBeGreaterThanOrEqual(2);
    for (const value of declared) {
      expect(value).toContain('repeating-linear-gradient');
    }
  });

  it('closes the wordmark ramp on the colour it opened with', () => {
    // A ramp whose ends disagree shows a seam once per cycle, which is the
    // whole reason the periodic form exists.
    const [light] = declarationsOf('--nc-grad-brand-text-cycle');
    const stops = [...light.matchAll(/#[0-9a-f]{6}/gi)].map((m) => m[0].toLowerCase());
    expect(stops.at(0)).toBe(stops.at(-1));
  });
});
