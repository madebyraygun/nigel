import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { NIGEL_PALETTE, gradientColor } from '../src/tokens/gradient.js';

/**
 * `gradientColor` is a port of `effects::gradient_color`, and the two are used
 * to paint the same things — the wordmark ramp, the Snake body. Parity is
 * asserted the way `palette-parity.test.ts` asserts the palette's: against the
 * Rust source itself, so a change to the interpolation on either side shows up
 * here rather than as a browser that cycles slightly differently.
 */
const here = dirname(fileURLToPath(import.meta.url));
const effectsRs = resolve(here, '../../../../src/effects.rs');

/** The literal `gradient_color` body, for the assertions that read it. */
function rustGradientFn(): string {
  const source = readFileSync(effectsRs, 'utf8');
  const start = source.indexOf('pub fn gradient_color');
  expect(start, 'gradient_color not found in src/effects.rs').toBeGreaterThan(-1);
  return source.slice(start, source.indexOf('\n}', start));
}

const channels = (hex: string) => [
  parseInt(hex.slice(1, 3), 16),
  parseInt(hex.slice(3, 5), 16),
  parseInt(hex.slice(5, 7), 16),
];

describe('gradientColor', () => {
  /**
   * Within a unit per channel rather than exactly. `2/7 * 7` is 1.9999…, and
   * truncating that lands the interpolation one step short of the stop — which
   * `as u8` in `effects.rs` does identically, so rounding here to make the
   * assertion pretty would be the browser departing from the terminal.
   */
  it('lands on a stop at that stop position', () => {
    NIGEL_PALETTE.forEach((stop, i) => {
      const got = channels(gradientColor(i / NIGEL_PALETTE.length));
      channels(stop).forEach((want, c) => {
        expect(Math.abs(got[c] - want), `${stop} channel ${c}`).toBeLessThanOrEqual(1);
      });
    });
  });

  it('closes the loop back to the first stop', () => {
    expect(gradientColor(1)).toBe(NIGEL_PALETTE[0]);
    expect(gradientColor(0)).toBe(NIGEL_PALETTE[0]);
  });

  it('wraps rather than clamping, so a phase can keep advancing', () => {
    expect(gradientColor(3.25)).toBe(gradientColor(0.25));
    expect(gradientColor(-0.75)).toBe(gradientColor(0.25));
  });

  it('interpolates between the two stops it sits between', () => {
    // Halfway along the first segment: pink #ffb3ba to peach #ffc8a2.
    expect(gradientColor(0.5 / 7)).toBe('#ffbdae');
  });

  it('always answers a six-digit hex', () => {
    for (let i = 0; i <= 200; i += 1) {
      expect(gradientColor(i / 200)).toMatch(/^#[0-9a-f]{6}$/);
    }
  });
});

describe('parity with effects::gradient_color', () => {
  it('interpolates over the same number of segments', () => {
    expect(rustGradientFn()).toContain('let segments = (GRADIENT.len() - 1) as f64');
  });

  it('truncates each channel the way `as u8` does, rather than rounding', () => {
    const body = rustGradientFn();
    expect(body).toContain('(r1 + (r2 - r1) * frac) as u8');
    expect(body).not.toContain('.round()');
  });

  it('wraps its position the way this one does', () => {
    expect(rustGradientFn()).toContain('t.rem_euclid(1.0)');
  });
});
