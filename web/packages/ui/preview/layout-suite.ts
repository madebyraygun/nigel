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

/**
 * Drop `@media` and `@supports` blocks.
 *
 * A print rule is a different box for a different medium; reading it as part
 * of the screen box would have `wc-app-shell`'s content area resolve to the
 * `display: block` it takes on paper.
 */
function unconditionalRules(text: string): string {
  let out = '';
  let index = 0;

  while (index < text.length) {
    const at = text.indexOf('@', index);
    const open = at === -1 ? -1 : text.indexOf('{', at);
    if (open === -1) return out + text.slice(index);

    out += text.slice(index, at);
    let depth = 1;
    index = open + 1;
    while (index < text.length && depth > 0) {
      if (text[index] === '{') depth += 1;
      else if (text[index] === '}') depth -= 1;
      index += 1;
    }
  }

  return out;
}

/** Comments sit in front of selectors, so they are part of reading one. */
function withoutComments(text: string): string {
  return text.replace(/\/\*[\s\S]*?\*\//g, '');
}

/** Every declaration block a selector carries, in source order. */
function blocksFor(text: string, selector: string): string[] {
  const blocks: string[] = [];
  const pattern = /([^{}]+)\{([^{}]*)\}/g;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(text)) !== null) {
    const found = (match[1] ?? '').trim().replace(/\s+/g, ' ');
    if (found === selector) blocks.push(match[2] ?? '');
  }
  return blocks;
}

/**
 * What a component's own stylesheet resolves one of its boxes to.
 *
 * jsdom performs no layout, so a unit test cannot measure a centred panel.
 * What it can do is put the declarations through the CSSOM and read back the
 * values a browser would hand to layout — which survives a shorthand, a
 * reordering, or a second rule for the same selector in a way that matching
 * the stylesheet's text does not.
 */
export function resolvedBox(ctor: StyledCtor, selector = ':host'): ResolvedBox {
  const blocks = blocksFor(unconditionalRules(withoutComments(styleText(ctor))), selector);
  if (blocks.length === 0) {
    throw new Error(`${ctor.name} has no rule for \`${selector}\``);
  }

  const probe = document.createElement('div');
  probe.style.cssText = blocks.join(';');
  document.body.appendChild(probe);
  const style = getComputedStyle(probe);
  const box: ResolvedBox = {
    display: style.display,
    flexDirection: style.flexDirection,
    flexGrow: style.flexGrow || '0',
    blockAlignment:
      style.alignContent || style.placeContent.split(' ')[0] || 'normal',
  };
  probe.remove();
  return box;
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
      const box = resolvedBox(ctor, selector);
      expect(box.display).toBe('flex');
      expect(box.flexDirection).toBe('column');
    });
  });
}
