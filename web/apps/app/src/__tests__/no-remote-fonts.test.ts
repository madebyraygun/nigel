import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * `nigel serve` is a single binary that may be running with no network at all.
 * Everything it needs is embedded, and the typeface is no exception — the
 * faces are committed in `@nigel/theme` and served from the same binary.
 *
 * One `<link rel="stylesheet" href="https://fonts.googleapis.com/…">` would
 * undo that without breaking a single other test: online it would look fine,
 * and offline the whole app would quietly render in a system face. So the
 * three HTML entry points are scanned instead.
 */
const here = dirname(fileURLToPath(import.meta.url));
const webRoot = resolve(here, '../../../../');

const PAGES = [
  'apps/app/index.html',
  'packages/ui/preview/index.html',
  'placeholder/index.html',
];

const FORBIDDEN: [RegExp, string][] = [
  [/fonts\.googleapis/i, 'a Google Fonts stylesheet'],
  [/fonts\.gstatic/i, 'a Google Fonts asset host'],
  [/use\.typekit|typekit\.net/i, 'Typekit'],
  [/rel=["']?preconnect/i, 'a preconnect hint (nothing is off-origin)'],
  [/rel=["']?dns-prefetch/i, 'a dns-prefetch hint (nothing is off-origin)'],
  [/@import\s+url\(\s*['"]?https?:/i, 'a remote @import'],
  [/src:\s*url\(\s*['"]?(https?:)?\/\//i, 'an absolute font url'],
];

describe('no remote fonts', () => {
  it.each(PAGES)('%s references no font host', (page) => {
    const html = readFileSync(resolve(webRoot, page), 'utf8');
    const found = FORBIDDEN.filter(([pattern]) => pattern.test(html)).map(([, what]) => what);

    expect(
      found,
      `${page} must not reach off-origin for type — the binary carries the faces:\n${found.join('\n')}`,
    ).toEqual([]);
  });

  it('reads pages that actually exist', () => {
    // Guards the guard: a mistyped path would make every case above vacuous.
    for (const page of PAGES) {
      expect(readFileSync(resolve(webRoot, page), 'utf8').length).toBeGreaterThan(100);
    }
  });

  it.each([
    ['<link rel="preconnect" href="https://fonts.gstatic.com">', true],
    ['<link href="https://fonts.googleapis.com/css2?family=X" rel="stylesheet">', true],
    ["@import url('https://example.test/f.css');", true],
    ["src: url('//cdn.test/f.woff2') format('woff2');", true],
    ["src: url('../fonts/ibm-plex-mono-400.woff2') format('woff2');", false],
    ['<link rel="icon" href="/favicon.png">', false],
  ])('detects %s as remote: %s', (line, expected) => {
    // Without this, excluding a page or mistyping a pattern could disarm the
    // whole guard and nothing would notice.
    expect(FORBIDDEN.some(([pattern]) => pattern.test(line))).toBe(expected);
  });
});
