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

/**
 * The document turns off user-select so the desktop shell does not paint a
 * selection while dragging its chrome, and that choice inherits through
 * every shadow boundary underneath it — including into a field someone is
 * typing in. This is the one place that reaches every wa-input in the app
 * without every component restating it.
 */
describe('field selection', () => {
  const rules = text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\s+/g, '');
  const selectors =
    rules.match(/([^{}]+)\{user-select:text;?\}/)?.[1] ?? '';

  it.each(['input', 'textarea', '[contenteditable]', 'wa-input'])(
    'restores user-select: text on %s',
    (selector) => {
      expect(selectors.split(',')).toContain(selector);
    },
  );
});

/** The rules behind AC #1: an edge on hover and on focus-visible. */
describe('the button hover edge', () => {
  /** Rules with their comments stripped and their whitespace collapsed. */
  const rules = text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\s+/g, '');

  /** The one rule that draws the edge, and the selector it draws it for. */
  const applied = rules.match(/([^{}]+)\{border-color:var\(--nc-hover-border\)/);
  const selectors = applied?.[1] ?? '';

  /** Which colour a variant points `--nc-hover-border` at. */
  const edgeFor = (selector: string): string | undefined =>
    rules.match(new RegExp(`${selector}\\{--nc-hover-border:var\\((--wa-color-[a-z]+)\\)`))?.[1];

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
    ['wa-button', '--wa-color-text'],
    ["wa-button\\[variant='brand']", '--wa-color-brand'],
    ["wa-button\\[variant='danger']", '--wa-color-danger'],
    ["wa-button\\[variant='success']", '--wa-color-success'],
    ["wa-button\\[variant='warning']", '--wa-color-warning'],
  ])('gives %s an edge in its own hue', (selector, token) => {
    expect(edgeFor(selector)).toBe(token);
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

  it('draws no halo, which the edge replaced', () => {
    expect(rules).not.toContain('--nc-glow');
  });

  it('restates the whole of the transition it had to override', () => {
    // A transition declared out here replaces wa-button's own list rather
    // than adding to it, so every property WA transitions on the base part is
    // named again. Anything dropped from this list stops transitioning on
    // every button in the app, silently.
    const declared = rules.match(/wa-button::part\(base\)\{transition:([^;]+);/)?.[1] ?? '';
    for (const property of ['background', 'color', 'opacity', 'transform']) {
      expect(declared).toContain(`${property}var(--wa-transition-fast)`);
    }
    // Both halves of the edge fade together, or the second pixel arrives
    // 380ms after the first.
    expect(declared).toContain('border-colorvar(--nc-duration-slow)');
    expect(declared).toContain('box-shadowvar(--nc-duration-slow)');
  });

  it('draws the second pixel inward, so hovering moves nothing', () => {
    // Widening the border would grow the box and shove every neighbour along.
    // An inset shadow is clipped to the padding edge, so it lands flush inside
    // the border and the two read as one --wa-border-width-m edge.
    // Anchored on the edge's own declaration: the brand drift's selector ends
    // the same way and would otherwise match first.
    const rule = rules.match(/\{border-color:var\(--nc-hover-border\);([^{}]+)\}/)?.[1] ?? '';
    expect(rule).toContain('box-shadow:inset000');
    expect(rule).toContain('calc(var(--wa-border-width-m)-var(--wa-form-control-border-width))');
    // The property, not the tokens the calc reads: setting it is the layout
    // shift this test exists to prevent.
    expect(rule).not.toContain('border-width:');
  });
});

/** The hover treatment for the one button with a gradient to move. */
describe('the brand button drift', () => {
  const rules = text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\s+/g, '');

  it('scrolls the ramp rather than recolouring it', () => {
    // Same seven stops: the image is periodic and the animation shifts it by
    // exactly one period. Nothing here rotates a hue.
    expect(rules).toContain('animation:nc-brand-cycle2.4slinearinfinite');
    expect(rules).not.toContain('hue-rotate');
  });

  it('sizes the base part for the period the keyframes step through', () => {
    // --nc-grad-brand is periodic and only reads correctly at this size; a
    // background shorthand on hover would reset it, which is why the hover
    // rule sets background-image alone.
    expect(rules).toContain('background-size:var(--nc-grad-brand-size)');
    expect(rules).toContain('background-image:var(--nc-grad-brand-hover)');
  });

  it('carries the keyframes it names, which a document sheet may not reach', () => {
    expect(rules).toContain('@keyframesnc-brand-cycle');
  });

  it('is the brand button alone', () => {
    const animated = rules.match(/([^{}]+)\{animation:nc-brand-cycle/)?.[1] ?? '';
    expect(animated).toContain("wa-button[variant='brand']");
  });

  it('stops moving under a reduced-motion preference', () => {
    const reduced = rules.slice(rules.indexOf('prefers-reduced-motion'));
    expect(reduced).toMatch(/wa-button\[variant='brand'][^{}]*\{animation:none/);
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
