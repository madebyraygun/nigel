import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * Every colour in `@nigel/ui` comes from `@nigel/theme`. A literal — even as a
 * `var()` fallback — is a second copy of a brand value that the theme cannot
 * move, and a fallback that disagrees with the token it stands behind is what
 * renders the day the token is renamed. Scan the source, fail the build, name
 * the line.
 *
 * Test files are exempt: they assert on colours rather than paint them.
 */
const here = dirname(fileURLToPath(import.meta.url));
const srcDir = resolve(here, '../');

const COLOUR = /#[0-9a-fA-F]{6}\b|#[0-9a-fA-F]{8}\b|#[0-9a-fA-F]{3}\b|\brgba?\(|\bhsla?\(/;

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === 'node_modules' || entry.name === 'dist' || entry.name === '__tests__') {
        continue;
      }
      out.push(...walk(full));
    } else if (entry.name.endsWith('.ts') && !entry.name.endsWith('.test.ts')) {
      out.push(full);
    }
  }
  return out;
}

describe('@nigel/ui carries no inline colours', () => {
  const files = walk(srcDir);

  it('finds the component sources', () => {
    expect(files.length).toBeGreaterThan(20);
  });

  it('paints every colour from a theme token', () => {
    const hits = files.flatMap((file) =>
      readFileSync(file, 'utf8')
        .split('\n')
        .map((line, i) => [line, i + 1] as const)
        .filter(([line]) => COLOUR.test(line))
        .map(([line, n]) => `${relative(srcDir, file)}:${n}: ${line.trim()}`),
    );
    expect(hits).toEqual([]);
  });
});
