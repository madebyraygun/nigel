/**
 * @vitest-environment jsdom
 *
 * The guards are about focus, shadow roots and event paths, so this one is a
 * DOM test even though the module it covers is pure.
 */
import { describe, it, expect, afterEach } from 'vitest';
import {
  SNAKE_KEY,
  deepActiveElement,
  isSnakeTrigger,
  isTypingContext,
  snakeAllowedOnBoot,
} from './snake-trigger.js';
import type { BootPhase } from './state/app-store.js';

/**
 * Ask the predicate from a window listener, mid-dispatch, which is where
 * `nigel-app` asks it. Not incidental: `composedPath()` is empty once dispatch
 * has finished, so a test that kept the event and asked afterwards would see
 * no path at all and pass whatever it was given.
 */
function ask(
  predicate: (event: KeyboardEvent) => boolean,
  target: EventTarget,
  init: KeyboardEventInit = {},
): boolean {
  let answer = false;
  const listener = (event: Event) => {
    answer = predicate(event as KeyboardEvent);
  };

  window.addEventListener('keydown', listener);
  target.dispatchEvent(
    new KeyboardEvent('keydown', {
      key: SNAKE_KEY,
      bubbles: true,
      composed: true,
      cancelable: true,
      ...init,
    }),
  );
  window.removeEventListener('keydown', listener);

  return answer;
}

const fires = (target: EventTarget, init: KeyboardEventInit = {}) =>
  ask(isSnakeTrigger, target, init);

/** A host with an open shadow root holding `inner`, appended to the page. */
function shadowHost(inner: HTMLElement): { host: HTMLElement; inner: HTMLElement } {
  const host = document.createElement('div');
  const root = host.attachShadow({ mode: 'open' });
  root.appendChild(inner);
  document.body.appendChild(host);
  return { host, inner };
}

describe('isSnakeTrigger', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('fires on a bare s from the page body', () => {
    expect(fires(document.body)).toBe(true);
  });

  it('does not fire on any other key', () => {
    expect(fires(document.body, { key: 'a' })).toBe(false);
  });

  it.each(['altKey', 'ctrlKey', 'metaKey', 'shiftKey'] as const)(
    'does not fire with %s held',
    (modifier) => {
      expect(fires(document.body, { [modifier]: true })).toBe(false);
    },
  );

  it('does not fire mid-composition, where s is part of a word', () => {
    expect(fires(document.body, { isComposing: true })).toBe(false);
  });

  it('does not fire on the repeats of a held key', () => {
    expect(fires(document.body, { repeat: true })).toBe(false);
  });

  it('does not fire on an event something else has already handled', () => {
    const handled = (event: KeyboardEvent) => {
      event.preventDefault();
      return isSnakeTrigger(event);
    };
    expect(ask(handled, document.body)).toBe(false);
  });
});

