import { describe, it, expect } from 'vitest';
import { nigelTheme } from '../src/themes/nigel.js';
import { NIGEL_PALETTE } from '../src/tokens/gradient.js';
import { declarationsOf, followVar } from './token-resolution.js';

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
    '--nc-font-figures',
    '--nc-font-money',
    '--nc-font-brand',
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

  it('makes the system face the primary one', () => {
    // Chrome, labels and prose are drawn in whatever the person already reads
    // every menu and dialog in; that is most of what makes an app look like it
    // belongs on the machine rather than in a tab.
    expect(text).toMatch(/--wa-font-family-sans:\s*system-ui/);
    expect(text).toMatch(/--wa-font-family-sans:[^;]*sans-serif;/);
  });

  it('keeps the bundled face out of the primary stack', () => {
    // A stack that still named Plex would fall back to it on an engine that
    // does not answer system-ui, and chrome would be mono again on exactly
    // the platform the split is for.
    for (const declared of declarationsOf('--wa-font-family-sans')) {
      expect(declared).not.toContain('IBM Plex Mono');
    }
  });

  it('keeps a system mono behind the bundled one, so a missing face still aligns columns', () => {
    // Every fallback here is mono: if the bundled face fails to load, money
    // columns should still line up rather than fall to a proportional face.
    expect(text).toMatch(/--wa-font-family-mono:\s*'IBM Plex Mono'/);
    expect(text).toMatch(/--wa-font-family-mono:[^;]*ui-monospace/);
    expect(text).toMatch(/--wa-font-family-mono:[^;]*monospace;/);
  });

  it('lands every figures and brand token on the bundled face', () => {
    // Each is an indirection so a component can say why it wants the face
    // rather than which face it wants; following the chain is what proves the
    // indirection still arrives somewhere.
    for (const token of ['--nc-font-figures', '--nc-font-money', '--nc-font-brand']) {
      expect(followVar(token), token).toContain("'IBM Plex Mono'");
    }
  });

  it('answers the four family names Web Awesome actually reads', () => {
    // WA's compiled styles never mention --wa-font-family-sans. They read
    // these four as bare var()s, and an undefined custom property is not a
    // default but a discarded declaration.
    for (const [token, want] of [
      ['--wa-font-family-body', 'system-ui'],
      ['--wa-font-family-heading', 'system-ui'],
      ['--wa-font-family-longform', 'system-ui'],
      ['--wa-font-family-code', "'IBM Plex Mono'"],
    ]) {
      expect(followVar(token), token).toContain(want);
    }
  });

  it('prints in the bundled face, so two machines print one document', () => {
    // A system face is whatever the machine has. Paper is the artifact an
    // accountant keeps, and its metrics cannot depend on the OS it came off.
    const print = text.slice(text.indexOf('@media print'));
    expect(print).toMatch(/--wa-font-family-sans:\s*var\(--wa-font-family-mono\)/);
    expect(followVar('--wa-font-family-sans')).not.toContain('IBM Plex Mono');
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
