import { describe, it, expect } from 'vitest';
import { controlsCss } from '../src/controls.js';
import { nigelTheme } from '../src/themes/nigel.js';

const text = controlsCss.cssText;
const composed = nigelTheme.cssText;

/**
 * `controlsCss` is the half of the old `global.ts` that only works *inside* a
 * shadow root. A `::part()` rule reaches one boundary down from the tree it is
 * written in, and every `wa-*` primitive in this app lives inside a `wc-*`
 * shadow root inside a screen inside `nigel-app` — so the document sheet is
 * three boundaries too far away and these rules have to be adopted instead.
 */
describe('controlsCss', () => {
  it.each([
    'wa-dialog::part(header)',
    'wa-dialog::part(body)',
    'wa-dialog::part(footer)',
    "wa-button[variant='brand']::part(base)",
    '::part(label)',
    ':focus-visible',
  ])('carries %s', (rule) => {
    expect(text).toContain(rule);
  });

  it.each([
    'wa-input::part(base)',
    'wa-select::part(base)',
    'wa-textarea::part(base)',
    '::part(form-control-label)',
  ])('leaves %s to the tokens', (rule) => {
    // Field chrome is --wa-form-control-* now. A part rule for it would also
    // override Web Awesome's disabled and appearance variants, because an
    // outer-tree ::part() rule wins over the shadow tree's own for the same
    // property regardless of specificity.
    expect(text).not.toContain(rule);
  });

  it('reads only tokens, never a literal brand value', () => {
    expect(text).not.toMatch(/#[0-9a-f]{6}/i);
  });
});

describe('nigelTheme', () => {
  it('ships no ::part() rule at all — a document sheet cannot reach one', () => {
    expect(composed).not.toContain('::part(');
  });

  it('does not carry controlsCss, which only works once adopted', () => {
    expect(composed).not.toContain(text);
  });
});
