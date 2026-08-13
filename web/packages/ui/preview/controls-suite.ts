import { describe, it, expect } from 'vitest';
import type { CSSResult, LitElement } from 'lit';
import { controlsCss } from '@nigel/theme';

type StyledCtor = typeof LitElement;

/**
 * The flattened text of everything a component adopts into its shadow root.
 *
 * `static styles` is a `CSSResult`, an array, or an array of arrays, so it is
 * walked recursively. `Array.prototype.flat(Infinity)` would do it at runtime
 * but its return type is inferred by unrolling the nesting, which on lit's
 * recursive `CSSResultArray` exceeds TypeScript's instantiation depth.
 */
export function styleText(ctor: StyledCtor): string {
  const out: string[] = [];
  const walk = (node: unknown): void => {
    if (Array.isArray(node)) {
      node.forEach(walk);
    } else if (node) {
      out.push(String((node as CSSResult).cssText));
    }
  };
  walk(ctor.styles);
  return out.join('\n');
}

/**
 * Assert a component hosting a Web Awesome primitive adopts `controlsCss`.
 *
 * A `::part()` rule reaches exactly one shadow boundary down, from the tree the
 * rule is written in, so the sheet has to be adopted by the component that
 * *hosts* the primitive — an ancestor's copy is one boundary too far and the
 * document's is several. `rules` names the specific declarations that were
 * being shipped and never matching, so a regression says which surface broke
 * rather than only that a sheet went missing.
 */
export function describeControlsAdoption(
  ctor: StyledCtor,
  ...rules: (string | RegExp)[]
): void {
  describe('controlsCss adoption', () => {
    it('adopts the wa-* part overrides into its own shadow root', () => {
      expect(styleText(ctor)).toContain(controlsCss.cssText);
    });

    it.each(rules)('carries %s, which a document sheet could not deliver', (rule) => {
      const text = styleText(ctor);
      if (typeof rule === 'string') {
        expect(text).toContain(rule);
      } else {
        expect(text).toMatch(rule);
      }
    });
  });
}
