import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import './wc-toast.js';
import { dispatchNcToast, MAX_VISIBLE_TOASTS, WcToast } from './wc-toast.js';
import { describePrintHiding } from '../../preview/print-suite.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { styleText } from '../../preview/controls-suite.js';
import {
  customProperties,
  fixedBox,
  isInsideViewport,
  resolveLength,
  resolvedDeclarations,
  type Viewport,
} from '../../preview/css-geometry.js';
import preview from './wc-toast.preview.js';

async function mount(): Promise<WcToast> {
  const el = document.createElement('wc-toast');
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function toastEl(el: WcToast): HTMLElement | null {
  return el.shadowRoot?.querySelector('.toast') ?? null;
}

function messages(el: WcToast): string[] {
  return [...(el.shadowRoot?.querySelectorAll('.toast .message') ?? [])].map((node) =>
    (node.textContent ?? '').trim(),
  );
}

function region(el: WcToast): HTMLElement | null {
  return el.shadowRoot?.querySelector('[data-toast-region]') ?? null;
}

describe('wc-toast', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('starts empty but keeps the live region mounted', async () => {
    const el = await mount();
    expect(toastEl(el)).toBeNull();
    // The region has to persist or assistive tech never subscribes to it.
    expect(region(el)).toBeTruthy();
  });

  it('shows a toast dispatched from an unrelated element', async () => {
    const el = await mount();
    // Deliberately not a descendant: the bus listens on window precisely so
    // detached and top-layer subtrees still reach the region.
    const stranger = document.createElement('div');
    document.body.appendChild(stranger);

    dispatchNcToast(stranger, { message: 'Saved.' });
    await el.updateComplete;

    expect(toastEl(el)?.textContent).toContain('Saved.');
  });

  it('announces politely for info and success', async () => {
    const el = await mount();
    dispatchNcToast(el, { message: 'Done.', variant: 'success' });
    await el.updateComplete;
    expect(region(el)?.getAttribute('role')).toBe('status');
    expect(region(el)?.getAttribute('aria-live')).toBe('polite');
    expect(toastEl(el)?.hasAttribute('role')).toBe(false);
  });

  it('announces assertively for danger without escalating the region', async () => {
    const el = await mount();
    dispatchNcToast(el, { message: 'Failed.', variant: 'danger' });
    await el.updateComplete;
    // The alert is the toast itself, so a later info toast in the same region
    // is not dragged up to assertive with it.
    expect(toastEl(el)?.getAttribute('role')).toBe('alert');
    expect(region(el)?.getAttribute('role')).toBe('status');
    expect(region(el)?.getAttribute('aria-live')).toBe('polite');
  });

  it('re-announces only what changed', async () => {
    const el = await mount();
    dispatchNcToast(el, { message: 'First.', duration: 0 });
    await el.updateComplete;
    // aria-atomic would make every arrival re-read the whole column.
    expect(region(el)?.hasAttribute('aria-atomic')).toBe(false);
  });

  it('auto-dismisses after the default duration', async () => {
    vi.useFakeTimers();
    const el = await mount();
    dispatchNcToast(el, { message: 'Transient.' });
    await el.updateComplete;
    expect(toastEl(el)).toBeTruthy();

    vi.advanceTimersByTime(4000);
    await el.updateComplete;
    expect(toastEl(el)).toBeNull();
  });

  it('gives an actionable toast longer to be read', async () => {
    vi.useFakeTimers();
    const el = await mount();
    dispatchNcToast(el, { message: 'Undone.', action: { label: 'Redo', onClick: () => {} } });
    await el.updateComplete;

    vi.advanceTimersByTime(4000);
    await el.updateComplete;
    expect(toastEl(el)).toBeTruthy();

    vi.advanceTimersByTime(4000);
    await el.updateComplete;
    expect(toastEl(el)).toBeNull();
  });

  it('stays put when the duration is zero', async () => {
    vi.useFakeTimers();
    const el = await mount();
    dispatchNcToast(el, { message: 'Sticky.', duration: 0 });
    await el.updateComplete;

    vi.advanceTimersByTime(60_000);
    await el.updateComplete;
    expect(toastEl(el)).toBeTruthy();
  });

  it('invokes the action and dismisses on click', async () => {
    const onClick = vi.fn();
    const el = await mount();
    dispatchNcToast(el, { message: 'Undone.', action: { label: 'Redo', onClick } });
    await el.updateComplete;

    el.shadowRoot?.querySelector<HTMLButtonElement>('[data-toast-action]')?.click();
    await el.updateComplete;

    expect(onClick).toHaveBeenCalledOnce();
    expect(toastEl(el)).toBeNull();
  });

  it('survives an action that throws', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    const el = await mount();
    dispatchNcToast(el, {
      message: 'Boom.',
      action: {
        label: 'Explode',
        onClick: () => {
          throw new Error('nope');
        },
      },
    });
    await el.updateComplete;

    expect(() =>
      el.shadowRoot?.querySelector<HTMLButtonElement>('[data-toast-action]')?.click(),
    ).not.toThrow();
    await el.updateComplete;
    expect(toastEl(el)).toBeNull();
  });

  it('ignores an event with no message', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    const el = await mount();
    el.dispatchEvent(
      new CustomEvent('nc-toast', { detail: { message: '' }, bubbles: true, composed: true }),
    );
    await el.updateComplete;
    expect(toastEl(el)).toBeNull();
  });

  it('stops listening once disconnected', async () => {
    const el = await mount();
    el.remove();
    dispatchNcToast(document.body, { message: 'Too late.' });
    await el.updateComplete;
    expect(toastEl(el)).toBeNull();
  });

  it('seeds a toast from .initial', async () => {
    const el = document.createElement('wc-toast');
    el.initial = { message: 'Seeded.', duration: 0 };
    document.body.appendChild(el);
    await el.updateComplete;
    expect(toastEl(el)?.textContent).toContain('Seeded.');
  });

  it('seeds a whole stack from .initial', async () => {
    const el = document.createElement('wc-toast');
    el.initial = [
      { message: 'First.', duration: 0 },
      { message: 'Second.', duration: 0 },
    ];
    document.body.appendChild(el);
    await el.updateComplete;
    expect(messages(el)).toEqual(['First.', 'Second.']);
  });
});

