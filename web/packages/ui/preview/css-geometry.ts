/**
 * A tiny layout model for fixed-position overlays.
 *
 * jsdom has no layout engine: `getBoundingClientRect()` is all zeros and
 * `getComputedStyle()` resolves neither shorthands nor `calc()`, so a test
 * cannot ask the DOM where an overlay lands. This resolves the same question
 * from the component's own stylesheet — cascade order, shorthand expansion,
 * `var()` fallbacks, `calc()`/`min()`/`max()` — and returns a box in viewport
 * pixels, so an assertion can be "the toast sits inside the viewport" rather
 * than "the stylesheet contains this declaration".
 *
 * The model covers what a fixed overlay uses: physical and logical inset
 * properties in a horizontal-tb LTR writing mode, sizing, and translate
 * transforms. Anything outside that vocabulary resolves to `null`, which
 * `fixedBox` reports as an unanchored box rather than guessing.
 */

export interface Viewport {
  width: number;
  height: number;
}

export interface LengthContext {
  viewport: Viewport;
  /** What `%` resolves against — the containing block's size on that axis. */
  percentBasis?: number;
  rootFontSize?: number;
  /** Custom properties in scope, e.g. from `customProperties(css, ':host')`. */
  vars?: Record<string, string>;
}

const DEFAULT_ROOT_FONT_SIZE = 16;

/** Split on a separator that is not nested inside parentheses. */
function splitTopLevel(input: string, separator: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let current = '';
  for (const char of input) {
    if (char === '(') depth += 1;
    if (char === ')') depth -= 1;
    if (depth === 0 && char === separator) {
      parts.push(current);
      current = '';
      continue;
    }
    current += char;
  }
  parts.push(current);
  return parts.map((part) => part.trim()).filter((part) => part.length > 0);
}

function unitToPixels(value: number, unit: string, ctx: LengthContext): number | null {
  const root = ctx.rootFontSize ?? DEFAULT_ROOT_FONT_SIZE;
  switch (unit) {
    case '':
      // A dimensionless number: `0`, or a factor inside calc().
      return value;
    case 'px':
      return value;
    case 'rem':
    case 'em':
      return value * root;
    case 'vw':
    case 'dvw':
    case 'svw':
    case 'lvw':
      return (value / 100) * ctx.viewport.width;
    case 'vh':
    case 'dvh':
    case 'svh':
    case 'lvh':
      return (value / 100) * ctx.viewport.height;
    case '%':
      return ctx.percentBasis === undefined ? null : (value / 100) * ctx.percentBasis;
    default:
      return null;
  }
}

/**
 * Resolve a CSS length expression to pixels.
 *
 * Returns `null` for `auto`, for a keyword, and for anything the grammar below
 * does not cover — every caller treats `null` as "no usable length here".
 */
