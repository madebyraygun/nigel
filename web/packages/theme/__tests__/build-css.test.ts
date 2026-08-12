import { describe, it, expect, beforeAll } from 'vitest';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { nigelTheme } from '../src/themes/nigel.js';

const here = dirname(fileURLToPath(import.meta.url));
const pkgRoot = resolve(here, '..');
const cssPath = resolve(pkgRoot, 'dist/css/nigel.css');

describe('build-css', () => {
  beforeAll(() => {
    // The script consumes dist/themes/nigel.js, so compile first. Running the
    // real pipeline is the point: a stylesheet that only exists because a test
    // wrote it proves nothing about `npm run build`.
    execFileSync('npx', ['tsc'], { cwd: pkgRoot, stdio: 'pipe' });
    execFileSync('node', ['scripts/build-css.js'], { cwd: pkgRoot, stdio: 'pipe' });
  }, 120_000);

  it('emits dist/css/nigel.css', () => {
    expect(existsSync(cssPath)).toBe(true);
  });

  it('writes the banner and the full composed sheet', () => {
    const css = readFileSync(cssPath, 'utf8');
    expect(css).toContain('Nigel theme tokens');
    expect(css).toContain(nigelTheme.cssText);
  });

  it('copies every declared face into dist/fonts', () => {
    // A @font-face pointing at a file that is not there fails silently: the
    // browser falls back and the whole UI renders in a system face while every
    // other test passes. This is the assertion that catches that.
    const declared = [
      ...nigelTheme.cssText.matchAll(/url\(['"]\.\.\/fonts\/(.*?)['"]\)/g),
    ].map((m) => m[1]);

    expect(declared.length).toBeGreaterThan(0);
    for (const file of declared) {
      expect(existsSync(resolve(pkgRoot, 'dist/fonts', file)), file).toBe(true);
    }
  });

  it('resolves those relative urls from where the stylesheet lands', () => {
    // The urls are relative to dist/css/nigel.css, so ../fonts/ is dist/fonts/.
    // Vite rewrites them at build time from that anchor; if the css moved and
    // the fonts did not, the paths would still look right and resolve nowhere.
    const declared = [
      ...readFileSync(cssPath, 'utf8').matchAll(/url\(['"]\.\.\/fonts\/(.*?)['"]\)/g),
    ].map((m) => m[1]);

    for (const file of declared) {
      expect(existsSync(resolve(cssPath, '..', '../fonts', file)), file).toBe(true);
    }
  });
});
