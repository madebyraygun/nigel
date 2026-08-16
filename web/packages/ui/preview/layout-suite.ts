import { describe, it, expect } from 'vitest';
import type { LitElement } from 'lit';
import { styleText } from './controls-suite.js';

type StyledCtor = typeof LitElement;

/** The part of a box that decides where a screen's empty state ends up. */
export interface ResolvedBox {
  display: string;
  flexDirection: string;
  /** Whether the box asks its parent for the space left over in it. */
  flexGrow: string;
  /** Block-axis alignment of the box's own content, however it was spelled. */
  blockAlignment: string;
}

interface AtRule {
  prelude: string;
  body: string;
}

interface Scanned {
  /** Everything outside an at-rule. */
  plain: string;
  atRules: AtRule[];
}

/** Comments sit in front of selectors, so they are part of reading one. */
export function withoutComments(text: string): string {
  return text.replace(/\/\*[\s\S]*?\*\//g, '');
}

/**
 * Split a stylesheet into its top-level rules and its at-rules.
 *
 * An `@` only starts an at-rule between rules: inside a declaration block it
 * is an ordinary character, in `content: '@'` or a `url()`. A blockless
 * statement (`@import 'x.css';`) ends at its semicolon and has no body — if it
 * were assumed to open a block, the scanner would swallow the rule after it.
 */
function scan(css: string): Scanned {
  let plain = '';
  const atRules: AtRule[] = [];
  let index = 0;
  let depth = 0;
  let quote: string | null = null;

  while (index < css.length) {
    const char = css[index] as string;

    if (quote !== null) {
      plain += char;
      if (char === '\\') {
        plain += css[index + 1] ?? '';
        index += 2;
        continue;
      }
      if (char === quote) quote = null;
      index += 1;
      continue;
    }

    if (char === '"' || char === "'") {
      quote = char;
      plain += char;
      index += 1;
      continue;
    }

    if (char === '@' && depth === 0) {
      let cursor = index;
      while (cursor < css.length && css[cursor] !== '{' && css[cursor] !== ';') cursor += 1;
      const prelude = css.slice(index, cursor).trim();

      if (cursor >= css.length || css[cursor] === ';') {
        index = cursor + 1;
        continue;
      }

      let open = 1;
      let end = cursor + 1;
      while (end < css.length && open > 0) {
        if (css[end] === '{') open += 1;
        else if (css[end] === '}') open -= 1;
        end += 1;
      }
      atRules.push({ prelude, body: css.slice(cursor + 1, end - 1) });
      index = end;
      continue;
    }

    if (char === '{') depth += 1;
    else if (char === '}') depth -= 1;
    plain += char;
    index += 1;
  }

  return { plain, atRules };
}

/** Every declaration block a selector carries, in source order. */
function blocksFor(css: string, selector: string): string[] {
  const blocks: string[] = [];
  const pattern = /([^{}]+)\{([^{}]*)\}/g;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(css)) !== null) {
    const found = (match[1] ?? '').trim().replace(/\s+/g, ' ');
    if (found === selector) blocks.push(match[2] ?? '');
  }
  return blocks;
}

/** The direction half of `flex-flow`, which cssstyle does not expand. */
function directionFromFlow(flow: string): string {
  return (
    flow.split(/\s+/).find((part) => /^(row|column)(-reverse)?$/.test(part)) ?? ''
  );
}

function resolve(declarations: string[]): ResolvedBox {
  const probe = document.createElement('div');
  probe.style.cssText = declarations.join(';');
  document.body.appendChild(probe);
  const style = getComputedStyle(probe);
  const box: ResolvedBox = {
    display: style.display,
    flexDirection: style.flexDirection || directionFromFlow(style.flexFlow),
    flexGrow: style.flexGrow || '0',
    blockAlignment:
      style.alignContent || style.placeContent.split(' ')[0] || 'normal',
  };
  probe.remove();
  return box;
}

/**
 * What a stylesheet resolves one of its boxes to.
 *
 * jsdom performs no layout, so a unit test cannot measure a centred panel.
 * What it can do is put the declarations through the CSSOM and read back the
 * values a browser would hand to layout. Every rule for the selector is
 * applied in source order, so a second rule overriding the first is resolved
 * rather than missed, and a shorthand answers for the longhand it sets.
 */
export function resolvedBoxFromCss(css: string, selector = ':host'): ResolvedBox {
  const blocks = blocksFor(scan(withoutComments(css)).plain, selector);
  if (blocks.length === 0) throw new Error(`no rule for \`${selector}\``);
  return resolve(blocks);
}

/** `resolvedBoxFromCss` over everything a component adopts. */
export function resolvedBox(ctor: StyledCtor, selector = ':host'): ResolvedBox {
  try {
    return resolvedBoxFromCss(styleText(ctor), selector);
  } catch {
    throw new Error(`${ctor.name} has no rule for \`${selector}\``);
  }
}

/**
 * The same box as the printer gets: the screen rules with the `@media print`
 * block applied over them, which is the order the cascade reads them in.
 */
export function printedBox(ctor: StyledCtor, selector = ':host'): ResolvedBox {
  const { plain, atRules } = scan(withoutComments(styleText(ctor)));
  const printed = atRules
    .filter((rule) => /^@media\b/.test(rule.prelude) && /\bprint\b/.test(rule.prelude))
    .map((rule) => rule.body)
    .join('\n');
  const blocks = [...blocksFor(plain, selector), ...blocksFor(printed, selector)];
  if (blocks.length === 0) {
    throw new Error(`${ctor.name} has no rule for \`${selector}\``);
  }
  return resolve(blocks);
}

/** Whether a box stacks its children and can hand over its spare height. */
export function isColumn(box: ResolvedBox): boolean {
  return box.display === 'flex' && box.flexDirection === 'column';
}

/**
 * Assert a component takes the space its parent column has left and centres
 * its content in it.
 *
 * The two halves are one behaviour: growing without centring puts the content
 * back at the top of a taller box, and centring without growing centres it in
 * a box the size of the content, which is no movement at all.
 */
export function describeFillsItsBox(ctor: StyledCtor, selector = ':host'): void {
  describe(`${selector} fills its box`, () => {
    it('asks its parent column for the height left over', () => {
      expect(resolvedBox(ctor, selector).flexGrow).toBe('1');
    });

    it('centres its content in the box it is given', () => {
      expect(resolvedBox(ctor, selector).blockAlignment).toBe('center');
    });
  });
}

/**
 * Assert a box is a column, which is what makes the height it was given
 * available to a child that asks for it.
 */
export function describeColumnLayout(ctor: StyledCtor, selector = ':host'): void {
  describe(`${selector} is a column`, () => {
    it('stacks its children and can hand over its spare height', () => {
      expect(isColumn(resolvedBox(ctor, selector))).toBe(true);
    });
  });
}

/**
 * Assert a box that is a flex column on screen is a block on paper.
 *
 * A printed page is fragmented into sheets and a flex container is not
 * required to fragment: Safari and older Chromium slice through a row rather
 * than break between two. Nothing on paper needs the column — the leftover
 * height it hands out is a property of a viewport — so the print sheet puts
 * the box back to the block flow that breaks cleanly.
 */
export function describePrintsAsBlock(ctor: StyledCtor, selector = ':host'): void {
  describe(`${selector} on paper`, () => {
    it('is a column on screen', () => {
      expect(isColumn(resolvedBox(ctor, selector))).toBe(true);
    });

    it('goes back to block flow, which fragments across sheets', () => {
      expect(printedBox(ctor, selector).display).toBe('block');
    });
  });
}