export function resolveLength(expression: string, ctx: LengthContext): number | null {
  const source = expression.trim();
  if (source.length === 0) return null;

  let index = 0;

  const skipSpace = (): void => {
    while (index < source.length && /\s/.test(source[index]!)) index += 1;
  };

  /** Consume `(...)` from the current position and return its arguments. */
  const takeArguments = (): string[] | null => {
    const open = source.indexOf('(', index);
    if (open === -1) return null;
    let depth = 0;
    for (let scan = open; scan < source.length; scan += 1) {
      if (source[scan] === '(') depth += 1;
      if (source[scan] === ')') {
        depth -= 1;
        if (depth === 0) {
          const inner = source.slice(open + 1, scan);
          index = scan + 1;
          return splitTopLevel(inner, ',');
        }
      }
    }
    return null;
  };

  const parseValue = (): number | null => {
    skipSpace();
    if (source[index] === '(') {
      index += 1;
      const inner = parseSum();
      skipSpace();
      if (source[index] === ')') index += 1;
      return inner;
    }

    const identifier = /^[a-zA-Z-]+(?=\()/.exec(source.slice(index))?.[0]?.toLowerCase();
    if (identifier === 'calc' || identifier === 'min' || identifier === 'max') {
      const args = takeArguments()?.map((argument) => resolveLength(argument, ctx));
      if (!args || args.some((argument) => argument === null)) return null;
      const numbers = args as number[];
      if (identifier === 'min') return Math.min(...numbers);
      if (identifier === 'max') return Math.max(...numbers);
      return numbers[0] ?? null;
    }
    if (identifier === 'var') {
      const args = takeArguments();
      if (!args || args.length === 0) return null;
      // A property the sheet declares itself resolves; one that comes from a
      // theme sheet this model does not load falls back, as it would in a
      // browser that has not loaded it either.
      const declared = ctx.vars?.[args[0]!];
      if (declared !== undefined) return resolveLength(declared, ctx);
      if (args.length < 2) return null;
      return resolveLength(args.slice(1).join(','), ctx);
    }

    const number = /^([+-]?(?:\d+\.?\d*|\.\d+))([a-zA-Z%]*)/.exec(source.slice(index));
    if (!number) return null;
    index += number[0].length;
    return unitToPixels(Number(number[1]), number[2]!.toLowerCase(), ctx);
  };

  const parseProduct = (): number | null => {
    let left = parseValue();
    for (;;) {
      skipSpace();
      const operator = source[index];
      if (operator !== '*' && operator !== '/') return left;
      index += 1;
      const right = parseValue();
      if (left === null || right === null) return null;
      left = operator === '*' ? left * right : left / right;
    }
  };

  const parseSum = (): number | null => {
    let left = parseProduct();
    for (;;) {
      skipSpace();
      const operator = source[index];
      if (operator !== '+' && operator !== '-') return left;
      // In calc(), an additive operator must be surrounded by whitespace, which
      // is also what keeps "-8px" from reading as a subtraction.
      if (!/\s/.test(source[index - 1] ?? '')) return left;
      index += 1;
      const right = parseProduct();
      if (left === null || right === null) return null;
      left = operator === '+' ? left + right : left - right;
    }
  };

  const result = parseSum();
  skipSpace();
  return index === source.length ? result : null;
}

/** Strip comments and every at-rule block, leaving only top-level rules. */
function topLevelRules(cssText: string): string {
  const withoutComments = cssText.replace(/\/\*[\s\S]*?\*\//g, '');
  let out = '';
  let index = 0;
  while (index < withoutComments.length) {
    const at = withoutComments.indexOf('@', index);
    if (at === -1) {
      out += withoutComments.slice(index);
      break;
    }
    out += withoutComments.slice(index, at);
    const open = withoutComments.indexOf('{', at);
    if (open === -1) break;
    let depth = 0;
    let scan = open;
    for (; scan < withoutComments.length; scan += 1) {
      if (withoutComments[scan] === '{') depth += 1;
      if (withoutComments[scan] === '}') {
        depth -= 1;
        if (depth === 0) break;
      }
    }
    index = scan + 1;
  }
  return out;
}

const LOGICAL_TO_PHYSICAL: Record<string, string> = {
  'inset-block-start': 'top',
  'inset-block-end': 'bottom',
  'inset-inline-start': 'left',
  'inset-inline-end': 'right',
  'inline-size': 'width',
  'block-size': 'height',
  'max-inline-size': 'max-width',
  'max-block-size': 'max-height',
};

/** Expand one declaration into physical longhands, in cascade order. */
function expand(property: string, value: string): [string, string][] {
  const physical = LOGICAL_TO_PHYSICAL[property];
  if (physical) return [[physical, value]];

  const parts = splitTopLevel(value, ' ');
  if (property === 'inset') {
    const [top, right = top, bottom = top, left = right] = parts as [string, ...string[]];
    return [
      ['top', top],
      ['right', right],
      ['bottom', bottom],
      ['left', left],
    ];
  }
  if (property === 'inset-block') {
    const [start, end = start] = parts as [string, ...string[]];
    return [
      ['top', start],
      ['bottom', end],
    ];
  }
  if (property === 'inset-inline') {
    const [start, end = start] = parts as [string, ...string[]];
    return [
      ['left', start],
      ['right', end],
    ];
  }
  return [[property, value]];
}

/** Every declaration a selector's top-level rules carry, in source order. */
function* declarationsFor(
  cssText: string,
  selector: string,
): Generator<[property: string, value: string]> {
  for (const match of topLevelRules(cssText).matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    if (!match[1]!.split(',').some((one) => one.trim() === selector)) continue;
    for (const declaration of splitTopLevel(match[2]!, ';')) {
      const colon = declaration.indexOf(':');
      if (colon === -1) continue;
      yield [declaration.slice(0, colon).trim(), declaration.slice(colon + 1).trim()];
    }
  }
}

/**
 * The declarations a selector ends up with, shorthands expanded and later
 * declarations winning — the same last-one-wins pass a browser makes within a
 * single rule.
 */
export function resolvedDeclarations(
  cssText: string,
  selector: string,
): Map<string, string> {
  const resolved = new Map<string, string>();
  for (const [property, value] of declarationsFor(cssText, selector)) {
    if (property.startsWith('--')) continue;
    for (const [expanded, expandedValue] of expand(property.toLowerCase(), value)) {
      resolved.set(expanded, expandedValue);
    }
  }
  return resolved;
}

/** The custom properties a selector declares. */
export function customProperties(
  cssText: string,
  selector: string,
): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [property, value] of declarationsFor(cssText, selector)) {
    if (property.startsWith('--')) out[property] = value;
  }
  return out;
}

export interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
  right: number;
  bottom: number;
}

export interface FixedBoxOptions {
  viewport: Viewport;
  /** Size the element would take if nothing constrained it. */
  content: { width: number; height: number };
  /** Containing block for the element; defaults to the viewport. */
  containingBlock?: Box;
  rootFontSize?: number;
  vars?: Record<string, string>;
}

function translateOffsets(
  transform: string | undefined,
  size: { width: number; height: number },
  ctx: LengthContext,
): { x: number; y: number } {
  if (!transform || transform === 'none') return { x: 0, y: 0 };
  let x = 0;
  let y = 0;
  for (const match of transform.matchAll(/translate([XY]?)\(([^)]*)\)/g)) {
    const axis = match[1];
    const args = splitTopLevel(match[2]!, ',');
    const onX = resolveLength(args[0] ?? '0', { ...ctx, percentBasis: size.width }) ?? 0;
    const onY = resolveLength(args[1] ?? '0', { ...ctx, percentBasis: size.height }) ?? 0;
    if (axis === 'X') x += onX;
    else if (axis === 'Y') y += onY;
    else {
      x += onX;
      y += onY;
    }
  }
  return { x, y };
}

/**
 * Where a fixed-position element lands, or `null` when the stylesheet leaves
 * it unanchored on an axis — both insets `auto`, so the browser falls back to
 * the element's static position, which is wherever the box tree happens to put
 * it and is not a placement the stylesheet can promise.
 */
export function fixedBox(
  declarations: Map<string, string>,
  options: FixedBoxOptions,
): Box | null {
  const block = options.containingBlock ?? {
    left: 0,
    top: 0,
    width: options.viewport.width,
    height: options.viewport.height,
    right: options.viewport.width,
    bottom: options.viewport.height,
  };
  const base = {
    viewport: options.viewport,
    rootFontSize: options.rootFontSize,
    vars: options.vars,
  };
  const horizontal: LengthContext = { ...base, percentBasis: block.width };
  const vertical: LengthContext = { ...base, percentBasis: block.height };

  const axis = (
    startProperty: 'left' | 'top',
    endProperty: 'right' | 'bottom',
    sizeProperty: 'width' | 'height',
    maxProperty: 'max-width' | 'max-height',
    ctx: LengthContext,
    blockStart: number,
    blockSize: number,
    contentSize: number,
  ): { start: number; size: number } | null => {
    const start = resolveLength(declarations.get(startProperty) ?? 'auto', ctx);
    const end = resolveLength(declarations.get(endProperty) ?? 'auto', ctx);
    const declaredSize = resolveLength(declarations.get(sizeProperty) ?? 'auto', ctx);
    const maximum = resolveLength(declarations.get(maxProperty) ?? 'none', ctx);

    if (start !== null && end !== null && declaredSize === null) {
      // Both insets pin the element: the size falls out of the containing
      // block, and a max that cuts it over-constrains the axis — which a
      // browser settles by dropping the end inset, not the size.
      const pinned = blockSize - start - end;
      const size = maximum !== null ? Math.min(pinned, maximum) : pinned;
      return { start: blockStart + start, size };
    }
    let size = declaredSize ?? contentSize;
    if (maximum !== null) size = Math.min(size, maximum);
    if (start !== null) return { start: blockStart + start, size };
    if (end !== null) return { start: blockStart + blockSize - end - size, size };
    return null;
  };

  const across = axis(
    'left',
    'right',
    'width',
    'max-width',
    horizontal,
    block.left,
    block.width,
    options.content.width,
  );
  const down = axis(
    'top',
    'bottom',
    'height',
    'max-height',
    vertical,
    block.top,
    block.height,
    options.content.height,
  );
  if (!across || !down) return null;

  const shift = translateOffsets(
    declarations.get('transform'),
    { width: across.size, height: down.size },
    base,
  );

  const left = across.start + shift.x;
  const top = down.start + shift.y;
  return {
    left,
    top,
    width: across.size,
    height: down.size,
    right: left + across.size,
    bottom: top + down.size,
  };
}

/** True when the box lies wholly within the viewport. */
export function isInsideViewport(box: Box, viewport: Viewport): boolean {
  return (
    box.left >= 0 &&
    box.top >= 0 &&
    box.right <= viewport.width &&
    box.bottom <= viewport.height
  );
}
