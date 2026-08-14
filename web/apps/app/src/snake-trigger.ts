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
 * Would this keystroke land in a form control or a dialog?
 *
 * Two sources, because neither covers the other: the composed path is where
 * the event actually came from, and the focus chain catches a dialog holding
 * the screen while the keystroke was delivered to the body — which is what an
 * open dialog with nothing focusable inside it looks like.
 */
export function isTypingContext(event: KeyboardEvent): boolean {
  const path = event
    .composedPath()
    .filter((node): node is Element => node instanceof Element);
  return [...path, ...focusChain()].some(blocks);
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