describe('wc-toast dismissal', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  function closeButton(el: WcToast): HTMLButtonElement | null {
    return el.shadowRoot?.querySelector<HTMLButtonElement>('[data-toast-close]') ?? null;
  }

  it('gives a toast that never expires a close button', async () => {
    const el = await mount();
    // nigel-app's status error is exactly this shape: sticky, no action.
    dispatchNcToast(el, { message: 'Could not reach the server.', duration: 0 });
    await el.updateComplete;

    const close = closeButton(el);
    expect(close).toBeTruthy();
    expect(close?.getAttribute('aria-label')).toBe('Dismiss');

    close?.click();
    await el.updateComplete;
    expect(messages(el)).toEqual([]);
  });

  it('leaves the close button off a toast that expires on its own', async () => {
    const el = await mount();
    dispatchNcToast(el, { message: 'Saved.' });
    await el.updateComplete;
    expect(closeButton(el)).toBeNull();
  });

  it('leaves the close button off a sticky toast whose action ends it', async () => {
    const el = await mount();
    dispatchNcToast(el, {
      message: 'Import undone.',
      duration: 0,
      action: { label: 'Redo', onClick: () => {} },
    });
    await el.updateComplete;
    expect(closeButton(el)).toBeNull();
  });

  it('closes only the toast whose button was clicked', async () => {
    const el = await mount();
    dispatchNcToast(el, { message: 'Stays.', duration: 0 });
    dispatchNcToast(el, { message: 'Goes.', duration: 0 });
    await el.updateComplete;

    el.shadowRoot?.querySelectorAll<HTMLButtonElement>('[data-toast-close]')[1]?.click();
    await el.updateComplete;
    expect(messages(el)).toEqual(['Stays.']);
  });

  it('dismisses one toast by the id show() answered with', async () => {
    const el = await mount();
    const first = el.show({ message: 'First.', duration: 0 });
    el.show({ message: 'Second.', duration: 0 });
    await el.updateComplete;

    expect(first).not.toBeNull();
    el.dismiss(first!);
    await el.updateComplete;
    expect(messages(el)).toEqual(['Second.']);
  });
});

/**
 * The region rides the top layer, so what matters is *when* it is (re-)shown:
 * the top layer is ordered by open time, and a re-show jumps the region above
 * whatever opened since.
 */
