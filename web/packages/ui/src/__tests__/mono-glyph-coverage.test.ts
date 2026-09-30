import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, extname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { ICON_TAGS } from '../icons/icons.js';

/**
 * IBM Plex Mono is the app's bundled face — figures, the wordmark, the company
 * name, and every code-shaped field — and it has no glyph for any of these
 * eight characters. That is a property of the complete upstream release, not
 * of the subset in `@nigel/theme`, so a wider subset would not fix it. Drawn
 * as text they each come from whatever fallback face the browser finds, which
 * puts two typefaces on one line.
 *
 * The ban is on typing them anywhere rather than only in the places Plex
 * draws, because which token a string ends up under is a property of the
 * component that renders it and can change under a character that stayed put.
 *
 * They are drawn as `wc-icon-*` SVGs instead, and this is what keeps the next
 * component from typing one back in: online it would look plausible on the
 * author's machine and no other test would notice.
 *
 * Test files are not scanned — the tests for those very components have to
 * name the characters to say what they no longer render.
 */
const here = dirname(fileURLToPath(import.meta.url));
const webRoot = resolve(here, '../../../../');

/** Everything that reaches a browser: components, the preview shell, the app. */
const TREES = [
  'packages/ui/src',
  'packages/ui/preview',
  'packages/theme/src',
  'apps/app/src',
];

/**
 * Not just `.ts`. A character can be typed into a stylesheet's `content`, a
 * markup entity in an HTML shell, or a fixture a screen renders verbatim.
 */
const SCANNED = ['.ts', '.js', '.mjs', '.cjs', '.css', '.html', '.json', '.txt'];

/** Each character, with the icon that draws it. */
const MISSING: [string, string][] = [
  ['✗', 'wc-icon-close'],
  ['⟳', 'wc-icon-refresh'],
  ['◑', 'wc-icon-status-partial'],
  ['●', 'wc-icon-status-paid'],
  ['◆', 'wc-icon-status-sent'],
  ['▲', 'wc-icon-status-overdue'],
  ['⊘', 'wc-icon-status-void'],
  ['◻', 'wc-icon-status-draft'],
];

/**
 * Symbols the UI does set as text, and may: every one is inside a range the
 * subset keeps, so it is drawn by IBM Plex Mono like the words around it.
 * `✓` (the preview shell's zero-violations line, U+2713) has its own range;
 * the rest sit in Latin-1, General Punctuation or the arrows block.
 */
const PRESENT = ['✓', '×', '·', '•', '—', '–', '…', '−', '←', '↑', '→', '↓'];

export function isScanned(filename: string): boolean {
  return SCANNED.includes(extname(filename)) && !filename.endsWith('.test.ts');
}

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === 'node_modules' || entry.name === 'dist') continue;
      out.push(...walk(full));
    } else if (isScanned(entry.name)) {
      out.push(full);
    }
  }
  return out;
}

/** Every scanned file, read once, as [path relative to web/, contents]. */
const sources: [string, string][] = TREES.flatMap((tree) => walk(resolve(webRoot, tree))).map(
  (file) => [relative(webRoot, file), readFileSync(file, 'utf8')],
);

describe('glyphs the primary face does not have', () => {
  it('scans every tree, and finds files in each', () => {
    // Guards the guard: a mistyped path would make the sweep below vacuous,
    // and it would still pass.
    expect(sources.length).toBeGreaterThan(50);
    for (const tree of TREES) {
      expect(
        sources.filter(([path]) => path.startsWith(tree)).length,
        `${tree} contributed no files`,
      ).toBeGreaterThan(0);
    }
    // The preview shell is markup, not only modules; nothing about the rule is
    // specific to TypeScript.
    expect(sources.map(([path]) => path)).toContain('packages/ui/preview/index.html');
  });

  it.each([
    ['wc-invoice-status.ts', true],
    ['nigel.css', true],
    ['index.html', true],
    ['subset-fonts.mjs', true],
    ['previews.json', true],
    ['sample-statement.txt', true],
    ['wc-invoice-status.test.ts', false],
    ['ibm-plex-mono-400.woff2', false],
    ['logo.svg', false],
  ])('scans %s: %s', (filename, scanned) => {
    // A dropped extension would disarm the sweep for a whole file type with
    // every case above still passing — there is no .css in these trees today,
    // and the first one added must not arrive unswept.
    expect(isScanned(filename)).toBe(scanned);
  });

  it.each(MISSING)('%s is drawn by %s, never typed as a character', (char, icon) => {
    const offenders = sources
      .filter(([, text]) => text.includes(char))
      .map(([path]) => path);

    expect(
      offenders,
      `IBM Plex Mono has no ${char} — render <${icon}> instead:\n${offenders.join('\n')}`,
    ).toEqual([]);
  });

  it('names an icon that exists for every character it forbids', () => {
    // A rule pointing at an icon nobody registered is advice, not a fix.
    for (const [, icon] of MISSING) {
      expect(ICON_TAGS as readonly string[]).toContain(icon);
    }
  });

  it('forbids nothing it also permits', () => {
    const forbidden = MISSING.map(([char]) => char);
    expect(PRESENT.filter((char) => forbidden.includes(char))).toEqual([]);
  });
});
