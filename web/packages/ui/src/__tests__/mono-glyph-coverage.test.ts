import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * IBM Plex Mono is the app's primary face and has no glyph for any of these
 * eight characters — a property of the complete upstream release, not of the
 * subset in `@nigel/theme`, so a wider subset would not fix it. Drawn as text
 * they each come from whatever fallback face the browser finds, which puts two
 * typefaces on one line.
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

const TREES = ['packages/ui/src', 'packages/theme/src', 'apps/app/src'];

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

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === 'node_modules' || entry.name === 'dist') continue;
      out.push(...walk(full));
    } else if (entry.name.endsWith('.ts') && !entry.name.endsWith('.test.ts')) {
      out.push(full);
    }
  }
  return out;
}

const files = TREES.flatMap((tree) => walk(resolve(webRoot, tree)));

describe('glyphs the primary face does not have', () => {
  it('scans a tree that is actually there', () => {
    // Guards the guard: a mistyped path would make the sweep below vacuous.
    expect(files.length).toBeGreaterThan(50);
  });

  it.each(MISSING)('%s is drawn by %s, never typed as a character', (char, icon) => {
    const offenders = files
      .filter((file) => readFileSync(file, 'utf8').includes(char))
      .map((file) => relative(webRoot, file));

    expect(
      offenders,
      `IBM Plex Mono has no ${char} — render <${icon}> instead:\n${offenders.join('\n')}`,
    ).toEqual([]);
  });

  it('names an icon that exists for every character it forbids', async () => {
    // A rule pointing at an icon nobody registered is advice, not a fix.
    const { ICON_TAGS } = await import('../icons/icons.js');
    for (const [, icon] of MISSING) {
      expect(ICON_TAGS as readonly string[]).toContain(icon);
    }
  });
});
