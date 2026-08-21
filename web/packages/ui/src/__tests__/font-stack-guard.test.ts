import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, extname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * No component names a typeface.
 *
 * `@nigel/theme` owns which face answers which job — `--wa-font-family-sans`
 * for chrome, `--nc-font-figures` and `--nc-font-money` for columns of digits,
 * `--nc-font-brand` for the wordmark and the company name — and a component
 * asks for the job, never the family. A stack written into a component would
 * be invisible to the theme: it would keep its old face through a change made
 * everywhere else, and no other test would notice, because a hardcoded stack
 * renders perfectly well.
 *
 * A generic fallback *inside* the `var()` is fine and often wanted — the
 * second argument only applies when the token is undefined, which is a
 * stylesheet that failed to load rather than a component making a choice.
 *
 * Test files are not scanned: the tests for the theme itself have to name the
 * families to assert which one a token ends at.
 */
const here = dirname(fileURLToPath(import.meta.url));
const webRoot = resolve(here, '../../../../');

/** Everything that reaches a browser and is not the theme's own declarations. */
const TREES = ['packages/ui/src', 'packages/ui/preview', 'apps/app/src'];

/** The shell is markup rather than a module, and carries a pre-theme stack. */
const FILES = ['apps/app/index.html'];

const SCANNED = ['.ts', '.js', '.mjs', '.cjs', '.css', '.html'];

function isScanned(filename: string): boolean {
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

const sources: [string, string][] = [
  ...TREES.flatMap((tree) => walk(resolve(webRoot, tree))),
  ...FILES.map((file) => resolve(webRoot, file)),
].map((file) => [relative(webRoot, file), readFileSync(file, 'utf8')]);

/** Every `font-family` declaration, with the file it was written in. */
function declarations(): { path: string; value: string }[] {
  const found: { path: string; value: string }[] = [];
  for (const [path, text] of sources) {
    for (const [, value] of text.matchAll(/font-family:\s*([^;}\n]+)/g)) {
      found.push({ path, value: value.trim() });
    }
  }
  return found;
}

describe('font stacks live in @nigel/theme', () => {
  it('finds declarations in every tree it scans', () => {
    // Guards the guard: a mistyped path would make the sweep below vacuous,
    // and it would still pass.
    for (const tree of [...TREES, ...FILES]) {
      expect(
        declarations().filter(({ path }) => path.startsWith(tree)).length,
        `${tree} contributed no font-family declaration`,
      ).toBeGreaterThan(0);
    }
  });

  it('resolves every one of them through a token', () => {
    const offenders = declarations()
      .filter(({ value }) => !value.startsWith('var('))
      .map(({ path, value }) => `${path}: ${value}`);

    expect(
      offenders,
      `read a @nigel/theme token instead of naming a family:\n${offenders.join('\n')}`,
    ).toEqual([]);
  });
});
