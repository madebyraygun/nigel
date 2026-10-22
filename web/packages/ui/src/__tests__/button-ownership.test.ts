import { describe, it, expect } from 'vitest';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { buttonStyling, describeButtonOwnership } from '../../preview/button-guard.js';

const here = dirname(fileURLToPath(import.meta.url));

describeButtonOwnership(resolve(here, '../'));

describe('buttonStyling', () => {
  const sheet = (rules: string): string => `static styles = css\`${rules}\`;`;

  it.each([
    ['a wa-button radius', 'wa-button { border-radius: 8px; }'],
    ['a wa-button part radius', 'wa-button::part(base) { border-top-left-radius: 0; }'],
    ['a wa-button height', '.actions wa-button { height: 40px; }'],
    ['a wa-button control height', 'wa-button[pill] { --wa-form-control-height: 38px; }'],
    ['a wa-button fill', "wa-button[variant='brand']::part(base) { background: red; }"],
    ['a wa-button family override', 'wa-button { --wa-color-fill-loud: transparent; }'],
    ['a brand-filled native button', 'button.primary { background: var(--wa-color-brand); }'],
    [
      'a brand-filled native button in a list',
      '.row, .actions button { color: x; background-color: var(--wa-color-brand); }',
    ],
    ['a wa-switch size', 'wa-switch { --height: 22px; --width: 38px; }'],
    ['a wa-switch track fill', 'wa-switch::part(control) { background: var(--nc-color-switch-off); }'],
    ['a checked wa-switch fill', 'wa-switch.on::part(control) { background: var(--wa-color-brand); }'],
    ['a wa-switch knob', 'wa-switch::part(thumb) { background: white; border: 1px solid; }'],
    ['a hand-drawn switch', "[role='switch'] { width: 38px; border-radius: 999px; }"],
    ['a native button on the brand ramp', 'button.go { background: var(--nc-grad-brand); }'],
    [
      'a native button on the brand ramp as an image',
      '.cta button { background-image: var(--nc-grad-brand-text); }',
    ],
    [
      'a brand button hidden in a list beside an exempt toggle',
      "button[aria-checked='true'], button.primary { background: var(--wa-color-brand); }",
    ],
    [
      'a brand button whose list mentions a selected option elsewhere',
      "[role='option'][aria-selected='true'], button.go { background: var(--wa-color-brand); }",
    ],
    [
      'a selected (not checked or pressed) native button',
      "button[aria-selected='true'] { background: var(--wa-color-brand); }",
    ],
    ['a current native button', "button[aria-current='page'] { background: var(--wa-color-brand); }"],
    ['a classed host reaching its base part', '.pill::part(base) { border-radius: 999px; }'],
    ['an unknown host reaching its button part', 'wc-thing::part(button) { color: red; }'],
    ['a host restyling a base edge', 'sl-button::part(base) { border-color: var(--wa-color-brand); }'],
    ['a base part with its own height', 'my-control::part(base) { height: 40px; }'],
  ])('flags %s', (_label, rules) => {
    expect(buttonStyling(sheet(rules))).not.toEqual([]);
  });

  it.each([
    ['a wa-button margin', 'wa-button { margin-left: auto; }'],
    ['a wa-button-group', 'wa-button-group { border-radius: 8px; }'],
    ['an icon button class on something else', '.icon-button { border-radius: 8px; }'],
    ['a native button with a neutral fill', 'button { background: var(--wa-color-surface); }'],
    [
      'the checked segment of a toggle',
      ".kinds button[aria-checked='true'] { background: var(--wa-color-brand); }",
    ],
    ['a brand-filled option', "[role='option'][aria-selected='true'] { background: var(--wa-color-brand); }"],
    ['a comment that names one', '/* wa-button { border-radius: 8px; } */ p { margin: 0; }'],
    ['a wa-switch placed in a row', 'wa-switch { margin-inline-start: auto; }'],
    ['the mode switcher', 'wc-mode-switcher { display: block; }'],
    [
      'a pressed toggle on the brand',
      "button[aria-pressed='true'] { background: var(--wa-color-brand); }",
    ],
    [
      'an input reaching its own base part',
      'wa-input::part(base) { border-color: var(--wa-color-brand); }',
    ],
    ['a dialog panel', 'wa-dialog::part(base) { background: var(--wa-color-surface); }'],
    ['a part that is not a button', 'wc-period-nav::part(pager) { height: 32px; }'],
    ['a list whose commas sit inside :is()', ':is(a, b) p { color: red; }'],
  ])('leaves %s alone', (_label, rules) => {
    expect(buttonStyling(sheet(rules))).toEqual([]);
  });

  it.each([
    ['<wa-button pill variant="brand">Go</wa-button>'],
    ['<wa-button variant="brand" solid>Go</wa-button>'],
    ['<wa-button\n  pill\n  appearance="outlined">Back</wa-button>'],
    ['<wa-button size="large" variant="brand">Go</wa-button>'],
    ['<wa-button size="small">Retry</wa-button>'],
  ])('flags the opt-in in %s', (template) => {
    expect(buttonStyling(template)).not.toEqual([]);
  });

  it('leaves a plain wa-button template alone', () => {
    expect(buttonStyling('<wa-button variant="brand" size="l">Go</wa-button>')).toEqual([]);
  });
});
