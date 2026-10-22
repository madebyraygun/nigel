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
 * matching the New invoice button that opened it. This scan fails the build
 * on the shapes that drift:
 *
 * - a rule whose selector names `wa-button` and sets a radius, a height or a
 *   fill (`background*`, or the `--wa-color-*` and `--wa-form-control-*`
 *   properties the button reads them from);
 * - a rule whose selector names a native `button` and fills it with the brand
 *   colour, unless it is the checked or pressed state of a toggle — a segment
 *   or an option showing it is selected, not an action;
 * - a `pill` or `solid` attribute on a `wa-button` in a template: the theme
 *   has one button, not opt-ins to a better one;
 * - a rule whose selector names `wa-switch`, or an element with
 *   `role="switch"`, and sets its size, shape, edge or fill — the standard
 *   switch is drawn in the same sheet for the same reason.
 */

const WA_BUTTON = /(^|[^a-z-])wa-button(?![a-z-])/;
const SWITCH = /(^|[^a-z-])wa-switch(?![a-z-])|\[role=['"]?switch/;
const SWITCH_FORBIDDEN =
  /(?:^|;)\s*(--(?:width|height|thumb-size)|(?:min-|max-)?(?:width|height)|border(?:-[a-z]+)*|background(?:-color|-image)?)\s*:/g;
const NATIVE_BUTTON = /(^|[\s,>+~(])button(?![a-z-])/;
const SELECTED_STATE = /\[aria-(checked|pressed|selected|current)|:checked/;

const WA_BUTTON_FORBIDDEN =
  /(?:^|;)\s*(border(?:-[a-z]+)*-radius|(?:min-|max-)?height|(?:min-|max-)?block-size|background(?:-color|-image)?|--wa-form-control-(?:height|border-radius|padding-inline)|--wa-border-radius-[a-z]+|--wa-color-(?:fill|on|border)-[a-z]+|--wa-color-brand[a-z-]*)\s*:/g;

const BRAND_FILL = /(?:^|;)\s*background(?:-color)?\s*:[^;]*var\(--wa-color-brand\b/;

const OPT_IN = /<wa-button\b[^>]*?\s(pill|solid)(?=[\s=>/])/g;

/** The text inside every `css\`…\`` template in a source file, comments stripped. */
export function cssBlocks(source: string): string[] {
  return [...source.matchAll(/\bcss`([\s\S]*?)`/g)].map((m) =>
    m[1].replace(/\/\*[\s\S]*?\*\//g, ''),
  );
}

/** Every way `source` styles a button itself, one line per offence. */
export function buttonStyling(source: string): string[] {
  const out: string[] = [];

  for (const block of cssBlocks(source)) {
    for (const [, rawSelector, declarations] of block.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      const selector = rawSelector.replace(/\s+/g, ' ').trim();

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
        !SELECTED_STATE.test(selector) &&
        BRAND_FILL.test(declarations)
      ) {
        out.push(`${selector} { brand fill }`);
      }
    }
  }

  for (const [, attribute] of source.matchAll(OPT_IN)) {
    out.push(`<wa-button ${attribute}>`);
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
trigger, size="large" for 38px, a labelled icon alone for an icon button) or
<wa-switch>, and leave their shape and fill to controlsCss in @nigel/theme:
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
