import { describe, it, expect } from 'vitest';
import { nigelTheme } from '../src/themes/nigel.js';
import { NIGEL_PALETTE, NIGEL_PALETTE_INK } from '../src/tokens/gradient.js';
import { contrast, declarationsOf, resolveToken, type Mode } from './token-resolution.js';

/**
 * The app-refresh handoff's token table (`docs/design/app-refresh/README.md`,
 * "Design tokens"), keyed by the role each shorthand name maps onto. The
 * handoff is high fidelity, so its values are the theme's values: this
 * fails if either drifts from the other.
 */
const COLORS: [shorthand: string, role: string, light: string, dark: string][] = [
  ['--bg', '--wa-color-bg', '#f3f2f7', '#17171d'],
  ['--sf', '--wa-color-surface', '#ffffff', '#1f1f28'],
  ['--sa', '--wa-color-surface-alt', '#f6f4fb', '#25252f'],
  ['--sb', '--nc-color-sidebar-bg', '#fbfafe', '#22222b'],
  ['--hl', '--nc-color-highlight', '#f1edff', '#2a2740'],
  ['--hv', '--nc-color-hover', '#efedf5', '#2a2a35'],
  ['--sel', '--nc-color-selected-bg', '#f1edff', '#2a2740'],
  ['--bd', '--wa-color-border', '#dedaeb', '#2e2e3c'],
  ['--bs', '--wa-color-border-soft', '#ebe8f3', '#26262f'],
  ['--tx', '--wa-color-text', '#383843', '#ece9f5'],
  ['--mu', '--wa-color-muted', '#63636f', '#a5a2b5'],
  ['--br', '--wa-color-brand', '#5a3fd6', '#c4b7ff'],
  ['--onbr', '--wa-color-on-brand', '#ffffff', '#17171d'],
  ['--field', '--nc-color-field', '#ffffff', '#1b1b22'],
  ['--inc', '--nc-color-income', '#17683a', '#7fe0a0'],
  ['--exp', '--nc-color-expense', '#b3283f', '#ff9fa8'],
  ['--exp (danger)', '--wa-color-danger', '#b3283f', '#ff9fa8'],
  ['--flag', '--nc-color-flagged', '#855508', '#ffe0a3'],
  ['--blue', '--wa-color-info', '#266ba1', '#bae1ff'],
  ['--incf', '--nc-color-income-fill', '#5a9473', '#6fcf92'],
  ['--expf', '--nc-color-expense-fill', '#cb6b7b', '#f0919c'],
];

const VALUES: [shorthand: string, role: string, light: string, dark: string][] = [
  ['--ring', '--nc-color-ring', 'rgb(90 63 214 / 22%)', 'rgb(196 183 255 / 25%)'],
  ['--bk', '--wa-color-overlay-modal', 'rgb(40 36 60 / 18%)', 'rgb(0 0 0 / 45%)'],
  [
    '--fsh',
    '--nc-shadow-floating',
    '0 1px 2px rgb(43 43 51 / 6%), 0 8px 24px rgb(43 43 51 / 6%)',
    '0 8px 24px rgb(0 0 0 / 35%)',
  ],
  [
    '--sh',
    '--wa-shadow-lg',
    '0 16px 40px rgb(43 43 51 / 18%), 0 2px 6px rgb(43 43 51 / 6%)',
    '0 20px 50px rgb(0 0 0 / 55%), 0 0 0 1px rgb(255 255 255 / 4%)',
  ],
  [
    '--grad',
    '--nc-grad-brand-text',
    `linear-gradient(90deg, ${NIGEL_PALETTE_INK.join(', ')})`,
    `linear-gradient(90deg, ${NIGEL_PALETTE.join(', ')})`,
  ],
  [
    '--pastel',
    '--nc-grad-pastel',
    `linear-gradient(90deg, ${NIGEL_PALETTE.join(', ')})`,
    `linear-gradient(90deg, ${NIGEL_PALETTE.join(', ')})`,
  ],
];

function declared(role: string, mode: Mode): string {
  const all = declarationsOf(role);
  expect(all.length, `${role} is not declared`).toBeGreaterThan(0);
  return mode === 'light' ? all[0] : all[all.length - 1];
}

describe('the handoff token table', () => {
  describe.each<Mode>(['light', 'dark'])('%s mode', (mode) => {
    it.each(COLORS)('%s maps onto %s', (_short, role, light, dark) => {
      expect(resolveToken(role, mode)).toBe(mode === 'light' ? light : dark);
    });

    it.each(VALUES)('%s maps onto %s', (_short, role, light, dark) => {
      expect(declared(role, mode)).toBe(mode === 'light' ? light : dark);
    });
  });

  it('carries the monogram ink, which is the on-gradient ink', () => {
    expect(declarationsOf('--nc-color-on-gradient')).toEqual(['#2b2b33']);
  });

  it.each([
    ['--wa-radius-xs', '4px'],
    ['--wa-radius-sm', '6px'],
    ['--wa-radius-md', '8px'],
    ['--nc-radius-tile', '10px'],
    ['--wa-radius-lg', '12px'],
    ['--wa-radius-xl', '14px'],
    ['--wa-radius-2xl', '18px'],
    ['--wa-radius-pill', '999px'],
  ])('carries the radius %s at %s', (role, value) => {
    expect(declared(role, 'light')).toBe(value);
  });

  it.each([
    ['--nc-font-size-display', '28px'],
    ['--nc-font-size-heading', '26px'],
    ['--nc-font-size-heading-sm', '24px'],
    ['--nc-font-size-figure', '22px'],
    ['--nc-font-size-section', '16px'],
    ['--nc-font-size-card-title', '15px'],
    ['--nc-font-size-body', '13.5px'],
    ['--nc-font-size-control', '13px'],
    ['--nc-font-size-meta', '12px'],
    ['--nc-font-size-caption', '11.5px'],
    ['--nc-font-size-label', '10.5px'],
    ['--nc-letter-spacing-tight', '-0.02em'],
    ['--nc-letter-spacing-label', '0.08em'],
    ['--wa-font-size-base', '14px'],
    ['--wa-line-height', '1.5'],
  ])('carries the type token %s at %s', (role, value) => {
    expect(declared(role, 'light')).toBe(value);
  });

  it('sets every figure on tabular numerals from the root', () => {
    expect(nigelTheme.cssText).toMatch(/:root\s*{[^}]*font-variant-numeric:\s*tabular-nums/);
  });
});

