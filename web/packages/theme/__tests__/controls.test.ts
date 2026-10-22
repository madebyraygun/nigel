import { describe, it, expect } from 'vitest';
import { controlsCss } from '../src/controls.js';
import { nigelTheme } from '../src/themes/nigel.js';
import { SWITCH_CHECK, SWITCH_CHECK_PATH } from '../src/tokens/switch.js';

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
    "wa-button:is(:not([variant]), [variant='neutral']):not([appearance='plain'])::part(base)",
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
 * The handoff's two focus treatments: a field is ringed — brand border and a
 * 3px translucent halo — and everything else takes a 2px brand outline on
 * keyboard focus only.
 */
describe('focus', () => {
  const rules = text.replace(/\/\*[\s\S]*?\*\//g, '');
  const block = (selector: string): string => {
    const at = rules.indexOf(selector);
    expect(at, `no rule for ${selector}`).toBeGreaterThanOrEqual(0);
    return rules.slice(rules.indexOf('{', at), rules.indexOf('}', at) + 1);
  };

  it('outlines a focused control in brand, clear of its edge, on keyboard focus only', () => {
    const outline = block('\n  :focus-visible');
    expect(outline).toMatch(/outline:\s*2px solid var\(--wa-color-focus\)/);
    expect(outline).toMatch(/outline-offset:\s*2px/);
  });

  it.each(['select:focus', 'textarea:focus', "[type='color']):focus"])(
    'rings a native field: %s',
    (selector) => {
      const ring = block(selector);
      expect(ring).toMatch(/border-color:\s*var\(--wa-color-brand\)/);
      expect(ring).toMatch(/box-shadow:\s*0 0 0 3px var\(--nc-color-ring\)/);
      // Forced colors drops the halo and repaints a transparent outline in
      // the system colour; `none` would leave a focused field unmarked.
      expect(ring).toMatch(/outline:\s*2px solid transparent/);
      expect(ring).not.toMatch(/outline:\s*none/);
    },
  );

  it('marks a focused field in forced-colors mode', () => {
    const forced = rules.slice(rules.indexOf('@media (forced-colors: active)'));
    expect(rules).toContain('@media (forced-colors: active)');
    for (const selector of ['textarea:focus', 'select:focus', 'wa-input:focus-within::part(base)']) {
      expect(forced).toContain(selector);
    }
    expect(forced).toMatch(/outline:\s*2px solid Highlight/);
  });

  it('never takes the outline off a field', () => {
    expect(rules).not.toMatch(/outline:\s*none/);
  });

  it('leaves checkboxes, radios and file pickers to the outline', () => {
    expect(rules).toMatch(
      /input:not\(\[type='checkbox'\], \[type='radio'\], \[type='file'\][^)]*\):focus/,
    );
  });

  it.each([
    'wa-input:focus-within::part(base)',
    'wa-textarea:focus-within::part(base)',
    'wa-select:focus-within::part(combobox)',
    "wa-select[open]::part(combobox)",
  ])('rings %s through the part that carries its border', (selector) => {
    expect(rules).toContain(selector);
    const ring = block("wa-select[open]::part(combobox)");
    expect(ring).toMatch(/border-color:\s*var\(--wa-color-brand\)/);
    expect(ring).toMatch(/box-shadow:\s*0 0 0 3px var\(--nc-color-ring\)/);
    expect(ring).toMatch(/outline-color:\s*transparent/);
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

/**
 * TASK-154.15: every wa-button is the refresh's pill, and this sheet is the
 * one place that draws it.
 */
describe('the standard button', () => {
  /** Rules with their comments stripped and their whitespace collapsed. */
  const rules = text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\s+/g, '');

  /** The declarations of the first rule whose selector is exactly `selector`. */
  const body = (selector: string): string => {
    const head = `${selector}{`;
    const at = rules.startsWith(head) ? 0 : rules.indexOf(`}${head}`) + 1;
    expect(at, `no rule for ${selector}`).toBeGreaterThan(rules.startsWith(head) ? -1 : 0);
    const open = at + head.length;
    return rules.slice(open, rules.indexOf('}', open));
  };

  it('is a pill at the 32px control height with 13px type', () => {
    const base = body('wa-button');
    expect(base).toContain('--wa-form-control-height:var(--nc-button-height)');
    expect(base).toContain('--wa-form-control-border-radius:var(--wa-radius-pill)');
    expect(base).toContain('border-radius:var(--wa-radius-pill)');
    expect(base).toContain('font-size:var(--nc-font-size-control)');
  });

  it('is 38px at size=l and 28px at size=s', () => {
    expect(body("wa-button[size='l']")).toContain(
      '--wa-form-control-height:var(--nc-button-height-lg)',
    );
    expect(body("wa-button[size='s']")).toContain(
      '--wa-form-control-height:var(--nc-button-height-sm)',
    );
  });

  it('sets a filled button in 500 and an outline at the body weight', () => {
    expect(body('wa-button')).toContain('--wa-font-weight-action:var(--wa-font-weight-normal)');
    expect(
      body(
        "wa-button:is([variant='brand'],[variant='danger']):is(:not([appearance]),[appearance='accent'])",
      ),
    ).toContain('--wa-font-weight-action:var(--wa-font-weight-medium)');
  });

  it('fills brand flat, with no gradient and no drift', () => {
    // The fill itself is Web Awesome's accent read through the brand family
    // (wa-contract.ts); nothing here paints over it.
    expect(rules).not.toContain('--nc-grad-brand');
    expect(rules).not.toContain('animation');
    expect(rules).not.toContain('@keyframes');
    expect(rules).toMatch(
      /wa-button\[variant='brand'\][^{}]*:hover::part\(base\)\{background-color:var\(--wa-color-brand-hover\);?\}/,
    );
  });

  it('draws neutral as an outline in the border colour on the card ground', () => {
    const neutral = body(
      "wa-button:is(:not([variant]),[variant='neutral']):not([appearance='plain'])::part(base)",
    );
    expect(neutral).toContain('background-color:var(--wa-color-surface)');
    expect(neutral).toContain('border-color:var(--wa-color-border)');
    expect(neutral).toContain('color:var(--wa-color-text)');
  });

  it('dims a disabled button to 40%', () => {
    expect(body('wa-button[disabled]')).toBe('opacity:0.4;');
  });

  it('changes nothing on hover for a disabled or loading button', () => {
    const hovers = [...rules.matchAll(/([^{}]+):hover::part\(base\)\{/g)].map((m) => m[1]);
    expect(hovers.length).toBeGreaterThan(0);
    for (const selector of hovers) {
      expect(selector).toMatch(/:not\(\[disabled\],\[loading\]\)/);
    }
  });

  it('makes an icon-only button a circle the button height across', () => {
    const icon = body('wa-button:has(>[label]:not([slot]))');
    expect(icon).toContain('--wa-form-control-padding-inline:0');
    expect(icon).toContain('width:var(--wa-form-control-height)');
    expect(icon).toContain('--nc-icon-size:var(--nc-button-icon-size)');
  });

  it('keeps a focus outline and an edge under forced colors', () => {
    const forced = rules.slice(rules.indexOf('@media(forced-colors:active)'));
    expect(forced).toContain('wa-button::part(base){border-color:ButtonBorder;}');
    expect(forced).toContain(
      'wa-button::part(base):focus-visible{outline:2pxsolidHighlight;outline-offset:2px;}',
    );
  });

  it('draws the 2px focus ring Web Awesome reads from the tokens', () => {
    // wa-button's .button:focus-visible reads --wa-focus-ring and its offset.
    const composedRules = composed.replace(/\s+/g, ' ');
    expect(composedRules).toMatch(/--wa-focus-ring-width: 2px/);
    expect(composedRules).toMatch(/--wa-focus-ring-offset: 2px/);
    expect(composedRules).toMatch(/--wa-focus-ring: var\(--wa-focus-ring-style\) var\(--wa-focus-ring-width\) var\(--wa-color-focus\)/);
  });

  it('has no opt-in attributes: one button, not a family of them', () => {
    expect(rules).not.toMatch(/wa-button\[(pill|solid)\]/);
  });
});

/** TASK-154.16: every wa-switch is the theme's switch. */
describe('the standard switch', () => {
  const rules = text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\s+/g, '');
  const body = (selector: string): string => {
    const at = rules.indexOf(`}${selector}{`);
    expect(at, `no rule for ${selector}`).toBeGreaterThan(-1);
    const open = at + selector.length + 2;
    return rules.slice(open, rules.indexOf('}', open));
  };

  it('is 38×22 with a 16px knob', () => {
    const host = body('wa-switch');
    expect(host).toContain('--width:var(--nc-switch-width)');
    expect(host).toContain('--height:var(--nc-switch-height)');
    expect(host).toContain('--thumb-size:var(--nc-switch-knob-size)');
  });

  it('is a pill whose off edge and track come from the tokens', () => {
    const track = body('wa-switch::part(control)');
    expect(track).toContain('border-radius:var(--wa-radius-pill)');
    expect(track).toContain('border:1pxsolidvar(--nc-color-switch-border)');
    expect(track).toContain('background-color:var(--wa-color-surface-alt)');
    expect(track).toContain('box-sizing:border-box');
  });

  it('slides on the 150ms switch transition, which reduced motion zeroes', () => {
    expect(body('wa-switch::part(control)')).toContain('translatevar(--nc-transition-switch)');
    const composedRules = composed.replace(/\s+/g, ' ');
    expect(composedRules).toContain('--nc-transition-switch: 150ms ease');
    const reduced = composedRules.slice(composedRules.indexOf('prefers-reduced-motion'));
    expect(reduced).toContain('--nc-transition-switch: 0ms linear');
  });

  it('gives the white knob the same edge when off', () => {
    const knob = body('wa-switch::part(thumb)');
    expect(knob).toContain('border:1pxsolidvar(--nc-color-switch-border)');
    expect(knob).toContain('background-color:var(--nc-color-switch-knob)');
  });

  it('fills the track with the brand and checks the knob when on', () => {
    expect(body('wa-switch:state(checked)::part(control)')).toContain(
      'background-color:var(--wa-color-brand)',
    );
    const knob = body('wa-switch:state(checked)::part(thumb)');
    expect(knob).toContain('background-image:var(--nc-switch-check-image)');
    expect(knob).toContain('border-color:var(--nc-color-switch-check)');
  });

  it('dims a disabled switch to 40% on the part Web Awesome dims', () => {
    expect(body('wa-switch[disabled]::part(base)')).toBe('opacity:0.4;');
  });

  it('keeps an edge and a focus outline under forced colors', () => {
    const forced = rules.slice(rules.indexOf('wa-switch::part(control),wa-switch::part(thumb){border-color:ButtonText'));
    expect(forced.length).toBeGreaterThan(0);
    expect(forced).toContain(
      'wa-switch:focus-within::part(thumb){outline:2pxsolidHighlight;outline-offset:2px;}',
    );
    // The on state in system colours: a solid knob rather than the
    // brand-inked check image.
    expect(forced).toContain(
      'wa-switch:state(checked)::part(thumb){background-color:ButtonText;background-image:none;}',
    );
  });

  it('draws the check image from the icon set’s path in the check colour', () => {
    const image = composed.match(/--nc-switch-check-image:\s*url\("data:image\/svg\+xml,([^"]+)"\)/);
    expect(image).not.toBeNull();
    const svg = decodeURIComponent(image![1]);
    expect(svg).toContain(`d='${SWITCH_CHECK_PATH}'`);
    expect(svg).toContain(`stroke='${SWITCH_CHECK}'`);
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
