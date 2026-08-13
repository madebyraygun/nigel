import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * A `::part()` rule reaches one shadow boundary down, from the tree it is
 * written in. So the theme's treatment for Web Awesome primitives has to be
 * adopted by the component that *hosts* the primitive — nothing an ancestor or
 * the document does can reach it.
 *
 * That is a convention, and a convention the next component can forget is how
 * the whole `wa-*` treatment came to ship dead for as long as it did. This is
 * the same enforcement shape `apps/app/src/__tests__/api-seam.test.ts` uses for
 * the api client: scan the source, fail the build, name the file.
 *
 * The rule is deliberately blunt and has no exemption list. `wc-dropzone`
 * renders only `wa-format-bytes`, which exposes no part we style, and it adopts
 * the sheet anyway — an exemption list is a place for the next component to
 * hide.
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

  it('is present in every file that renders a wa-* primitive', () => {
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

  it('actually scans the components', () => {
    // Guards the guard: a walk that silently returned nothing would pass above.
    const hosts = sources.filter((f) => rendersPrimitive(readFileSync(f, 'utf8')));
    expect(sources.length).toBeGreaterThan(10);
    expect(hosts.length).toBeGreaterThan(10);
  });

  it.each([
    ["import '@awesome.me/webawesome/dist/components/dialog/dialog.js';", true],
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
