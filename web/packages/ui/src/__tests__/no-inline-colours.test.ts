import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * Every colour the web UI paints comes from `@nigel/theme`. A literal — even
 * as a `var()` fallback — is a second copy of a brand value that the theme
 * cannot move, and a fallback that disagrees with the token it stands behind
 * is what renders the day the token is renamed. Scan the source, fail the
 * build, name the line.
 *
 * Covered: `@nigel/ui` (components, icons, and the preview harness, whose
 * chrome is reviewed in both modes like everything else) and `@nigel/app`
 * (screens and `index.html`). `@nigel/theme` is where the values live, so it
 * is the one place they are allowed. Test files are exempt: they assert on
 * colours rather than paint them.
 */
const here = dirname(fileURLToPath(import.meta.url));
const webDir = resolve(here, '../../../../');

const ROOTS = [
  'packages/ui/src',
  'packages/ui/preview',
  'apps/app/src',
  'apps/app/index.html',
].map((p) => join(webDir, p));

/** Colour functions, every syntax CSS has. `color-mix(` is not one of them. */
const COLOUR_FUNCTION = /\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\(/;

/**
 * Hex literals of the lengths CSS accepts. A run of digits only is also how
 * an invoice number reads in prose ("Invoice #1251"), so the four-digit form
 * — which collides with those — counts only when it contains a hex letter.
 */
const HEX = /#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3})\b|#(?=[0-9]*[a-fA-F])[0-9a-fA-F]{4}\b/;

const NAMED = 'white|black|red|green|blue|gr[ae]y|silver|yellow|orange|purple|pink|navy|teal|maroon';

/** A named colour as a colour property's value, or as a `var()` fallback. */
const NAMED_COLOUR = new RegExp(
  String.raw`(?:\b(?:color|background(?:-color)?|border(?:-[a-z]+)*|outline(?:-color)?|fill|stroke|box-shadow|text-shadow|text-decoration-color|caret-color|accent-color)\s*:\s*[^;{}]*\b(?:${NAMED})\b)` +
    String.raw`|(?:var\(--[\w-]+\s*,\s*(?:${NAMED})\s*\))`,
  'i',
);

function paintsColour(line: string): boolean {
  return COLOUR_FUNCTION.test(line) || HEX.test(line) || NAMED_COLOUR.test(line);
}

function walk(path: string): string[] {
  if (!statSync(path).isDirectory()) return [path];
  const out: string[] = [];
  for (const entry of readdirSync(path, { withFileTypes: true })) {
    const full = join(path, entry.name);
    if (entry.isDirectory()) {
      if (['node_modules', 'dist', '__tests__'].includes(entry.name)) continue;
      out.push(...walk(full));
    } else if (/\.(ts|html)$/.test(entry.name) && !entry.name.endsWith('.test.ts')) {
      out.push(full);
    }
  }
  return out;
}

describe('the colour matcher', () => {
  it.each([
    'background: #fff;',
    'color: var(--wa-color-text, #2b2b33);',
    'border: 1px solid #000;',
    "inverse: '#17171d',",
    'box-shadow: 0 0 0 1px rgb(0 0 0 / 10%);',
    'color: oklch(70% 0.1 250);',
    'background: color(display-p3 1 0 0);',
    'background: hwb(0 0% 0%);',
    'color: white;',
    'border-color: black;',
    'background: var(--x, white);',
    'outline: 2px solid #abcd;',
  ])('flags %s', (line) => {
    expect(paintsColour(line)).toBe(true);
  });

  it.each([
    "'<h1>Invoice #1251</h1><p>Acme Co — $1,850.00</p>'",
    'Invoice #1248 is overdue',
    'white-space: nowrap;',
    'background: color-mix(in srgb, var(--wa-color-danger) 10%, var(--wa-color-surface));',
    'const c = gradientColor(t);',
    'the white card behind it',
    'color: var(--wa-color-text);',
  ])('leaves %s alone', (line) => {
    expect(paintsColour(line)).toBe(false);
  });
});

describe('the web UI carries no inline colours', () => {
  const files = ROOTS.flatMap(walk);

  it('finds the sources it is meant to scan', () => {
    for (const root of ROOTS) {
      expect(files.some((f) => f.startsWith(root)), root).toBe(true);
    }
  });

  it('paints every colour from a theme token', () => {
    const hits = files.flatMap((file) =>
      readFileSync(file, 'utf8')
        .split('\n')
        .map((line, i) => [line, i + 1] as const)
        .filter(([line]) => paintsColour(line))
        .map(([line, n]) => `${relative(webDir, file)}:${n}: ${line.trim()}`),
    );
    expect(hits).toEqual([]);
  });
});
