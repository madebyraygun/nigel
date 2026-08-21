import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, extname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * No component names a typeface.
 *
 * `@nigel/theme` owns which face answers which job, and a component asks for
 * the job. A stack written into a component would be invisible to the theme:
 * it would keep its old face through a change made everywhere else, and no
 * other test would notice, because a hardcoded stack renders perfectly well.
 *
 * Test files are not scanned — the tests for the theme itself have to name the
 * families to assert which one a token ends at.
 */
const here = dirname(fileURLToPath(import.meta.url));
const webRoot = resolve(here, '../../../../');

/** Everything that reaches a browser and is not the theme's own declarations. */
const TREES = ['packages/ui/src', 'packages/ui/preview', 'apps/app/src'];

/**
 * The shell is markup rather than a module, and its `font-family` is the one
 * declaration in the app that legitimately spells a stack out: it paints the
 * first frame, before the token sheet has parsed. It is held to the token by
 * the drift test at the bottom of this file instead.
 */
const SHELL = 'apps/app/index.html';

const SCANNED = ['.ts', '.js', '.mjs', '.cjs', '.css', '.html'];

/**
 * The tokens a component may name, and nothing else. A whitelist rather than
 * "starts with var(" because both of the ways this rule gets broken quietly
 * pass that looser test: `var(--wa-font-family-sans), Helvetica` appends a
 * family the theme cannot reach, and `var(--wa-font-familly-sans)` resolves to
 * nothing at all and renders in the user agent's default.
 */
const ALLOWED = [
  '--wa-font-family-sans',
  '--wa-font-family-mono',
  '--nc-font-figures',
  '--nc-font-money',
  '--nc-font-brand',
];

function isScanned(filename: string): boolean {
  return SCANNED.includes(extname(filename)) && !filename.endsWith('.test.ts');
}

function walk(dir: string): string[] {
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch (cause) {
    throw new Error(
      `font-stack-guard cannot read ${relative(webRoot, dir)} — if a tree moved, ` +
        `update TREES in this file rather than letting the sweep go quiet`,
      { cause },
    );
  }

  const out: string[] = [];
  for (const entry of entries) {
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

function read(file: string): [string, string] {
  try {
    return [relative(webRoot, file), readFileSync(file, 'utf8')];
  } catch (cause) {
    throw new Error(`font-stack-guard cannot read ${relative(webRoot, file)}`, { cause });
  }
}

const sources: [string, string][] = TREES.flatMap((tree) =>
  walk(resolve(webRoot, tree)),
).map(read);

export interface Declaration {
  path: string;
  property: 'font-family' | 'font';
  value: string;
}

/**
 * Every declaration that can set a family: `font-family`, and the `font`
 * shorthand, whose last component is one. `font: inherit` is neither — it
 * takes whatever the tree above already resolved and names nothing.
 */
export function declarationsIn(path: string, text: string): Declaration[] {
  const found: Declaration[] = [];

  for (const [, value] of text.matchAll(/font-family:\s*([^;}\n]+)/g)) {
    found.push({ path, property: 'font-family', value: value.trim() });
  }

  for (const [, value] of text.matchAll(/(?<!-)\bfont:\s*([^;}\n]+)/g)) {
    const shorthand = value.trim();
    if (shorthand === 'inherit' || shorthand === 'initial' || shorthand === 'unset') continue;
    found.push({ path, property: 'font', value: shorthand });
  }

  return found;
}

/** The family half of a declaration: everything, or the shorthand's tail. */
export function familyOf(declaration: Declaration): string {
  if (declaration.property === 'font-family') return declaration.value;
  // In the shorthand the family is last, after the size (and any /line-height).
  const size = declaration.value.lastIndexOf(' ');
  return size === -1 ? declaration.value : declaration.value.slice(size + 1).trim();
}

