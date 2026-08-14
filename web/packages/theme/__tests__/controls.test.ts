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

/** The rules behind AC #1: a glow on hover and on focus-visible. */
describe('the button glow', () => {
  /** Rules with their comments stripped and their whitespace collapsed. */
  const rules = text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\s+/g, '');

  /** The one rule that draws a glow, and the selector it draws it for. */
  const applied = rules.match(/([^{}]+)\{box-shadow:var\(--nc-glow\)/);
  const selectors = applied?.[1] ?? '';

  /** Which token a variant points `--nc-glow` at. */
  const hueFor = (selector: string): string | undefined =>
    rules.match(new RegExp(`${selector}\\{--nc-glow:var\\((--nc-glow-[a-z]+)\\)`))?.[1];

  it('is applied by one rule, so every button is excluded on the same terms', () => {
    expect(applied).not.toBeNull();
  });

  it('draws on hover and on keyboard focus', () => {
    expect(selectors).toContain(':hover');
    expect(selectors).toContain(':focus-visible');
  });

  it('reaches the control through the host, which wa-button delegates focus to', () => {
    expect(selectors).toMatch(/::part\(base\)$/);
  });

  it.each([
    ['wa-button', '--nc-glow-neutral'],
    ["wa-button\\[variant='brand']", '--nc-glow-brand'],
    ["wa-button\\[variant='danger']", '--nc-glow-danger'],
    ["wa-button\\[variant='success']", '--nc-glow-success'],
    ["wa-button\\[variant='warning']", '--nc-glow-warning'],
  ])('gives %s a halo in its own hue', (selector, token) => {
    expect(hueFor(selector)).toBe(token);
  });

  it('excludes the three buttons that must not invite a click', () => {
    // A plain button is a row action drawn as bare text; a disabled one is
    // refusing; a loading one swallows the click in handleClick.
    expect(selectors).toContain(":not([appearance='plain'],[disabled],[loading])");
  });

  it('names no variant Web Awesome does not have', () => {
    // wa-button's variants are neutral, brand, success, warning and danger.
    expect(rules).not.toContain("variant='primary'");
  });

  it('declares no duration of its own', () => {
    // wa-button's base part already transitions box-shadow over
    // --wa-transition-fast, which the theme zeroes under reduced motion. A
    // transition written here would replace that whole list instead.
    expect(rules).not.toContain('transition');
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
