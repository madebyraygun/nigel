import { describe, it, expect } from 'vitest';
import { nigelTheme } from '../src/themes/nigel.js';
import { NIGEL_PALETTE, NIGEL_PALETTE_INK } from '../src/tokens/gradient.js';

/**
 * The brand palette is pastel, which is exactly the failure mode this guards:
 * a pastel foreground on a white surface looks fine in a mockup and is
 * unreadable in use. Every foreground/background pairing the UI actually
 * renders is held to WCAG AA (4.5:1) in both modes.
 */

const AA_NORMAL = 4.5;
/** WCAG 1.4.11: non-text content — a chart bar, a swatch, an icon. */
const AA_GRAPHIC = 3;

function channel(hex: string): [number, number, number] {
  const v = hex.replace('#', '');
  return [
    parseInt(v.slice(0, 2), 16),
    parseInt(v.slice(2, 4), 16),
    parseInt(v.slice(4, 6), 16),
  ];
}

function relativeLuminance(hex: string): number {
  const srgb = channel(hex).map((c) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * srgb[0] + 0.7152 * srgb[1] + 0.0722 * srgb[2];
}

function contrast(a: string, b: string): number {
  const la = relativeLuminance(a);
  const lb = relativeLuminance(b);
  const [hi, lo] = la > lb ? [la, lb] : [lb, la];
  return (hi + 0.05) / (lo + 0.05);
}

/**
 * Pull a token's value out of the composed sheet. `occurrence` selects between
 * the light declaration (0) and the dark ones that follow it.
 */
function token(name: string, occurrence: number): string {
  const matches = [
    ...nigelTheme.cssText.matchAll(
      new RegExp(`${name}:\\s*(#[0-9a-fA-F]{6})`, 'g'),
    ),
  ].map((m) => m[1].toLowerCase());
  const value = matches[occurrence];
  expect(value, `${name} occurrence ${occurrence} not found`).toBeDefined();
  return value;
}

const light = (name: string) => token(name, 0);
const dark = (name: string) => token(name, 1);

describe('contrast helper', () => {
  it('computes the reference extremes', () => {
    expect(contrast('#000000', '#ffffff')).toBeCloseTo(21, 1);
    expect(contrast('#ffffff', '#ffffff')).toBeCloseTo(1, 5);
  });
});

describe.each([
  ['light', light],
  ['dark', dark],
])('%s mode meets WCAG AA', (_mode, t) => {
  it.each([
    ['text on bg', '--wa-color-text', '--wa-color-bg'],
    ['text on surface', '--wa-color-text', '--wa-color-surface'],
    ['text on surface-alt', '--wa-color-text', '--wa-color-surface-alt'],
    ['muted on bg', '--wa-color-muted', '--wa-color-bg'],
    ['muted on surface', '--wa-color-muted', '--wa-color-surface'],
    ['on-brand on brand', '--wa-color-on-brand', '--wa-color-brand'],
    ['brand on bg', '--wa-color-brand', '--wa-color-bg'],
    ['danger on surface', '--wa-color-danger', '--wa-color-surface'],
    ['success on surface', '--wa-color-success', '--wa-color-surface'],
    ['warning on surface', '--wa-color-warning', '--wa-color-surface'],
    ['info on surface', '--wa-color-info', '--wa-color-surface'],
    ['income on bg', '--nc-color-income', '--wa-color-bg'],
    ['income on surface', '--nc-color-income', '--wa-color-surface'],
    ['expense on bg', '--nc-color-expense', '--wa-color-bg'],
    ['expense on surface', '--nc-color-expense', '--wa-color-surface'],
    ['flagged on surface', '--nc-color-flagged', '--wa-color-surface'],
    ['text on selected row', '--wa-color-text', '--nc-color-selected-bg'],
    ['text on sidebar', '--wa-color-text', '--nc-color-sidebar-bg'],
    ['muted on sidebar', '--wa-color-muted', '--nc-color-sidebar-bg'],
    ['brand on sidebar', '--wa-color-brand', '--nc-color-sidebar-bg'],
  ])('%s', (_label, fg, bg) => {
    expect(contrast(t(fg), t(bg))).toBeGreaterThanOrEqual(AA_NORMAL);
  });

  /**
   * A chart bar is a large block rather than a glyph, so WCAG asks 3:1 of it
   * instead of 4.5:1. That headroom is the whole reason the fills are separate
   * tokens from the figures: it lets the bars sit much lighter on a white card
   * without touching the contrast of a number printed in the same hue.
   */
  it.each([
    ['income fill on surface', '--nc-color-income-fill', '--wa-color-surface'],
    ['expense fill on surface', '--nc-color-expense-fill', '--wa-color-surface'],
    ['income fill on bg', '--nc-color-income-fill', '--wa-color-bg'],
    ['expense fill on bg', '--nc-color-expense-fill', '--wa-color-bg'],
  ])('%s clears the graphic threshold', (_label, fg, bg) => {
    expect(contrast(t(fg), t(bg))).toBeGreaterThanOrEqual(AA_GRAPHIC);
  });
});

/**
 * The gradient is the one background that is not a token pair: it is seven
 * pastel stops, identical in both modes, and a primary button's label sits on
 * whichever one it happens to land on. So the label is checked against all of
 * them, and it is checked once rather than per mode — a foreground that flips
 * with the mode cannot clear a background that does not, which is why
 * `--nc-color-on-gradient` exists instead of reusing `--wa-color-text`.
 */
describe('the brand gradient carries readable text', () => {
  const onGradient = light('--nc-color-on-gradient');

  it('is declared once, with no dark override', () => {
    const declarations = [
      ...nigelTheme.cssText.matchAll(/--nc-color-on-gradient:\s*#[0-9a-fA-F]{6}/g),
    ];
    expect(declarations).toHaveLength(1);
  });

  it.each(NIGEL_PALETTE)('clears AA on the %s stop', (stop) => {
    expect(contrast(onGradient, stop)).toBeGreaterThanOrEqual(AA_NORMAL);
  });
});

/**
 * The wordmark is gradient-filled text, so every stop of the ramp is a
 * foreground colour and each one has to clear AA on its own. The pastels do
 * that on a dark surface and nowhere near it on a light one, which is why
 * light mode gets its own ramp.
 */
describe('the wordmark ramp is legible on the surface it is drawn on', () => {
  it.each(NIGEL_PALETTE_INK)('light: %s clears AA on the sidebar', (stop) => {
    expect(contrast(stop, light('--nc-color-sidebar-bg'))).toBeGreaterThanOrEqual(AA_NORMAL);
  });

  it.each(NIGEL_PALETTE)('dark: %s clears AA on the sidebar', (stop) => {
    expect(contrast(stop, dark('--nc-color-sidebar-bg'))).toBeGreaterThanOrEqual(AA_NORMAL);
  });

  it('keeps the two ramps the same length, one hue per stop', () => {
    expect(NIGEL_PALETTE_INK).toHaveLength(NIGEL_PALETTE.length);
  });

  it('leaves the shared palette alone, so effects.rs parity is untouched', () => {
    // palette-parity.test.ts pins NIGEL_PALETTE to GRADIENT in src/effects.rs.
    // The light ramp is additive precisely so that stays true.
    for (const stop of NIGEL_PALETTE_INK) {
      expect(NIGEL_PALETTE).not.toContain(stop);
    }
  });
});
