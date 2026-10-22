import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { cssBlocks, splitSelectorList } from '../../preview/button-guard.js';

/**
 * Every table heading is set at one weight, `--nc-font-weight-table-heading`.
 * A component that sets a `th`'s weight itself — a literal, or another token —
 * is how the tables drifted to four different weights; this fails the build on
 * it. Row bands that are not headings in that sense (a `tfoot` or total row,
 * a cell selector shared with `td`) keep their own weights.
 */
const here = dirname(fileURLToPath(import.meta.url));
const componentsDir = resolve(here, '../components');

const TOKEN = 'var(--nc-font-weight-table-heading)';

/** A selector that is a table heading and nothing else. */
function isHeading(selector: string): boolean {
  return (
    /(^|[\s>])th(?![a-z-])/.test(selector) &&
    !/(^|[\s>,])td(?![a-z-])|tfoot|tr\.|scope='row'|scope="row"/.test(selector)
  );
}

export function headingWeights(source: string): string[] {
  const out: string[] = [];
  for (const block of cssBlocks(source)) {
    for (const [, list, declarations] of block.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      const weight = declarations.match(/font-weight:\s*([^;]+);/)?.[1]?.trim();
      if (!weight) continue;
      for (const selector of splitSelectorList(list.replace(/\s+/g, ' '))) {
        if (isHeading(selector) && weight !== TOKEN) out.push(`${selector} { ${weight} }`);
      }
    }
  }
  return out;
}

describe('table headings', () => {
  const sources = readdirSync(componentsDir)
    .filter((name) => name.endsWith('.ts') && !name.endsWith('.test.ts'))
    .map((name) => join(componentsDir, name));

  it('set their weight from the one token', () => {
    const offenders = sources.flatMap((file) =>
      headingWeights(readFileSync(file, 'utf8')).map(
        (offence) => `${relative(componentsDir, file)}: ${offence}`,
      ),
    );
    expect(offenders, `set a table heading's weight with ${TOKEN}`).toEqual([]);
  });

  it('is read by every table family', () => {
    const readers = sources.filter((file) => readFileSync(file, 'utf8').includes(TOKEN));
    for (const table of [
      'wc-manager-table.ts',
      'wc-invoice-table.ts',
      'wc-document-table.ts',
      'wc-register-table.ts',
      'wc-report-table.ts',
      'wc-line-items.ts',
    ]) {
      expect(readers.map((file) => relative(componentsDir, file))).toContain(table);
    }
  });

  it.each([
    ['th { font-weight: var(--wa-font-weight-medium); }', 1],
    ['thead th { font-weight: 600; }', 1],
    ['th.end, td.end { font-weight: 600; }', 1],
    [`th { font-weight: ${TOKEN}; }`, 0],
    ['tfoot th, tfoot td { font-weight: var(--wa-font-weight-semibold); }', 0],
    ["th[scope='row'], td { font-weight: var(--wa-font-weight-normal); }", 0],
  ])('judges %s', (rules, count) => {
    expect(headingWeights(`css\`${rules}\``)).toHaveLength(count);
  });
});