describe('wc-toast top-layer promotion', () => {
  let calls: string[] = [];

  function trackPopover(): void {
    const open = new WeakSet<Element>();
    const realMatches = Element.prototype.matches;
    vi.spyOn(HTMLElement.prototype, 'showPopover').mockImplementation(function (
      this: HTMLElement,
    ) {
      open.add(this);
      calls.push('show');
    });
    vi.spyOn(HTMLElement.prototype, 'hidePopover').mockImplementation(function (
      this: HTMLElement,
    ) {
      open.delete(this);
      calls.push('hide');
    });
    vi.spyOn(Element.prototype, 'matches').mockImplementation(function (
      this: Element,
      selector: string,
    ) {
      if (selector === ':popover-open') return open.has(this);
      return realMatches.call(this, selector);
    });
  }

  beforeEach(() => {
    calls = [];
    trackPopover();
  });

  afterEach(() => {
    document.body.innerHTML = '';
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('promotes the region when a toast arrives', async () => {
    const el = await mount();
    expect(calls).toEqual([]);

    dispatchNcToast(el, { message: 'First.', duration: 0 });
    await el.updateComplete;
    expect(calls).toEqual(['show']);
  });

  it('re-promotes above anything opened since the last toast', async () => {
    const el = await mount();
    dispatchNcToast(el, { message: 'First.', duration: 0 });
    await el.updateComplete;
    dispatchNcToast(el, { message: 'Second.', duration: 0 });
    await el.updateComplete;
    expect(calls).toEqual(['show', 'hide', 'show']);
  });

  it('leaves the region alone when a toast expires with others behind it', async () => {
    vi.useFakeTimers();
    const el = await mount();
    dispatchNcToast(el, { message: 'Sticks.', duration: 0 });
    await el.updateComplete;
    dispatchNcToast(el, { message: 'Brief.', duration: 1000 });
    await el.updateComplete;
    calls = [];

    vi.advanceTimersByTime(1000);
    await el.updateComplete;

    expect(messages(el)).toEqual(['Sticks.']);
    // Re-showing here would jump the survivors above a modal opened after them.
    expect(calls).toEqual([]);
  });

  it('re-promotes a shrunken stack as soon as a new toast arrives', async () => {
    vi.useFakeTimers();
    const el = await mount();
    dispatchNcToast(el, { message: 'Sticks.', duration: 0 });
    dispatchNcToast(el, { message: 'Brief.', duration: 1000 });
    await el.updateComplete;
    vi.advanceTimersByTime(1000);
    await el.updateComplete;
    calls = [];

    dispatchNcToast(el, { message: 'Newest.', duration: 0 });
    await el.updateComplete;
    expect(calls).toEqual(['hide', 'show']);
  });

  it('withdraws the region once the last toast goes', async () => {
    vi.useFakeTimers();
    const el = await mount();
    dispatchNcToast(el, { message: 'Brief.', duration: 1000 });
    await el.updateComplete;
    calls = [];

    vi.advanceTimersByTime(1000);
    await el.updateComplete;
    expect(calls).toEqual(['hide']);
  });
});

describe('wc-toast stacking', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    vi.useRealTimers();
  });

  it('keeps earlier toasts visible, newest last', async () => {
    const el = await mount();
    dispatchNcToast(el, { message: 'Older.', duration: 0 });
    dispatchNcToast(el, { message: 'Newer.', duration: 0 });
    await el.updateComplete;
    expect(messages(el)).toEqual(['Older.', 'Newer.']);
  });

  it('drops the oldest past the visible limit', async () => {
    const el = await mount();
    for (let n = 1; n <= MAX_VISIBLE_TOASTS + 2; n += 1) {
      dispatchNcToast(el, { message: `Toast ${n}.`, duration: 0 });
    }
    await el.updateComplete;
    expect(messages(el)).toEqual(['Toast 3.', 'Toast 4.', 'Toast 5.']);
  });

  it('dismisses each toast on its own timer', async () => {
    vi.useFakeTimers();
    const el = await mount();
    dispatchNcToast(el, { message: 'Short.', duration: 1000 });
    dispatchNcToast(el, { message: 'Long.', duration: 5000 });
    await el.updateComplete;

    vi.advanceTimersByTime(1000);
    await el.updateComplete;
    expect(messages(el)).toEqual(['Long.']);

    vi.advanceTimersByTime(4000);
    await el.updateComplete;
    expect(messages(el)).toEqual([]);
  });

  it('leaves the other toasts alone when one action fires', async () => {
    const el = await mount();
    dispatchNcToast(el, { message: 'Stays.', duration: 0 });
    dispatchNcToast(el, {
      message: 'Goes.',
      duration: 0,
      action: { label: 'Redo', onClick: () => {} },
    });
    await el.updateComplete;

    el.shadowRoot?.querySelector<HTMLButtonElement>('[data-toast-action]')?.click();
    await el.updateComplete;

    expect(messages(el)).toEqual(['Stays.']);
  });

  it('dismiss() clears the whole stack', async () => {
    const el = await mount();
    dispatchNcToast(el, { message: 'One.', duration: 0 });
    dispatchNcToast(el, { message: 'Two.', duration: 0 });
    await el.updateComplete;

    el.dismiss();
    await el.updateComplete;
    expect(messages(el)).toEqual([]);
  });
});

