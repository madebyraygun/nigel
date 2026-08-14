import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * Screens are columns filling the content area.
 *
 * `wc-app-shell` stretches whatever is in its default slot to the whole
 * content area, and `wc-empty-state` asks a column for the height nothing else
 * claimed. Both halves are in `@nigel/ui`; the part that lives here is the
 * screen being a column, which is what carries the height between them. A
 * screen that stacks its children some other way is not wrong so much as
 * silently different: an empty state on it sits at the top of the page while
 * the same element on every other screen is centred.
 *
 * `unlock` is exempt. It replaces the shell rather than filling it — there is
 * no sidebar and no header behind the password prompt — and centres itself.
 */
const here = dirname(fileURLToPath(import.meta.url));
const screensDir = resolve(here, '../screens');

const EXEMPT = new Set(['unlock.ts']);

function screenSources(): { name: string; text: string }[] {
  return readdirSync(screensDir)
    .filter((name) => name.endsWith('.ts'))
    .filter((name) => !name.endsWith('.test.ts') && !EXEMPT.has(name))
    .map((name) => ({ name, text: readFileSync(join(screensDir, name), 'utf8') }))
    .filter(({ text }) => text.includes('@customElement('));
}

export function hostBlock(text: string): string | null {
  const withoutComments = text.replace(/\/\*[\s\S]*?\*\//g, '');
  return /:host\s*\{([^}]*)\}/.exec(withoutComments)?.[1] ?? null;
}

export function isColumn(block: string | null): boolean {
  if (block === null) return false;
  return /display:\s*flex\s*;/.test(block) && /flex-direction:\s*column\s*;/.test(block);
}

describe('screen layout', () => {
  const screens = screenSources();

  it('finds the screens', () => {
    // A rename that empties the list would otherwise pass every check below.
    expect(screens.length).toBeGreaterThan(10);
  });

  it('lays every screen out as a column that fills the content area', () => {
    const offenders = screens
      .filter(({ text }) => !isColumn(hostBlock(text)))
      .map(({ name }) => name);
    expect(offenders).toEqual([]);
  });
});
