/**
 * The key that opens Snake, and the rules about when it may not.
 *
 * The TUI puts Snake on `s` from its home screen. The browser hides the same
 * game behind the same key and says so nowhere, which only works if the key
 * still belongs to whoever is typing: `s` is a letter before it is a shortcut,
 * so the moment focus is in a field or a dialog this is not a trigger at all.
 *
 * Pure, and separate from `nigel-app` on purpose — "does this keystroke open
 * the game" is the whole of the risk here, and it is worth being able to test
 * it against a hand-built event rather than through a mounted application.
 */
import type { BootPhase } from './state/app-store.js';

/** The letter, matching the TUI's dashboard menu. */
export const SNAKE_KEY = 's';

/**
 * Elements a keystroke belongs to rather than to the page.
 *
 * The `wa-*` half is this app's Web Awesome vocabulary. They are listed by
 * name because a custom element has no `HTMLInputElement` to be an instance
 * of and reports no `isContentEditable`: from the outside, `wa-input` is a
 * tag with a shadow root, so the tag is what there is to recognise.
 */
const FORM_CONTROLS = new Set([
  'input',
  'textarea',
  'select',
  'option',
  'optgroup',
  'button',
  'datalist',
  'wa-input',
  'wa-textarea',
  'wa-select',
  'wa-option',
  'wa-radio',
  'wa-radio-group',
  'wa-switch',
  'wa-checkbox',
  'wa-button',
  'wa-slider',
  'wa-color-picker',
]);

/** Anything that has taken over the screen and should keep the keyboard. */
const DIALOGS = new Set(['dialog', 'wa-dialog', 'wa-drawer']);

/** The same set as a selector, for finding one that nothing is focused inside. */
const OPEN_MODAL = 'dialog[open], wa-dialog[open], wa-drawer[open], [aria-modal="true"]';

/** ARIA saying the same thing an element's tag might not. */
const BLOCKING_ROLES = new Set([
  'textbox',
  'searchbox',
  'combobox',
  'spinbutton',
  'listbox',
  'menu',
  'menuitem',
  'dialog',
  'alertdialog',
]);

function blocks(element: Element): boolean {
  const tag = element.localName;
  if (FORM_CONTROLS.has(tag) || DIALOGS.has(tag)) return true;

  const role = element.getAttribute('role');
  if (role && BLOCKING_ROLES.has(role)) return true;
  if (element.getAttribute('aria-modal') === 'true') return true;

  // The attribute rather than `isContentEditable`: inheritance is already
  // covered by walking the chain, and the property is one of the things jsdom
  // does not implement, so the guard would be untestable through it.
  const editable = element.getAttribute('contenteditable');
  if (editable !== null && editable !== 'false') return true;

  return false;
}

/**
 * The focused element, following shadow roots down to the real one.
 *
 * `document.activeElement` stops at the outermost custom element — for a
 * cursor sitting in the register's inline editor it answers `nigel-app` — so
 * every check here would pass on an element that is not what has the caret.
 */
export function deepActiveElement(root: DocumentOrShadowRoot = document): Element | null {
  const active = root.activeElement;
  if (!active) return null;
  const nested = (active as Element & { shadowRoot?: ShadowRoot | null }).shadowRoot;
  return nested ? (deepActiveElement(nested) ?? active) : active;
}

/** The focused element and everything it sits inside, shadow hosts included. */
function focusChain(): Element[] {
  const chain: Element[] = [];
  let node: Node | null = deepActiveElement();

  while (node) {
    if (node instanceof Element) chain.push(node);
    const parent: Node | null = node.parentNode;
    node = parent instanceof ShadowRoot ? parent.host : parent;
  }

  return chain;
}

/**
 * Is a modal open anywhere on the page?
 *
 * Shadow roots are walked rather than queried past, because every dialog in
 * this app is inside one — `wc-confirm` renders a `wa-dialog`, which renders a
 * native `dialog`, two boundaries down from the document. A flat
 * `document.querySelector` would find none of them and the guard would be
 * dead code that reads as protection. The walk costs a tree traversal on the
 * `s` keystrokes that get this far, and nothing on any other key.
 */
export function hasOpenModal(root: Document | ShadowRoot = document): boolean {
  if (root.querySelector(OPEN_MODAL)) return true;

  for (const element of root.querySelectorAll('*')) {
    const nested = (element as Element & { shadowRoot?: ShadowRoot | null }).shadowRoot;
    if (nested && hasOpenModal(nested)) return true;
  }

  return false;
}

/**
 * Would this keystroke land in a form control or a dialog?
 *
 * Three sources, because no one of them covers the others. The composed path
 * is where the event actually came from. The focus chain is what has the
 * caret, which the path misses when the event is retargeted. And a modal open
 * anywhere catches the case both of those miss together: a dialog with nothing
 * focusable inside it leaves the focus on the body, and the keystroke is then
 * delivered to the body too, so neither the path nor the chain has anything in
 * it to recognise while a dialog is very much on screen.
 */
export function isTypingContext(event: KeyboardEvent): boolean {
  const path = event
    .composedPath()
    .filter((node): node is Element => node instanceof Element);
  if ([...path, ...focusChain()].some(blocks)) return true;
  return hasOpenModal();
}

/**
 * Whether this keydown is the hidden Snake trigger.
 *
 * A bare, unmodified `s` and nothing else: a modifier makes it somebody's
 * browser shortcut, `Shift` makes it a capital letter, and an IME composition
 * is mid-word in a language whose input this has no business interrupting. A
 * repeat is dropped so a held key opens the game once.
 */
export function isSnakeTrigger(event: KeyboardEvent): boolean {
  if (event.key !== SNAKE_KEY) return false;
  if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return false;
  if (event.repeat || event.isComposing) return false;
  if (event.defaultPrevented) return false;
  return !isTypingContext(event);
}

/**
 * Is there a dashboard under the game to come back to?
 *
 * Only `ready` renders one. `failed` still draws the shell, but around a
 * retry banner over a dashboard that could not load — not a screen to cover
 * with a snake. Written as an exhaustive switch on purpose: a phase added to
 * `BootPhase` later fails this to compile rather than falling through to a
 * default that quietly lets the game open over it.
 */
export function snakeAllowedOnBoot(boot: BootPhase): boolean {
  switch (boot) {
    case 'ready':
      return true;
    case 'starting':
    case 'locked':
    case 'failed':
      return false;
    default: {
      const unhandled: never = boot;
      return unhandled;
    }
  }
}
