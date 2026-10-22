import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { join, relative } from 'node:path';

/**
 * The standard button and switch are drawn in one place: `controlsCss` in
 * `@nigel/theme`.
 *
 * Every screen and component that drew its own — a native `<button>` with a
 * brand fill, a `wa-button` given its own corners or height — drifted from it
 * the moment the theme moved, which is how one editor's Create draft stopped
 * matching the New invoice button that opened it. This scan reads every
 * `css\`…\`` template, splits each selector list on its top-level commas, and
 * fails the build on any complex selector that:
 *
 * - names `wa-button` and sets a radius, a height or a fill (`background*`,
 *   or the `--wa-color-*` and `--wa-form-control-*` properties the button
 *   reads them from);
 * - names a native `button` and fills it with the brand (`--wa-color-brand*`
 *   or `--nc-grad-brand*` in a `background*` declaration), unless that same
 *   selector is a toggle's checked or pressed state (`[aria-checked`,
 *   `[aria-pressed`, `:checked`);
 * - reaches a `base` or `button` part on any host that is not a known
 *   non-button primitive, and sets its radius, height, fill, border colour or
 *   text colour;
 * - names `wa-switch`, or an element with `role="switch"`, and sets its size,
 *   edge or fill.
 *
 * In templates it fails on a `pill` or `solid` attribute on a `wa-button` —
 * the theme has one button, not opt-ins to a better one — and on the
 * deprecated `size="small"` or `size="large"`, which the theme does not size.
 */

