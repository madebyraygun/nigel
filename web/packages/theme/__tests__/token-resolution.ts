import { nigelTheme } from '../src/themes/nigel.js';

/**
 * Resolving a token the way a browser would, so a suite can assert what a
 * control actually paints rather than what the sheet says.
 *
 * The theme is written in indirection: `--wa-color-danger-fill-loud` names
 * `--wa-color-danger`, which is a hex in light mode and another in dark. A
 * test that reads the declaration sees `var(--wa-color-danger)` and learns
 * nothing about contrast; following the chain to the hex is what makes the
 * question answerable.
 */

export type Mode = 'light' | 'dark';

const HEX = /^#[0-9a-f]{6}$/i;
const VAR_ONLY = /^var\(\s*(--[a-z0-9-]+)\s*\)$/i;
const SRGB_MIX = /^color-mix\(\s*in srgb\s*,\s*(.+?)\s+([\d.]+)%\s*,\s*(.+?)\s*\)$/i;

/**
 * The sheet as it applies on screen. The print block redefines a handful of
 * tokens at the very end, and taking the last declaration without cutting it
 * off would resolve dark mode to the printed value.
 */
function screenSheet(): string {
  const text = nigelTheme.cssText;
  const printAt = text.indexOf('@media print');
  return printAt === -1 ? text : text.slice(0, printAt);
}

/** Every value declared for a custom property, in sheet order. */
export function declarationsOf(name: string): string[] {
  return [...screenSheet().matchAll(new RegExp(`${name}:\\s*([^;]+);`, 'g'))].map((m) =>
    m[1].trim().replace(/\s+/g, ' '),
  );
}

/**
 * Light mode reads the first declaration and dark the last: the light block
 * comes first, the two dark blocks (media query and `.dark-mode`) after it and
 * identical to each other. A token declared once answers both.
 */
function declarationFor(name: string, mode: Mode): string {
  const declared = declarationsOf(name);
  if (declared.length === 0) throw new Error(`${name} is not declared on screen`);
  return mode === 'light' ? declared[0] : declared[declared.length - 1];
}

function channels(hex: string): [number, number, number] {
  const v = hex.replace('#', '');
  return [
    parseInt(v.slice(0, 2), 16),
    parseInt(v.slice(2, 4), 16),
    parseInt(v.slice(4, 6), 16),
  ];
}

/** `color-mix(in srgb, a p%, b)` — a straight per-channel interpolation. */
function mixSrgb(a: string, portion: number, b: string): string {
  const [ar, ag, ab] = channels(a);
  const [br, bg, bb] = channels(b);
  return (
    '#' +
    [
      [ar, br],
      [ag, bg],
      [ab, bb],
    ]
      .map(([x, y]) =>
        Math.round(x * portion + y * (1 - portion))
          .toString(16)
          .padStart(2, '0'),
      )
      .join('')
  );
}

function resolveValue(value: string, mode: Mode, trail: string[]): string {
  if (HEX.test(value)) return value.toLowerCase();

  const reference = VAR_ONLY.exec(value);
  if (reference) {
    const next = reference[1];
    if (trail.includes(next)) throw new Error(`token cycle: ${[...trail, next].join(' -> ')}`);
    return resolveValue(declarationFor(next, mode), mode, [...trail, next]);
  }

  const mix = SRGB_MIX.exec(value);
  if (mix) {
    const [, first, percent, second] = mix;
    return mixSrgb(
      resolveValue(first, mode, trail),
      Number(percent) / 100,
      resolveValue(second, mode, trail),
    );
  }

  throw new Error(`cannot resolve ${trail.join(' -> ')} in ${mode} mode: ${value}`);
}

/** Follow a token to the colour it ends at, in the given mode. */
export function resolveToken(name: string, mode: Mode): string {
  return resolveValue(declarationFor(name, mode), mode, [name]);
}

export function relativeLuminance(hex: string): number {
  const srgb = channels(hex).map((c) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * srgb[0] + 0.7152 * srgb[1] + 0.0722 * srgb[2];
}

export function contrast(a: string, b: string): number {
  const la = relativeLuminance(a);
  const lb = relativeLuminance(b);
  const [hi, lo] = la > lb ? [la, lb] : [lb, la];
  return (hi + 0.05) / (lo + 0.05);
}