/**
 * Body copy owes WCAG AA's 4.5:1, and both text tokens are drawn on every
 * plane the app has: the canvas, the floating panels, cards, the subtle fill,
 * fields, and the hover, highlight and selection fills that sit on top of
 * them. Resolved per mode, so a plane added to the table above without a
 * legible text pairing fails here.
 */
const PLANES = [
  '--wa-color-bg',
  '--nc-color-sidebar-bg',
  '--wa-color-surface',
  '--wa-color-surface-alt',
  '--nc-color-field',
  '--nc-color-hover',
  '--nc-color-highlight',
  '--nc-color-selected-bg',
];

/**
 * The coloured foregrounds — links and the brand, signed money, the flag, the
 * Sent status — land on the same planes as text does: a highlighted row, a
 * hovered one, a field, the sidebar's review count.
 */
const ACCENTS = [
  '--wa-color-brand',
  '--nc-color-income',
  '--nc-color-expense',
  '--nc-color-flagged',
  '--wa-color-info',
];
const ACCENT_PLANES = [
  '--wa-color-surface-alt',
  '--nc-color-hover',
  '--nc-color-highlight',
  '--nc-color-field',
  '--nc-color-sidebar-bg',
];

describe.each<Mode>(['light', 'dark'])('%s mode: accents are AA on every plane', (mode) => {
  it.each(ACCENTS.flatMap((fg) => ACCENT_PLANES.map((bg) => [fg, bg])))('%s on %s', (fg, bg) => {
    expect(contrast(resolveToken(fg, mode), resolveToken(bg, mode))).toBeGreaterThanOrEqual(4.5);
  });

  /** wc-toast labels its success and danger chips, and an info fill, in on-brand. */
  it.each(['--wa-color-success', '--wa-color-danger', '--wa-color-info'])(
    'on-brand is AA on a %s fill',
    (fill) => {
      expect(
        contrast(resolveToken('--wa-color-on-brand', mode), resolveToken(fill, mode)),
      ).toBeGreaterThanOrEqual(4.5);
    },
  );
});

type Rgba = { r: number; g: number; b: number; a: number };

function parseRgba(value: string): Rgba {
  const m = /^rgb\(\s*(\d+)\s+(\d+)\s+(\d+)\s*\/\s*([\d.]+)%\s*\)$/.exec(value);
  expect(m, `${value} is not rgb(r g b / a%)`).not.toBeNull();
  const [, r, g, b, a] = m!;
  return { r: Number(r), g: Number(g), b: Number(b), a: Number(a) / 100 };
}

function hexChannels(hex: string): [number, number, number] {
  return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)) as [number, number, number];
}

/** A translucent colour laid over an opaque one, as `#rrggbb`. */
function over(top: Rgba, ground: string): string {
  const [r, g, b] = hexChannels(ground);
  const mix = (t: number, u: number) =>
    Math.round(t * top.a + u * (1 - top.a))
      .toString(16)
      .padStart(2, '0');
  return `#${mix(top.r, r)}${mix(top.g, g)}${mix(top.b, b)}`;
}

describe.each<Mode>(['light', 'dark'])('%s mode: the translucent tokens', (mode) => {
  const ring = () => parseRgba(declared('--nc-color-ring', mode));
  const backdrop = () => parseRgba(declared('--wa-color-overlay-modal', mode));

  it('draws the focus ring in the brand colour', () => {
    const { r, g, b } = ring();
    expect([r, g, b]).toEqual(hexChannels(resolveToken('--wa-color-brand', mode)));
  });

  it('keeps the focus ring a halo: translucent, and at the handoff alpha', () => {
    expect(ring().a).toBeCloseTo(mode === 'light' ? 0.22 : 0.25, 5);
  });

  it('tints the field it surrounds, so the ring is seen', () => {
    const field = resolveToken('--nc-color-field', mode);
    expect(over(ring(), field)).not.toBe(field);
  });

  it('sets the backdrop at the handoff alpha', () => {
    expect(backdrop().a).toBeCloseTo(mode === 'light' ? 0.18 : 0.45, 5);
  });

  it.each(['--wa-color-bg', '--wa-color-surface', '--nc-color-sidebar-bg'])(
    'dims %s rather than washing it out',
    (plane) => {
      const ground = resolveToken(plane, mode);
      const dimmed = over(backdrop(), ground);
      const lum = (hex: string) => contrast(hex, '#000000');
      expect(lum(dimmed)).toBeLessThan(lum(ground));
    },
  );
});

describe.each<Mode>(['light', 'dark'])('%s mode: text is AA on every plane', (mode) => {
  it.each(
    ['--wa-color-text', '--wa-color-muted'].flatMap((fg) => PLANES.map((bg) => [fg, bg])),
  )('%s on %s', (fg, bg) => {
    expect(contrast(resolveToken(fg, mode), resolveToken(bg, mode))).toBeGreaterThanOrEqual(4.5);
  });
});
