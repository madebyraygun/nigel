import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { controlsCss } from '../src/controls.js';
import { resolveToken, type Mode } from './token-resolution.js';

/**
 * What a `wa-button[variant='danger']` actually paints.
 *
 * Web Awesome colours a variant in two hops: `variants.styles` points the
 * generic `--wa-color-fill-loud` at the family token `--wa-color-danger-fill-loud`,
 * and the component reads the generic one. The theme owns the second hop — and
 * a family it never defines is not a default but nothing, so the declaration is
 * discarded and the button falls back to the neutral fill. That is how every
 * delete confirmation came to render its destructive action as a grey button.
 *
 * The mapping is read out of the installed package rather than written down
 * here, for `wa-contract.test.ts`'s reason: the failure being guarded against
 * is upstream changing what it reads while a hand-copied list stays green.
 */
const here = dirname(fileURLToPath(import.meta.url));
const chunkDir = resolve(here, '../../../node_modules/@awesome.me/webawesome/dist/chunks');

const MODES: Mode[] = ['light', 'dark'];

/** variant -> the generic token -> the family token it is pointed at. */
function variantIndirection(): Map<string, Map<string, string>> {
  const map = new Map<string, Map<string, string>>();

  for (const file of readdirSync(chunkDir).filter((f) => f.endsWith('.js'))) {
    const text = readFileSync(join(chunkDir, file), 'utf8');
    if (!text.includes('--wa-color-fill-loud: var(--wa-color-')) continue;

    for (const [, variant, body] of text.matchAll(
      /:host\(\[variant='([a-z]+)']\)\s*\{([^}]*)\}/g,
    )) {
      const roles = map.get(variant) ?? new Map<string, string>();
      for (const [, generic, family] of body.matchAll(
        /(--wa-color-(?:fill|on|border)-(?:loud|normal|quiet)):\s*var\((--wa-color-[a-z-]+)\)/g,
      )) {
        roles.set(generic, family);
      }
      map.set(variant, roles);
    }
  }
  return map;
}

describe('the wa-button variant families', () => {
  const indirection = variantIndirection();
  const neutral = indirection.get('neutral');

  it('reads the indirection out of the installed package', () => {
    // Guards the guard: a moved chunk layout would find no variants at all and
    // make every assertion below vacuously true.
    expect([...indirection.keys()].sort()).toEqual([
      'brand',
      'danger',
      'neutral',
      'success',
      'warning',
    ]);
    for (const [variant, roles] of indirection) {
      expect(roles.size, variant).toBe(9);
    }
  });

  /**
   * The outcome, not the declaration: whatever the sheet says, the colour a
   * danger button ends up filled with has to differ from the neutral one, or
   * the variant is invisible to the person looking at the dialog.
   */
  it.each(['brand', 'danger', 'success', 'warning'])(
    'fills a %s button with a colour of its own, in both modes',
    (variant) => {
      const family = indirection.get(variant)!.get('--wa-color-fill-loud')!;
      const neutralFamily = neutral!.get('--wa-color-fill-loud')!;

      for (const mode of MODES) {
        expect(resolveToken(family, mode), `${variant} in ${mode}`).not.toBe(
          resolveToken(neutralFamily, mode),
        );
      }
    },
  );

  it.each(['danger', 'success', 'warning'])(
    'fills a %s button with that semantic colour itself',
    (variant) => {
      const family = indirection.get(variant)!.get('--wa-color-fill-loud')!;
      for (const mode of MODES) {
        expect(resolveToken(family, mode), mode).toBe(resolveToken(`--wa-color-${variant}`, mode));
      }
    },
  );

  it.each(['danger', 'success', 'warning'])(
    'tints the quiet and normal fills of a %s button toward that colour',
    (variant) => {
      // The washes behind an outlined button's hover and a filled one's
      // background. A neutral grey there reads as a different control.
      for (const mode of MODES) {
        const surface = resolveToken('--wa-color-surface', mode);
        for (const role of ['--wa-color-fill-quiet', '--wa-color-fill-normal']) {
          const family = indirection.get(variant)!.get(role)!;
          expect(resolveToken(family, mode), `${role} in ${mode}`).not.toBe(surface);
        }
      }
    },
  );

  it.each(['danger', 'success', 'warning'])(
    'draws a %s button its hover edge from the colour it is filled with',
    (variant) => {
      // The edge and the fill have to be the same hue or a hovered danger
      // button wears somebody else's outline. Both sides read the one token,
      // so the pairing holds in either mode without a second declaration.
      const family = indirection.get(variant)!.get('--wa-color-fill-loud')!;
      const edge = controlsCss.cssText.match(
        new RegExp(`wa-button\\[variant='${variant}'\\]\\s*\\{\\s*--nc-hover-border:\\s*([^;]+);`),
      );

      expect(edge?.[1]).toBe(`var(--wa-color-${variant})`);
      for (const mode of MODES) {
        expect(resolveToken(family, mode)).toBe(resolveToken(`--wa-color-${variant}`, mode));
      }
    },
  );

  it('leaves the brand button its gradient', () => {
    // controlsCss paints --nc-grad-brand over ::part(base), and an outer-tree
    // part rule wins over the shadow tree's own for the same property. The
    // brand family below it is what the outlined and plain appearances show.
    const family = indirection.get('brand')!.get('--wa-color-fill-loud')!;
    for (const mode of MODES) {
      expect(resolveToken(family, mode)).toBe(resolveToken('--wa-color-brand', mode));
    }
  });
});
