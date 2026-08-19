/**
 * @vitest-environment jsdom
 */
import { describe, it, expect } from 'vitest';
import { LitElement, unsafeCSS } from 'lit';
import {
  isColumn,
  resolvedBox,
} from '../../../../packages/ui/preview/layout-suite.js';

/**
 * Screens are columns filling the content area.
 *
 * `wc-app-shell` stretches whatever is in its default slot to the whole
 * content area, and `wc-empty-state` asks a column for the height nothing else
 * claimed. Both halves are in `@nigel/ui`; the part that lives here is the
 * screen being a column, which is what carries the height between them. A
 * screen that stacks its children some other way is not wrong so much as
 * silently different: an empty state on it sits at the top of the page while
 * the same element on every other screen is centred.
 *
 * The verdict is the resolved box rather than the text of a rule, so a
 * shorthand, a second `:host` rule, or a declaration written without its
 * trailing semicolon all answer the way the browser would.
 *
 * The gates are exempt. Each replaces the shell rather than filling it — there
 * is no sidebar and no header behind the password prompt or the first-run
 * questions — and each centres itself.
 */
const EXEMPT = ['setup.ts', 'unlock.ts'];

const modules = import.meta.glob<Record<string, unknown>>(
  ['../screens/*.ts', '!../screens/*.test.ts'],
  { eager: true },
);

type ScreenCtor = typeof LitElement;

function isElementCtor(value: unknown): value is ScreenCtor {
  return (
    typeof value === 'function' &&
    (value as ScreenCtor).prototype instanceof HTMLElement &&
    (value as ScreenCtor).styles !== undefined
  );
}

function screens(): { file: string; ctor: ScreenCtor }[] {
  const found: { file: string; ctor: ScreenCtor }[] = [];
  for (const [path, module] of Object.entries(modules)) {
    const file = path.slice(path.lastIndexOf('/') + 1);
    if (EXEMPT.includes(file)) continue;
    for (const exported of Object.values(module)) {
      if (isElementCtor(exported)) found.push({ file, ctor: exported });
    }
  }
  return found;
}

/** A stand-in screen, to drive the verdict from a rule rather than a file. */
function styledAs(rules: string): ScreenCtor {
  return class extends LitElement {
    static styles = unsafeCSS(rules);
  };
}

describe('screen layout', () => {
  const found = screens();

  it('finds the screens', () => {
    // A rename that emptied the list would otherwise pass every check below.
    expect(found.length).toBeGreaterThan(10);
  });

  it('lays every screen out as a column that fills the content area', () => {
    const offenders = found
      .filter(({ ctor }) => {
        try {
          return !isColumn(resolvedBox(ctor));
        } catch {
          return true;
        }
      })
      .map(({ file }) => file);
    expect(offenders).toEqual([]);
  });

  it('accepts a screen that says the direction through flex-flow', () => {
    expect(isColumn(resolvedBox(styledAs(':host { display: flex; flex-flow: column }')))).toBe(
      true,
    );
  });

  it('rejects a screen whose later rule puts the host back to block', () => {
    const ctor = styledAs(
      ':host { display: flex; flex-direction: column } :host { display: block }',
    );
    expect(isColumn(resolvedBox(ctor))).toBe(false);
  });

  it('rejects a screen with no host rule at all', () => {
    expect(() => resolvedBox(styledAs('.thing { display: flex }'))).toThrow(':host');
  });
});