describe('the typing guard', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it.each([
    'input',
    'textarea',
    'select',
    'button',
    'wa-input',
    'wa-textarea',
    'wa-select',
    'wa-switch',
    'wa-radio-group',
  ])('refuses a keystroke from inside %s', (tag) => {
    const control = document.createElement(tag);
    document.body.appendChild(control);
    expect(fires(control)).toBe(false);
  });

  it('refuses a keystroke from a contenteditable region', () => {
    const editor = document.createElement('div');
    editor.setAttribute('contenteditable', 'true');
    document.body.appendChild(editor);
    expect(ask(isTypingContext, editor)).toBe(true);
  });

  it.each(['textbox', 'searchbox', 'combobox', 'spinbutton'])(
    'refuses a keystroke from role=%s, whatever the tag is',
    (role) => {
      const widget = document.createElement('div');
      widget.setAttribute('role', role);
      document.body.appendChild(widget);
      expect(fires(widget)).toBe(false);
    },
  );

  it('refuses a keystroke from inside a dialog', () => {
    const dialog = document.createElement('wa-dialog');
    const inside = document.createElement('span');
    dialog.appendChild(inside);
    document.body.appendChild(dialog);
    expect(fires(inside)).toBe(false);
  });

  it('refuses a keystroke from inside anything marked aria-modal', () => {
    const overlay = document.createElement('div');
    overlay.setAttribute('aria-modal', 'true');
    const inside = document.createElement('span');
    overlay.appendChild(inside);
    document.body.appendChild(overlay);
    expect(fires(inside)).toBe(false);
  });

  it('sees a field inside a shadow root, which activeElement alone does not', () => {
    const field = document.createElement('input');
    const { host, inner } = shadowHost(field);
    inner.focus();

    expect(document.activeElement).toBe(host);
    expect(deepActiveElement()).toBe(field);
    // Dispatched at the host, as a retargeted event arrives at the window.
    expect(fires(host)).toBe(false);
  });

  it('refuses while a field elsewhere holds the focus', () => {
    const dialog = document.createElement('wa-dialog');
    const field = document.createElement('input');
    dialog.appendChild(field);
    document.body.appendChild(dialog);
    field.focus();

    // The event names only the body; the focus chain is what catches this.
    expect(fires(document.body)).toBe(false);
  });

  /**
   * The third source. A modal with nothing focusable in it leaves the focus on
   * the body, and the keystroke is then delivered to the body too — so neither
   * the composed path nor the focus chain has anything in it to recognise,
   * while a dialog is very much on screen.
   */
  describe('an open modal, wherever the keystroke went', () => {
    it.each(['<dialog open></dialog>', '<div aria-modal="true"></div>'])(
      'refuses while %s is open in the page',
      (markup) => {
        document.body.insertAdjacentHTML('beforeend', markup);
        expect(fires(document.body)).toBe(false);
      },
    );

    it('refuses while a dialog is open inside a shadow root', () => {
      // Which is where every dialog in this app is: wc-confirm renders a
      // wa-dialog, which renders a native dialog, two boundaries down.
      const inner = document.createElement('wa-dialog');
      inner.setAttribute('open', '');
      shadowHost(inner);

      expect(fires(document.body)).toBe(false);
    });

    it('fires again once the dialog has gone', () => {
      document.body.insertAdjacentHTML('beforeend', '<dialog open></dialog>');
      expect(fires(document.body)).toBe(false);

      document.querySelector('dialog')?.remove();
      expect(fires(document.body)).toBe(true);
    });
  });

  it('allows a keystroke from ordinary page furniture', () => {
    const heading = document.createElement('h1');
    document.body.appendChild(heading);
    expect(fires(heading)).toBe(true);
  });
});

describe('snakeAllowedOnBoot', () => {
  it('allows the game only where a dashboard is actually rendered', () => {
    expect(snakeAllowedOnBoot('ready')).toBe(true);
  });

  it.each(['starting', 'locked', 'failed'] as const)(
    'refuses over the %s screen',
    (phase) => {
      expect(snakeAllowedOnBoot(phase)).toBe(false);
    },
  );

  it('covers every phase the store can be in', () => {
    // The switch is exhaustive, so this list and BootPhase cannot disagree
    // without failing the typecheck — the assertion is that it is a list at
    // all, and that none of it throws.
    const phases: BootPhase[] = ['starting', 'locked', 'failed', 'ready'];
    expect(phases.map(snakeAllowedOnBoot)).toEqual([false, false, false, true]);
  });
});

describe('deepActiveElement', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('answers null when nothing has focus', () => {
    const detached = document.createElement('div').attachShadow({ mode: 'open' });
    expect(deepActiveElement(detached)).toBeNull();
  });

  it('descends through nested shadow roots', () => {
    const field = document.createElement('input');
    const middle = document.createElement('div');
    middle.attachShadow({ mode: 'open' }).appendChild(field);
    shadowHost(middle);
    field.focus();

    expect(deepActiveElement()).toBe(field);
  });
});
