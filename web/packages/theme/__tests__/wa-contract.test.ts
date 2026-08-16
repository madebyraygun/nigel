import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { waContractCss } from '../src/tokens/wa-contract.js';
import { nigelTheme } from '../src/themes/nigel.js';

const here = dirname(fileURLToPath(import.meta.url));
const waDist = resolve(here, '../../../node_modules/@awesome.me/webawesome/dist');

/**
 * The nine Web Awesome primitives this app renders. Kept in step with the
 * components that import them by `controls-adoption`'s sibling guard in
 * packages/ui — if a tenth is adopted, this list is where its tokens get
 * answered.
 */
const COMPONENTS = [
  'button',
  'dialog',
  'format-bytes',
  'input',
  'option',
  'radio',
  'radio-group',
  'select',
  'switch',
];

/**
 * Every `--wa-*` token those components' compiled styles actually read.
 *
 * Read out of the installed package rather than written down, because the
 * whole failure this guards against is a token WA consumes and we never
 * define. A hand-maintained list would go stale on the next WA upgrade in
 * exactly the way that produces an unstyled control and a green suite.
 */
function tokensWebAwesomeReads(): Map<string, Set<string>> {
  const chunkDir = join(waDist, 'chunks');
  const chunks = new Set(readdirSync(chunkDir).filter((f) => f.endsWith('.js')));
  const consumed = new Map<string, Set<string>>();
  const seen = new Set<string>();

  const walk = (component: string, chunk: string, depth = 0): void => {
    const key = `${component}:${chunk}`;
    if (depth > 4 || seen.has(key) || !chunks.has(chunk)) return;
    seen.add(key);

    const text = readFileSync(join(chunkDir, chunk), 'utf8');
    // `var(--x, fallback)` is not a token we owe a value. WA uses that form
    // for its per-variant indirection — `var(--wa-color-fill-loud,
    // var(--wa-color-neutral-fill-loud))` is meant to be set by a variant
    // class, and defining it at :root would pin every button to one variant.
    // Only a bare `var(--x)` is a hole.
    for (const [, token, next] of text.matchAll(/var\(\s*(--wa-[a-z0-9-]+)\s*(.)/g)) {
      if (next === ',') continue;
      if (!consumed.has(token)) consumed.set(token, new Set());
      consumed.get(token)!.add(component);
    }
    for (const [, next] of text.matchAll(/\.\/(chunk\.[A-Z0-9]+\.js)/g)) {
      walk(component, next, depth + 1);
    }
  };

  for (const component of COMPONENTS) {
    const entry = join(waDist, 'components', component, `${component}.styles.js`);
    if (!existsSync(entry)) continue;
    for (const [, chunk] of readFileSync(entry, 'utf8').matchAll(
      /chunks\/(chunk\.[A-Z0-9]+\.js)/g,
    )) {
      walk(component, chunk);
    }
  }
  return consumed;
}

/**
 * The leaf end of Web Awesome's variant indirection.
 *
 * Two classes of token come out of that indirection and they are owed opposite
 * things. The *intermediate* names — `--wa-color-fill-loud` and its eight
 * siblings — are deliberately not demanded above: a variant class sets them,
 * and defining them at `:root` would pin every control to one variant. The
 * *leaf families* they point at, `--wa-color-{variant}-{fill,on,border}-{loud,
 * normal,quiet}`, are the theme's job, and a missing one is not a default but
 * nothing — so the declaration is discarded and the control falls back to the
 * neutral fill.
 *
 * They also hide from the scan above twice over: inside a component's styles
 * they appear only as the fallback half of `var(--x, var(--y))`, which the
 * bare-var rule skips, and the sheet that reads them properly —
 * `variants.styles` — is pulled in by the component *module*, which a walk
 * from `<name>.styles.js` never reaches. Hence the separate entry point.
 */
const VARIANT_FAMILY = /--wa-color-(?:brand|neutral|danger|success|warning)-(?:fill|on|border)-(?:loud|normal|quiet)/g;

function variantFamiliesWebAwesomeReads(): Map<string, Set<string>> {
  const chunkDir = join(waDist, 'chunks');
  const chunks = new Set(readdirSync(chunkDir).filter((f) => f.endsWith('.js')));
  const consumed = new Map<string, Set<string>>();
  const seen = new Set<string>();

  const walk = (component: string, chunk: string, depth = 0): void => {
    const key = `${component}:${chunk}`;
    if (depth > 4 || seen.has(key) || !chunks.has(chunk)) return;
    seen.add(key);

    const text = readFileSync(join(chunkDir, chunk), 'utf8');
    for (const [token] of text.matchAll(VARIANT_FAMILY)) {
      if (!consumed.has(token)) consumed.set(token, new Set());
      consumed.get(token)!.add(component);
    }
    for (const [, next] of text.matchAll(/\.\/(chunk\.[A-Z0-9]+\.js)/g)) {
      walk(component, next, depth + 1);
    }
  };

  for (const component of COMPONENTS) {
    const entry = join(waDist, 'components', component, `${component}.js`);
    if (!existsSync(entry)) continue;
    for (const [, chunk] of readFileSync(entry, 'utf8').matchAll(
      /\.\/(?:\.\.\/)*chunks\/(chunk\.[A-Z0-9]+\.js)/g,
    )) {
      walk(component, chunk);
    }
  }
  return consumed;
}

/**
 * What the sheet defines *on screen*. The print block redefines a handful of
 * tokens, and counting those would report a token as defined when a component
 * on screen still sees nothing — which is how three shadow tokens hid.
 */
function definedOnScreen(): Set<string> {
  const text = nigelTheme.cssText;
  const printAt = text.indexOf('@media print');
  const screen = printAt === -1 ? text : text.slice(0, printAt);
  return new Set([...screen.matchAll(/(--wa-[a-z0-9-]+)\s*:/g)].map((m) => m[1]));
}

describe('the Web Awesome token contract', () => {
  const consumed = tokensWebAwesomeReads();

  it('reads a realistic number of tokens out of the installed package', () => {
    // Guards the guard: a moved chunk layout would silently consume nothing
    // and make the assertion below vacuously true.
    expect(consumed.size).toBeGreaterThan(50);
    expect(consumed.has('--wa-form-control-border-style')).toBe(true);
  });

  it('defines every token those components read', () => {
    const defined = definedOnScreen();
    const missing = [...consumed.keys()]
      .filter((token) => !defined.has(token))
      .sort()
      .map((token) => `  ${token}  (read by ${[...consumed.get(token)!].sort().join(', ')})`);

    expect(
      missing,
      `Web Awesome ships no stylesheet here, so a token it reads and the theme
does not define is not a default — it is nothing. Padding collapses to 0,
border-style falls to none, and a calc() containing one voids the whole
declaration. Define these in tokens/wa-contract.ts:\n${missing.join('\n')}`,
    ).toEqual([]);
  });

  it('defines every variant family those components consume', () => {
    const consumed = variantFamiliesWebAwesomeReads();
    const defined = definedOnScreen();

    // Guards the guard: the walk starts at the component module because
    // variants.styles hangs off the class, not the styles entry.
    expect(consumed.size).toBeGreaterThanOrEqual(45);

    const missing = [...consumed.keys()]
      .filter((token) => !defined.has(token))
      .sort()
      .map((token) => `  ${token}  (read by ${[...consumed.get(token)!].sort().join(', ')})`);

    expect(
      missing,
      `A variant's colour is two hops: variants.styles points --wa-color-fill-loud
at --wa-color-<variant>-fill-loud, and the component reads the first. An
undefined family is nothing, so the declaration is discarded and the control
renders in the neutral fill — which is a danger button that looks like every
other button. Define these in tokens/wa-contract.ts:\n${missing.join('\n')}`,
    ).toEqual([]);
  });

  it('is defined on screen, not only under @media print', () => {
    // --wa-shadow-s/m/l were previously defined only inside the print block,
    // so the dialog had no shadow on screen while a naive scan said otherwise.
    for (const token of ['--wa-shadow-s', '--wa-shadow-m', '--wa-shadow-l']) {
      expect(definedOnScreen().has(token), token).toBe(true);
    }
  });
});

describe('waContractCss', () => {
  it('carries no literal colour, so the contrast suite is unaffected', () => {
    // contrast.test.ts selects a token by the nth #rrggbb occurrence in the
    // composed sheet. A hex here would shift every index and fail that suite
    // with a message pointing nowhere near the cause.
    expect(waContractCss.cssText).not.toMatch(/#[0-9a-f]{3,8}\b/i);
  });

  it('derives from the package tokens rather than restating values', () => {
    // The point of the token route over per-part rules: one definition
    // reaches every wa-* component, and dark mode and print follow for free
    // because var() resolves at use time against whatever is in scope.
    const declarations = [...waContractCss.cssText.matchAll(/--wa-[a-z0-9-]+:\s*([^;]+);/g)];
    expect(declarations.length).toBeGreaterThan(50);

    const colourish = declarations.filter(([, value]) => /color/i.test(value));
    for (const [declaration, value] of colourish) {
      expect(value, declaration).toMatch(/var\(--|rgb\(/);
    }
  });

  it('composes into nigelTheme', () => {
    expect(nigelTheme.cssText).toContain(waContractCss.cssText);
  });
});
