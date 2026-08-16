/**
 * Awaiting a component's `updateComplete` says nothing about the components it
 * rendered: a `wc-icon-*` inside a freshly rendered shadow root has not drawn
 * its own SVG yet. Anything asserting on an icon's insides has to wait for the
 * icon too, which is what this is for.
 */
export async function settle<T extends Element>(node: T | null | undefined): Promise<T> {
  if (!node) throw new Error('settle() was given no element');
  await (node as T & { updateComplete?: Promise<unknown> }).updateComplete;
  return node;
}

/** The `<svg>` a rendered icon draws, once it has drawn it. */
export async function iconSvg(node: Element | null | undefined): Promise<SVGElement> {
  const icon = await settle(node);
  const svg = icon.shadowRoot?.querySelector('svg');
  if (!svg) throw new Error(`${icon.tagName.toLowerCase()} rendered no <svg>`);
  return svg;
}