/**
 * Where the toasts actually land.
 *
 * jsdom does no layout, so these resolve the component's own stylesheet into
 * viewport pixels — see `preview/css-geometry.ts` — and assert the resulting
 * boxes, not the declarations that produced them.
 */
describe('wc-toast placement', () => {
  const css = styleText(WcToast);
  const vars = customProperties(css, ':host');
  const regionRules = resolvedDeclarations(css, '.region');
  const toastRules = resolvedDeclarations(css, '.toast');

  // Three chips of a plausible height, plus the gaps between them.
  const STACK_HEIGHT = MAX_VISIBLE_TOASTS * 44 + (MAX_VISIBLE_TOASTS - 1) * 8;

  const viewports: [string, Viewport][] = [
    ['desktop', { width: 1440, height: 900 }],
    ['laptop', { width: 1024, height: 768 }],
    ['phone', { width: 375, height: 667 }],
  ];

  function place(viewport: Viewport, messageWidth: number) {
    const region = fixedBox(regionRules, {
      viewport,
      content: { width: messageWidth, height: STACK_HEIGHT },
      vars,
    });
    if (!region) {
      throw new Error('the region resolves to no box: nothing anchors it to the viewport');
    }
    const maxWidth =
      resolveLength(toastRules.get('max-width') ?? 'none', {
        viewport,
        percentBasis: region.width,
        vars,
      }) ?? Number.POSITIVE_INFINITY;
    const width = Math.min(messageWidth, maxWidth);
    // The column aligns its chips to the region's end edge.
    const toast = { ...region, width, left: region.right - width };
    return { region, toast, wrapped: width < messageWidth };
  }

  it.each(viewports)('anchors the region inside a %s viewport', (_name, viewport) => {
    const { region } = place(viewport, 280);
    expect(isInsideViewport(region, viewport)).toBe(true);
  });

  it.each(viewports)('keeps a short toast inside a %s viewport', (_name, viewport) => {
    const { toast } = place(viewport, 280);
    expect(isInsideViewport(toast, viewport)).toBe(true);
  });

  it.each(viewports)(
    'wraps a long message instead of running off a %s viewport',
    (_name, viewport) => {
      const natural = 2400;
      const { toast, wrapped } = place(viewport, natural);
      expect(wrapped).toBe(true);
      expect(toast.width).toBeLessThan(natural);
      expect(isInsideViewport(toast, viewport)).toBe(true);
    },
  );

  it('lands in the same corner whatever the viewport', () => {
    const corners = viewports.map(([, viewport]) => {
      const { region } = place(viewport, 280);
      return {
        fromRight: viewport.width - region.right,
        fromBottom: viewport.height - region.bottom,
      };
    });
    expect(new Set(corners.map((corner) => JSON.stringify(corner))).size).toBe(1);
    expect(corners[0]!.fromRight).toBeGreaterThan(0);
    expect(corners[0]!.fromBottom).toBeGreaterThan(0);
  });

  it('keeps a full stack clear of the top of the shortest viewport', () => {
    const [, viewport] = viewports.reduce((shortest, entry) =>
      entry[1].height < shortest[1].height ? entry : shortest,
    );
    const { region } = place(viewport, 280);
    expect(region.height).toBe(STACK_HEIGHT);
    expect(region.top).toBeGreaterThan(0);
  });
});

describePreviewA11y(preview);

describePrintHiding(WcToast, ':host');
