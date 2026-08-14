import { describe, it, expect } from 'vitest';
import {
  customProperties,
  fixedBox,
  isInsideViewport,
  resolveLength,
  resolvedDeclarations,
} from './css-geometry.js';

const viewport = { width: 1000, height: 800 };

describe('resolveLength', () => {
  it.each([
    ['24px', 24],
    ['1.5rem', 24],
    ['50%', 200],
    ['10vw', 100],
    ['25vh', 200],
    ['0', 0],
    ['calc(100vw - 32px)', 968],
    ['calc(100vw - 2 * 16px)', 968],
    ['min(360px, 100%)', 360],
    ['max(360px, 100%)', 400],
    ['var(--gap, 12px)', 12],
    ['calc(100% - var(--gap, 12px))', 388],
  ])('resolves %s', (expression, expected) => {
    expect(resolveLength(expression, { viewport, percentBasis: 400 })).toBe(expected);
  });

  it('reads a custom property that is in scope', () => {
    expect(resolveLength('var(--gutter)', { viewport, vars: { '--gutter': '20px' } })).toBe(
      20,
    );
  });

  it.each(['auto', 'none', 'fit-content', 'var(--absent)', '%'])(
    'has no pixel value for %s',
    (expression) => {
      expect(resolveLength(expression, { viewport })).toBeNull();
    },
  );
});

describe('resolvedDeclarations', () => {
  const css = `
    /* a comment { with braces } */
    .box {
      position: fixed;
      left: 50%;
      inset-block-start: 24px;
      inset-inline: auto;
    }
    @media print {
      .box { left: 0; }
    }
  `;

  it('expands logical properties onto physical ones', () => {
    expect(resolvedDeclarations(css, '.box').get('top')).toBe('24px');
  });

  it('lets a later shorthand override an earlier longhand', () => {
    // `inset-inline: auto` comes after `left: 50%`, so `left` ends up auto —
    // the cascade order a browser applies within one rule.
    expect(resolvedDeclarations(css, '.box').get('left')).toBe('auto');
  });

  it('ignores rules nested in at-rules', () => {
    expect(resolvedDeclarations('@media print { .box { top: 9px } }', '.box').size).toBe(0);
  });
});

describe('customProperties', () => {
  it('collects the properties a selector declares', () => {
    const vars = customProperties(':host { --gutter: 16px; display: contents; }', ':host');
    expect(vars).toEqual({ '--gutter': '16px' });
  });
});

describe('fixedBox', () => {
  const content = { width: 300, height: 40 };

  it('sizes an element pinned on both insets from its containing block', () => {
    const box = fixedBox(resolvedDeclarations('.r { inset: 16px; }', '.r'), {
      viewport,
      content,
    });
    expect(box).toEqual({
      left: 16,
      top: 16,
      width: 968,
      height: 768,
      right: 984,
      bottom: 784,
    });
  });

  it('places a corner-anchored element from the far edges', () => {
    const box = fixedBox(
      resolvedDeclarations('.r { inset-block: auto 16px; inset-inline: auto 16px; }', '.r'),
      { viewport, content },
    );
    expect(box).toMatchObject({ left: 684, top: 744, right: 984, bottom: 784 });
  });

  it('clamps content to max-width instead of overflowing', () => {
    const box = fixedBox(
      resolvedDeclarations('.r { inset-inline: auto 16px; inset-block: auto 16px; max-inline-size: 360px; }', '.r'),
      { viewport, content: { width: 5000, height: 40 } },
    );
    expect(box?.width).toBe(360);
  });

  it('reports no box when both insets on an axis are auto', () => {
    // Nothing anchors it: the browser would fall back to the static position,
    // which the stylesheet does not determine.
    const box = fixedBox(
      resolvedDeclarations('.r { position: fixed; top: 24px; }', '.r'),
      { viewport, content },
    );
    expect(box).toBeNull();
  });

  it('applies a translate transform to the placement', () => {
    const box = fixedBox(
      resolvedDeclarations('.r { left: 0; top: 0; transform: translateX(-50%); }', '.r'),
      { viewport, content },
    );
    expect(box?.left).toBe(-150);
    expect(isInsideViewport(box!, viewport)).toBe(false);
  });
});