const WA_BUTTON = /(^|[^a-z-])wa-button(?![a-z-])/;
const SWITCH = /(^|[^a-z-])wa-switch(?![a-z-])|\[role=['"]?switch/;
const SWITCH_FORBIDDEN =
  /(?:^|;)\s*(--(?:width|height|thumb-size)|(?:min-|max-)?(?:width|height)|border(?:-[a-z]+)*|background(?:-color|-image)?)\s*:/g;
const NATIVE_BUTTON = /(^|[\s>+~(])button(?![a-z-])/;
const TOGGLE_STATE = /\[aria-(checked|pressed)|:checked/;

const WA_BUTTON_FORBIDDEN =
  /(?:^|;)\s*(border(?:-[a-z]+)*-radius|(?:min-|max-)?height|(?:min-|max-)?block-size|background(?:-color|-image)?|--wa-form-control-(?:height|border-radius|padding-inline)|--wa-border-radius-[a-z]+|--wa-color-(?:fill|on|border)-[a-z]+|--wa-color-brand[a-z-]*)\s*:/g;

const BRAND_FILL =
  /(?:^|;)\s*background(?:-color|-image)?\s*:[^;]*var\(--(?:wa-color-brand|nc-grad-brand)/;

/** A `::part(base)` or `::part(button)`, and the compound it hangs off. */
const BUTTON_PART = /([^\s>+~]*)::part\((?:base|button)\)/;

/** Primitives whose `base` part is not a button. */
const NON_BUTTON_HOSTS = new Set([
  'wa-input',
  'wa-select',
  'wa-textarea',
  'wa-dialog',
  'wa-drawer',
  'wa-switch',
  'wa-checkbox',
  'wa-radio',
  'wa-radio-group',
  'wa-details',
  'wa-tooltip',
  'wa-popup',
  'wa-callout',
  'wa-card',
  'wa-tag',
  'wa-badge',
  'wa-spinner',
  'wa-progress-bar',
  'wa-format-bytes',
]);

const PART_FORBIDDEN =
  /(?:^|;)\s*(border(?:-[a-z]+)*-radius|(?:min-|max-)?height|(?:min-|max-)?block-size|background(?:-color|-image)?|border(?:-[a-z]+)*-color|color)\s*:/g;

const OPT_IN = /<wa-button\b[^>]*?\s(pill|solid)(?=[\s=>/])/g;
const DEPRECATED_SIZE = /<wa-button\b[^>]*?\ssize=["'](small|large)["']/g;

/** The text inside every `css\`…\`` template in a source file, comments stripped. */
export function cssBlocks(source: string): string[] {
  return [...source.matchAll(/\bcss`([\s\S]*?)`/g)].map((m) =>
    m[1].replace(/\/\*[\s\S]*?\*\//g, ''),
  );
}

/** A selector list split on its top-level commas, so `:is(a, b)` stays whole. */
export function splitSelectorList(list: string): string[] {
  const out: string[] = [];
  let depth = 0;
  let current = '';
  for (const char of list) {
    if (char === '(' || char === '[') depth += 1;
    else if (char === ')' || char === ']') depth -= 1;
    if (char === ',' && depth === 0) {
      out.push(current.trim());
      current = '';
    } else {
      current += char;
    }
  }
  if (current.trim() !== '') out.push(current.trim());
  return out;
}

/** The tag a `::part(base|button)` hangs off, '' for none, null for no such part. */
function partHost(selector: string): string | null {
  const match = selector.match(BUTTON_PART);
  if (!match) return null;
  return match[1].match(/^[a-z][a-z0-9-]*/)?.[0] ?? '';
}

/** Every way `source` styles a button or switch itself, one line per offence. */
export function buttonStyling(source: string): string[] {
  const out: string[] = [];

  for (const block of cssBlocks(source)) {
    for (const [, rawList, declarations] of block.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      for (const selector of splitSelectorList(rawList.replace(/\s+/g, ' '))) {
        if (WA_BUTTON.test(selector)) {
          for (const [, property] of declarations.matchAll(WA_BUTTON_FORBIDDEN)) {
            out.push(`${selector} { ${property} }`);
          }
        }

        if (SWITCH.test(selector)) {
          for (const [, property] of declarations.matchAll(SWITCH_FORBIDDEN)) {
            out.push(`${selector} { ${property} }`);
          }
        }

        if (
          NATIVE_BUTTON.test(selector) &&
          !TOGGLE_STATE.test(selector) &&
          BRAND_FILL.test(declarations)
        ) {
          out.push(`${selector} { brand fill }`);
        }

        const host = partHost(selector);
        if (host !== null && !WA_BUTTON.test(selector) && !NON_BUTTON_HOSTS.has(host)) {
          for (const [, property] of declarations.matchAll(PART_FORBIDDEN)) {
            out.push(`${selector} { ${property} }`);
          }
        }
      }
    }
  }

  for (const [, attribute] of source.matchAll(OPT_IN)) {
    out.push(`<wa-button ${attribute}>`);
  }
  for (const [, size] of source.matchAll(DEPRECATED_SIZE)) {
    out.push(`<wa-button size="${size}">`);
  }

  return out;
}

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === 'node_modules' || entry.name === 'dist') continue;
      out.push(...walk(full));
    } else if (entry.name.endsWith('.ts') && !entry.name.endsWith('.test.ts')) {
      out.push(full);
    }
  }
  return out;
}

/** Fail the build when any source file under `srcDir` styles a button itself. */
export function describeButtonOwnership(srcDir: string): void {
  describe('the standard button and switch belong to the theme', () => {
    const sources = walk(srcDir);

    it('is styled by no screen or component', () => {
      const offenders = sources.flatMap((file) =>
        buttonStyling(readFileSync(file, 'utf8')).map(
          (offence) => `${relative(srcDir, file)}: ${offence}`,
        ),
      );

      expect(
        offenders,
        `these style a button or switch themselves. Use <wa-button> (variant="brand"
for the primary, appearance="outlined" variant="danger" for a destructive
trigger, size="l" for 38px, size="s" for 28px, a labelled icon alone for an
icon button) or <wa-switch>, and leave their shape and fill to controlsCss in
@nigel/theme:
${offenders.join('\n')}`,
      ).toEqual([]);
    });

    it('actually scans the sources', () => {
      // Guards the guard: a walk that silently returned nothing would pass.
      expect(sources.length).toBeGreaterThan(10);
      expect(sources.some((file) => readFileSync(file, 'utf8').includes('<wa-button'))).toBe(
        true,
      );
    });
  });
}
