import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { COLOR_MODE_STORAGE_KEY, DARK_CLASS, LIGHT_CLASS } from '@nigel/theme';

/**
 * `main.ts` is a module script, so it runs after the document is parsed, and
 * `index.html` paints `html, body` from the tokens before that. For anyone
 * whose stored choice differs from their OS setting, that is a visible flash
 * of the wrong palette on every single load.
 *
 * A blocking inline script in `<head>` fixes it, at the cost of writing the
 * storage key and the class-name convention a second time, in HTML, where no
 * compiler will notice them drifting from `color-mode.ts`. This test is what
 * notices.
 */
const here = dirname(fileURLToPath(import.meta.url));
const indexHtml = readFileSync(resolve(here, '../../index.html'), 'utf8');

const head = indexHtml.slice(0, indexHtml.indexOf('</head>'));
const bootstrap = head.slice(head.indexOf('<script'), head.indexOf('</script>'));

describe('the first-paint color mode bootstrap', () => {
  it('exists, in the head, before the module script', () => {
    expect(head).toContain('<script>');
    expect(indexHtml.indexOf('<script>')).toBeLessThan(
      indexHtml.indexOf('type="module"'),
    );
  });

  it('is blocking — async or defer would put the flash back', () => {
    expect(bootstrap).not.toMatch(/\basync\b/);
    expect(bootstrap).not.toMatch(/\bdefer\b/);
    expect(bootstrap).not.toContain('type="module"');
  });

  it('uses the same storage key the module does', () => {
    expect(bootstrap).toContain(COLOR_MODE_STORAGE_KEY);
  });

  it.each([LIGHT_CLASS, DARK_CLASS])('uses the same %s class the module does', (cls) => {
    expect(bootstrap).toContain(cls);
  });

  it('survives storage being disabled', () => {
    // localStorage throws on access in a browser refusing storage, and an
    // unguarded throw here is a blank page before anything has rendered.
    expect(bootstrap).toContain('try');
    expect(bootstrap).toContain('catch');
  });

  it('adds a class only for an explicit light or dark', () => {
    // The inline copy must make the same choice color-mode.ts makes, or the
    // first frame and every frame after it would disagree. Anything other than
    // the two explicit values means follow the OS, which is no class at all.
    expect(bootstrap).toMatch(/===\s*'light'/);
    expect(bootstrap).toMatch(/===\s*'dark'/);

    const adds = bootstrap.match(/classList\.add\(/g) ?? [];
    expect(adds).toHaveLength(2);
  });
});
