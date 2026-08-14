import { describe, it, expect } from 'vitest';
import { resolvedBoxFromCss, isColumn, withoutComments } from './layout-suite.js';

describe('resolvedBoxFromCss', () => {
  it('reads the box a selector asks for', () => {
    const box = resolvedBoxFromCss(':host { display: flex; flex-direction: column; flex: 1 1 auto; }');
    expect(isColumn(box)).toBe(true);
    expect(box.flexGrow).toBe('1');
  });

  it('answers for a longhand the stylesheet only set through a shorthand', () => {
    const box = resolvedBoxFromCss(':host { display: flex; flex-flow: column wrap; }');
    expect(box.flexDirection).toBe('column');
    expect(isColumn(box)).toBe(true);
  });

  it('centres from place-content as well as from align-content', () => {
    expect(resolvedBoxFromCss(':host { place-content: center; }').blockAlignment).toBe(
      'center',
    );
    expect(resolvedBoxFromCss(':host { align-content: center; }').blockAlignment).toBe(
      'center',
    );
  });

  it('lets a later rule for the same selector win, as the cascade does', () => {
    const css = ':host { display: flex; flex-direction: column; } :host { display: block; }';
    expect(resolvedBoxFromCss(css).display).toBe('block');
    expect(isColumn(resolvedBoxFromCss(css))).toBe(false);
  });

  it('reads a rule that follows a blockless at-statement', () => {
    // `@import` ends at its semicolon. Assuming every `@` opens a block would
    // swallow the rule after it and report the component as having none.
    const css = "@import 'tokens.css';\n:host { display: flex; flex-direction: column; }";
    expect(isColumn(resolvedBoxFromCss(css))).toBe(true);
  });

  it('reads a rule that follows an at sign inside a declaration value', () => {
    // No trailing semicolon, which is where a scanner that looks for the next
    // `{` after an `@` runs on into the following rule and swallows it.
    const css = ".mark::before { content: '@' }\n:host { display: flex; flex-direction: column; }";
    expect(isColumn(resolvedBoxFromCss(css))).toBe(true);
  });

  it('leaves the rules of another medium out of the screen box', () => {
    const css =
      ':host { display: flex; flex-direction: column; } @media print { :host { display: block; } }';
    expect(resolvedBoxFromCss(css).display).toBe('flex');
  });

  it('throws when the selector has no rule at all', () => {
    expect(() => resolvedBoxFromCss('.other { display: flex; }')).toThrow(':host');
  });
});

describe('withoutComments', () => {
  it('takes a comment off the front of a selector', () => {
    const css = '/* the frame */ :host { display: flex; flex-direction: column; }';
    expect(withoutComments(css).trim().startsWith(':host')).toBe(true);
    expect(isColumn(resolvedBoxFromCss(css))).toBe(true);
  });
});