export function isSanctioned(family: string): boolean {
  const exact = /^var\(\s*(--[a-z0-9-]+)\s*(?:,\s*([\s\S]+))?\)$/i.exec(family.trim());
  if (!exact) return false;

  const [, token, fallback] = exact;
  if (!ALLOWED.includes(token)) return false;
  // A fallback is a generic family for the case where the sheet never
  // loaded, not a second chance to name a face the theme does not know.
  if (fallback === undefined) return true;
  return fallback
    .split(',')
    .every((part) =>
      ['serif', 'sans-serif', 'monospace', 'ui-monospace', 'ui-sans-serif', 'system-ui'].includes(
        part.trim(),
      ),
    );
}

const declarations = sources.flatMap(([path, text]) => declarationsIn(path, text));

describe('font stacks live in @nigel/theme', () => {
  it('finds declarations in every tree it scans', () => {
    // Guards the guard: a mistyped path would make the sweep below vacuous,
    // and it would still pass.
    for (const tree of TREES) {
      expect(
        declarations.filter(({ path }) => path.startsWith(tree)).length,
        `${tree} contributed no font declaration`,
      ).toBeGreaterThan(0);
    }
  });

  it.each([
    ['var(--wa-font-family-sans)', true],
    ['var(--nc-font-money, ui-monospace, monospace)', true],
    ['var(--wa-font-family-mono, monospace)', true],
    ['var(--wa-font-family-sans), Helvetica', false],
    ['var(--wa-font-familly-sans)', false],
    ['var(--nc-font-money, Menlo)', false],
    ['var(--some-other-token)', false],
    ["'IBM Plex Mono', monospace", false],
    ['system-ui', false],
  ])('judges %s: %s', (family, sanctioned) => {
    // The looser "starts with var(" test passed four of these five failures,
    // which is what this case list exists to keep from coming back.
    expect(isSanctioned(family)).toBe(sanctioned);
  });

  it.each([
    ['font-family: var(--nc-font-brand);', 1],
    ['font: inherit;', 0],
    ['font: 500 14px/1.5 var(--wa-font-family-sans);', 1],
    ['font-weight: 500;', 0],
    ['font-size: 14px;', 0],
  ])('reads %s as %i declaration(s)', (source, count) => {
    // `font-weight` and `font-size` must not be mistaken for the shorthand,
    // and the shorthand must not be skipped.
    expect(declarationsIn('x.ts', source)).toHaveLength(count);
  });

  it('resolves every one of them through a sanctioned token', () => {
    const offenders = declarations
      .filter((declaration) => !isSanctioned(familyOf(declaration)))
      .map(({ path, property, value }) => `${path}: ${property}: ${value}`);

    expect(
      offenders,
      `name a @nigel/theme token — one of ${ALLOWED.join(', ')}:\n${offenders.join('\n')}`,
    ).toEqual([]);
  });
});

/**
 * The app shell paints before the token sheet has parsed, so its rule carries
 * the stack twice: once as the token and once as the fallback that answers
 * for that first frame. The two have to say the same thing or the first frame
 * is in a different face from the second, which is the flash this exists to
 * prevent — the same bargain `color-mode-bootstrap.test.ts` holds the inline
 * mode script to.
 */
describe('the shell paints the first frame in the same face', () => {
  const families = (stack: string): string[] =>
    stack
      .split(',')
      .map((part) => part.trim().replace(/^['"]|['"]$/g, ''))
      .filter(Boolean);

  const shell = read(resolve(webRoot, SHELL))[1];
  const declared = readFileSync(
    resolve(webRoot, 'packages/theme/src/tokens/typography.ts'),
    'utf8',
  );

  it('names the token first, so the fallback only ever answers for one frame', () => {
    expect(shell).toMatch(/font-family:\s*var\(--wa-font-family-sans\s*,/);
  });

  it('carries the same stack the token declares', () => {
    const inShell = /font-family:\s*var\(--wa-font-family-sans\s*,\s*([^)]+)\)/.exec(shell);
    expect(inShell, `${SHELL} has no --wa-font-family-sans fallback`).not.toBeNull();

    const inToken = /--wa-font-family-sans:\s*([^;]+);/.exec(declared);
    expect(inToken, 'typography.ts does not declare --wa-font-family-sans').not.toBeNull();

    const wanted = families(inToken![1]);
    expect(wanted.length).toBeGreaterThan(1);
    expect(families(inShell![1])).toEqual(wanted);
  });
});
