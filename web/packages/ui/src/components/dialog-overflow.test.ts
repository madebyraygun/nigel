import { describe, it, expect } from 'vitest';
import { controlsCss } from '@nigel/theme';
import { styleText } from '../../preview/controls-suite.js';
import { withoutComments } from '../../preview/layout-suite.js';
import { WcManagerDialog } from './wc-manager-dialog.js';
import { WcAccountForm } from './wc-account-form.js';
import { WcCategoryForm } from './wc-category-form.js';
import { WcRuleForm } from './wc-rule-form.js';
import { WcClientForm } from './wc-client-form.js';
import { WcCategoryPicker } from './wc-category-picker.js';
import { WcRuleTestPreview } from './wc-rule-test-preview.js';

/**
 * Nothing in a manager dialog may be wider than the dialog.
 *
 * jsdom has no layout engine, so `scrollWidth` and `clientWidth` are zero for
 * every element and a measured check would pass whatever the CSS said. What
 * pushes a form past its dialog is a handful of declarations, though, and
 * those can be read: a fixed `min-width`, a grid track that cannot shrink
 * below its content (a bare `1fr`, whose minimum is `auto`), a
 * `width: 100%` that adds its padding and border on top, and a fieldset,
 * whose minimum width is its content's unless it is told otherwise. Each one
 * is refused here, in every component that renders inside a manager dialog.
 */
const COMPONENTS = [
  WcManagerDialog,
  WcAccountForm,
  WcCategoryForm,
  WcRuleForm,
  WcClientForm,
  WcCategoryPicker,
  WcRuleTestPreview,
];

interface Rule {
  selector: string;
  body: string;
}

/** The component's own rules: what it adopts from controlsCss is not its to answer for. */
function ownRules(ctor: (typeof COMPONENTS)[number]): Rule[] {
  const text = withoutComments(styleText(ctor).replace(controlsCss.cssText, ''));
  const rules: Rule[] = [];
  const pattern = /([^{}]+)\{([^{}]*)\}/g;
  for (const match of text.matchAll(pattern)) {
    rules.push({ selector: match[1].trim(), body: match[2] });
  }
  return rules;
}

function declarations(body: string): [string, string][] {
  return body
    .split(';')
    .map((part) => part.trim())
    .filter((part) => part.includes(':'))
    .map((part) => {
      const at = part.indexOf(':');
      return [part.slice(0, at).trim(), part.slice(at + 1).trim()];
    });
}

/** `fr` tracks outside a `minmax()`, whose minimum is their content's. */
function unshrinkableTracks(value: string): boolean {
  let depth = 0;
  let outside = '';
  let i = 0;
  while (i < value.length) {
    if (value.startsWith('minmax(', i)) {
      depth += 1;
      i += 'minmax('.length;
      continue;
    }
    const char = value[i];
    if (depth > 0 && char === '(') depth += 1;
    else if (depth > 0 && char === ')') depth -= 1;
    else if (depth === 0) outside += char;
    i += 1;
  }
  return /\d*\.?\d+fr\b/.test(outside);
}

describe('manager dialog contents fit the dialog', () => {
  it('recognises a track that cannot shrink', () => {
    expect(unshrinkableTracks('1fr 1fr')).toBe(true);
    expect(unshrinkableTracks('auto 1fr auto')).toBe(true);
    expect(unshrinkableTracks('minmax(0, 1fr) auto')).toBe(false);
    expect(unshrinkableTracks('repeat(auto-fit, minmax(min(100%, 12rem), 1fr))')).toBe(false);
    expect(unshrinkableTracks('max-content minmax(0, 1fr)')).toBe(false);
  });

  for (const ctor of COMPONENTS) {
    describe(ctor.name, () => {
      const rules = ownRules(ctor);

      it('sets no minimum width above zero', () => {
        const offenders = rules.flatMap(({ selector, body }) =>
          declarations(body)
            .filter(([name]) => name === 'min-width' || name === 'min-inline-size')
            .filter(([, value]) => value !== '0' && value !== '0px')
            .map(([name, value]) => `${selector} { ${name}: ${value} }`),
        );
        expect(offenders).toEqual([]);
      });

      it('lays grid columns out on tracks that can shrink', () => {
        const offenders = rules.flatMap(({ selector, body }) =>
          declarations(body)
            .filter(([name]) => name === 'grid-template-columns')
            .filter(([, value]) => unshrinkableTracks(value))
            .map(([, value]) => `${selector} { grid-template-columns: ${value} }`),
        );
        expect(offenders).toEqual([]);
      });

      it('sizes a full-width box to include its padding and border', () => {
        const offenders = rules
          .filter(({ body }) =>
            declarations(body).some(
              ([name, value]) => (name === 'width' || name === 'inline-size') && value === '100%',
            ),
          )
          .filter(
            ({ body }) =>
              !declarations(body).some(
                ([name, value]) => name === 'box-sizing' && value === 'border-box',
              ),
          )
          .map(({ selector }) => selector);
        expect(offenders).toEqual([]);
      });

      it('lets a fieldset shrink below its content', () => {
        const fieldsets = rules.filter(({ selector }) => /(^|[\s,>])fieldset\b/.test(selector));
        for (const { selector, body } of fieldsets) {
          const shrinks = declarations(body).some(
            ([name, value]) =>
              (name === 'min-inline-size' || name === 'min-width') && /^0(px)?$/.test(value),
          );
          expect(shrinks, selector).toBe(true);
        }
      });
    });
  }
});
