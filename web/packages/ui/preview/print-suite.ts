import { describe, it, expect } from 'vitest';
import type { LitElement } from 'lit';
import { styleText } from './controls-suite.js';

type StyledCtor = typeof LitElement;

/**
 * Assert a component hides its own chrome on paper.
 *
 * The print sheet used to do this from the document with
 * `wc-app-shell::part(sidebar)` and a list of `wc-*` tag names, none of which
 * could match: the shell and its neighbours live inside `nigel-app`'s shadow
 * root. Token repainting still rides the document sheet, because custom
 * properties inherit through boundaries — but hiding an element is a rule
 * about that element, so it has to live where the element does.
 *
 * `selectors` are the ones the component is responsible for. Naming them
 * rather than only checking for `@media print` is what makes a regression say
 * *which* piece of furniture came back onto the page.
 */
export function describePrintHiding(ctor: StyledCtor, ...selectors: string[]): void {
  describe('printing', () => {
    const text = styleText(ctor);

    it('carries its own @media print block', () => {
      expect(text).toContain('@media print');
    });

    it.each(selectors)('takes %s off the page', (selector) => {
      const rule = new RegExp(
        `${selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}[^{]*{[^}]*display:\\s*none`,
      );
      expect(text).toMatch(rule);
    });
  });
}
