import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * `ExportTarget` is declared once in the api client and once again in
 * `@nigel/ui`'s `wc-export-links` — deliberately, since `@nigel/ui` must not
 * import from this app. Nothing else keeps the two declarations in step, so
 * this reads both off disk and fails the moment they drift.
 */
const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, '../../../../../');

const CLIENT_PATH = resolve(repoRoot, 'web/apps/app/src/api/client.ts');
const UI_PATH = resolve(repoRoot, 'web/packages/ui/src/components/wc-export-links.ts');

/**
 * The `export type ExportTarget = …;` statement, without its doc comment.
 *
 * A plain "stop at the first `;`" regex is wrong here: each union member is
 * an object type whose properties are themselves `;`-separated, so the
 * declaration's own terminator is only the semicolon that closes the last
 * brace, not the first semicolon met. This walks the text tracking brace
 * depth and stops at the `;` that follows depth returning to zero.
 */
function extractExportTarget(source: string, path: string): string {
  const start = source.indexOf('export type ExportTarget =');
  if (start === -1) {
    throw new Error(`could not find an ExportTarget declaration in ${path}`);
  }

  let depth = 0;
  for (let i = start; i < source.length; i++) {
    const char = source[i];
    if (char === '{') depth += 1;
    else if (char === '}') depth -= 1;
    else if (char === ';' && depth === 0) {
      return source.slice(start, i + 1);
    }
  }
  throw new Error(`ExportTarget declaration in ${path} never terminates with ';'`);
}

describe('ExportTarget seam', () => {
  it('is declared identically in the api client and in @nigel/ui', () => {
    const clientSource = readFileSync(CLIENT_PATH, 'utf8');
    const uiSource = readFileSync(UI_PATH, 'utf8');

    const clientRelPath = relative(repoRoot, CLIENT_PATH);
    const uiRelPath = relative(repoRoot, UI_PATH);

    const clientDecl = extractExportTarget(clientSource, clientRelPath);
    const uiDecl = extractExportTarget(uiSource, uiRelPath);

    expect(
      clientDecl,
      `ExportTarget has drifted between ${clientRelPath} and ${uiRelPath}:\n\n` +
        `${clientRelPath}:\n${clientDecl}\n\n${uiRelPath}:\n${uiDecl}`,
    ).toBe(uiDecl);
  });
});
