import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { nigelTheme } from '@nigel/theme';

/**
 * A `var()` fallback for a token the theme always defines never renders —
 * the document sheet is loaded wherever a component is — so all it can do is
 * go stale and mislead the next reader about the value. Fallbacks stay only
 * for a component's own knobs (`--nc-register-height` and the like), which
 * nothing defines unless a caller sets them.
 */
const here = dirname(fileURLToPath(import.meta.url));
const srcDir = resolve(here, '../');

const themeTokens = new Set(
  [...nigelTheme.cssText.matchAll(/(--[\w-]+)\s*:/g)].map((m) => m[1]),
);

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (['node_modules', 'dist', '__tests__'].includes(entry.name)) continue;
      out.push(...walk(full));
    } else if (entry.name.endsWith('.ts') && !entry.name.endsWith('.test.ts')) {
      out.push(full);
    }
  }
  return out;
}

describe('@nigel/ui names no fallback for a theme token', () => {
  it('reads the token list from the theme', () => {
    expect(themeTokens.has('--wa-color-text')).toBe(true);
  });

  it('leaves every theme token to the theme', () => {
    const hits = walk(srcDir).flatMap((file) =>
      [...readFileSync(file, 'utf8').matchAll(/var\(\s*(--[\w-]+)\s*,/g)]
        .filter((m) => themeTokens.has(m[1]))
        .map((m) => `${relative(srcDir, file)}: ${m[0]}`),
    );
    expect(hits).toEqual([]);
  });
});
