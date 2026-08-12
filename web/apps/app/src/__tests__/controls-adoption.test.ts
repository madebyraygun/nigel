import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * The same guard `packages/ui` carries, for the screens that reach past
 * `@nigel/ui` and render a Web Awesome primitive directly.
 *
 * A `::part()` rule reaches one shadow boundary down, from the tree it is
 * written in, so a screen hosting a `wa-input` has to adopt `controlsCss`
 * itself — `main.ts` loading the document sheet does not reach it, and neither
 * does `nigel-app`. This is not styling logic in the app: it is adopting the
 * shared sheet, which is the only place it can be adopted from.
 *
 * A screen that needs a *new* primitive treatment still belongs in `@nigel/ui`
 * behind a `wc-*` wrapper; the Component-First workflow is unchanged.
 */
const here = dirname(fileURLToPath(import.meta.url));
const srcDir = resolve(here, '../');

const PRIMITIVE_IMPORT = '@awesome.me/webawesome/dist/components/';

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === 'node_modules' || entry.name === 'dist') continue;
      out.push(...walk(full));
    } else if (entry.name.endsWith('.ts')) {
      out.push(full);
    }
  }
  return out;
}

function isSource(file: string): boolean {
  return !file.endsWith('.test.ts') && !file.endsWith('.preview.ts');
}

export function rendersPrimitive(text: string): boolean {
  return text.includes(PRIMITIVE_IMPORT);
}

/**
 * Both halves are required. The import alone would pass a file that imports
 * the sheet and never puts it in `static styles`, which is exactly as dead as
 * not importing it.
 */
export function adoptsControls(text: string): boolean {
  return /from '@nigel\/theme'/.test(text) && /static styles = \[\s*controlsCss/.test(text);
}

describe('controlsCss adoption', () => {
  const sources = walk(srcDir).filter(isSource);

  it('is present in every screen that renders a wa-* primitive', () => {
    const offenders = sources
      .filter((f) => rendersPrimitive(readFileSync(f, 'utf8')))
      .filter((f) => !adoptsControls(readFileSync(f, 'utf8')))
      .map((f) => relative(srcDir, f));

    expect(
      offenders,
      `these render a Web Awesome primitive without adopting controlsCss.
Add \`import { controlsCss } from '@nigel/theme';\` and make styles
\`static styles = [controlsCss, css\`…\`]\` — a document-level ::part()
rule cannot reach into a component's shadow root:
${offenders.join('\n')}`,
    ).toEqual([]);
  });

  it('actually scans the screens', () => {
    // Guards the guard: a walk that silently returned nothing would pass above.
    const hosts = sources.filter((f) => rendersPrimitive(readFileSync(f, 'utf8')));
    expect(sources.length).toBeGreaterThan(10);
    expect(hosts.length).toBeGreaterThan(0);
  });

  it.each([
    ["import '@awesome.me/webawesome/dist/components/input/input.js';", true],
    ["import { html } from 'lit';", false],
  ])('detects %s as a primitive host: %s', (line, expected) => {
    expect(rendersPrimitive(line)).toBe(expected);
  });

  it.each([
    ["import { controlsCss } from '@nigel/theme';\n  static styles = [controlsCss, css``];", true],
    // Imported and never adopted is as dead as never imported.
    ["import { controlsCss } from '@nigel/theme';\n  static styles = css``;", false],
    ['  static styles = [controlsCss, css``];', false],
    ['  static styles = css``;', false],
  ])('detects adoption in %s as %s', (text, expected) => {
    expect(adoptsControls(text)).toBe(expected);
  });
});
