import { describe, it, expect, afterEach } from 'vitest';
import './wc-shortcut-help.js';
import { WcShortcutHelp, type ShortcutHint } from './wc-shortcut-help.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { styleText } from '../../preview/controls-suite.js';
import preview from './wc-shortcut-help.preview.js';

const hints: ShortcutHint[] = [
  { keys: ['ArrowUp', 'ArrowDown'], display: '↑ ↓', description: 'Move between rows' },
  { keys: ['f'], display: 'f', description: 'Flag or unflag the row' },
];

async function mount(props: Partial<WcShortcutHelp> = {}): Promise<WcShortcutHelp> {
  const el = document.createElement('wc-shortcut-help');
  Object.assign(el, { shortcuts: hints, ...props });
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function trigger(el: WcShortcutHelp): HTMLButtonElement {
  const button = el.shadowRoot?.querySelector<HTMLButtonElement>('.trigger');
  if (!button) throw new Error('no trigger');
  return button;
}

function panel(el: WcShortcutHelp): HTMLElement {
  const found = el.shadowRoot?.querySelector<HTMLElement>('.panel');
  if (!found) throw new Error('no panel');
  return found;
}

function pressEscape(target: EventTarget): void {
  target.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, composed: true }),
  );
}

describe('wc-shortcut-help', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('starts closed, with the panel out of the accessibility tree', async () => {
    const el = await mount();
    expect(trigger(el).getAttribute('aria-expanded')).toBe('false');
    expect(panel(el).hasAttribute('hidden')).toBe(true);
  });

  it('opens on the trigger and lists every shortcut it was given', async () => {
    const el = await mount();
    trigger(el).click();
    await el.updateComplete;

    expect(trigger(el).getAttribute('aria-expanded')).toBe('true');
    expect(panel(el).hasAttribute('hidden')).toBe(false);

    const terms = [...panel(el).querySelectorAll('dt')].map((dt) => dt.textContent?.trim());
    const details = [...panel(el).querySelectorAll('dd')].map((dd) => dd.textContent?.trim());
    expect(terms).toEqual(['↑ ↓', 'f']);
    expect(details).toEqual(['Move between rows', 'Flag or unflag the row']);
  });

  it('points the trigger at the panel it controls', async () => {
    const el = await mount({ open: true });
    expect(trigger(el).getAttribute('aria-controls')).toBe(panel(el).id);
    expect(panel(el).id).not.toBe('');
  });

  it('gives two on one page different panel ids', async () => {
    const first = await mount();
    const second = await mount();
    expect(panel(first).id).not.toBe(panel(second).id);
  });

  it('closes on the trigger again', async () => {
    const el = await mount({ open: true });
    trigger(el).click();
    await el.updateComplete;
    expect(el.open).toBe(false);
  });

  it('closes on Escape and hands focus back to the trigger', async () => {
    const el = await mount({ open: true });
    trigger(el).focus();

    const event = new KeyboardEvent('keydown', {
      key: 'Escape',
      bubbles: true,
      composed: true,
      cancelable: true,
    });
    trigger(el).dispatchEvent(event);
    await el.updateComplete;
    await el.updateComplete;

    expect(el.open).toBe(false);
    expect(el.shadowRoot?.activeElement).toBe(trigger(el));
    expect(event.defaultPrevented).toBe(true);
  });

  it('closes on an Escape meant for something else without taking the key or the focus', async () => {
    // The register's inline editor cancels on Escape. An open legend that
    // stole it would leave the edit open and yank focus to the trigger.
    const el = await mount({ open: true });
    const elsewhere = document.createElement('input');
    document.body.appendChild(elsewhere);
    elsewhere.focus();

    const event = new KeyboardEvent('keydown', {
      key: 'Escape',
      bubbles: true,
      composed: true,
      cancelable: true,
    });
    elsewhere.dispatchEvent(event);
    await el.updateComplete;
    await el.updateComplete;

    expect(el.open).toBe(false);
    expect(event.defaultPrevented).toBe(false);
    expect(document.activeElement).toBe(elsewhere);
  });

  it('closes when focus leaves it', async () => {
    const el = await mount({ open: true });
    trigger(el).focus();

    const elsewhere = document.createElement('input');
    document.body.appendChild(elsewhere);
    elsewhere.focus();
    elsewhere.dispatchEvent(new FocusEvent('focusin', { bubbles: true, composed: true }));
    await el.updateComplete;

    expect(el.open).toBe(false);
    expect(document.activeElement).toBe(elsewhere);
  });

  it('stays open while focus moves inside it', async () => {
    const el = await mount({ open: true });
    trigger(el).focus();
    trigger(el).dispatchEvent(new FocusEvent('focusin', { bubbles: true, composed: true }));
    await el.updateComplete;
    expect(el.open).toBe(true);
  });

  it('leaves Escape alone while it is closed', async () => {
    const el = await mount();
    let seen = 0;
    document.addEventListener('keydown', () => (seen += 1));
    pressEscape(document.body);
    await el.updateComplete;
    expect(seen).toBe(1);
  });

  it('closes on a press outside it', async () => {
    const el = await mount({ open: true });
    const outside = document.createElement('button');
    document.body.appendChild(outside);

    outside.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, composed: true }));
    await el.updateComplete;

    expect(el.open).toBe(false);
  });

  it('stays open for a press on its own panel', async () => {
    const el = await mount({ open: true });
    panel(el).dispatchEvent(
      new PointerEvent('pointerdown', { bubbles: true, composed: true }),
    );
    await el.updateComplete;
    expect(el.open).toBe(true);
  });

  it('stops listening once it is off the page', async () => {
    const el = await mount({ open: true });
    el.remove();
    // Nothing to assert but the absence of a throw: an open popover that had
    // been removed used to leave two document listeners behind.
    pressEscape(document.body);
    expect(el.isConnected).toBe(false);
  });

  it('is reachable and operable from the keyboard alone', async () => {
    const el = await mount();
    // A real button: it is in the tab order and Enter/Space activate it
    // without a click handler of our own.
    expect(trigger(el).tagName).toBe('BUTTON');
    expect(trigger(el).type).toBe('button');
    expect(trigger(el).tabIndex).toBe(0);
  });

  it('shifts back onto the screen when the trigger sits at the left edge', async () => {
    // The panel is right-anchored, so a trigger on the second line of a
    // wrapped toolbar would otherwise hang off the left of the window.
    const el = await mount();
    const box = panel(el);
    box.getBoundingClientRect = () =>
      ({ left: -120, right: 136, width: 256 }) as DOMRect;

    el.show();
    await el.updateComplete;

    expect(box.style.transform).toBe('translateX(128px)');
  });

  it('leaves a panel that already fits alone', async () => {
    const el = await mount();
    const box = panel(el);
    box.getBoundingClientRect = () =>
      ({ left: 200, right: 456, width: 256 }) as DOMRect;

    el.show();
    await el.updateComplete;

    expect(box.style.transform).toBe('');
  });

  describe('the panel is anchored rather than inline', () => {
    const text = styleText(WcShortcutHelp);

    it('takes the panel out of flow, against the trigger', () => {
      // The point of the change: the legend used to be a `details` block in
      // the toolbar row, so opening it pushed the register down the page.
      expect(text).toMatch(/:host\s*{[^}]*position:\s*relative/);
      expect(text).toMatch(/\.panel\s*{[^}]*position:\s*absolute/);
      expect(text).toMatch(/\.panel\s*{[^}]*top:\s*calc\(100%/);
    });

    it('stays off a printed page', () => {
      expect(text).toMatch(/@media print[\s\S]*:host[^{]*{[^}]*display:\s*none/);
    });
  });
});

describePreviewA11y(preview);
