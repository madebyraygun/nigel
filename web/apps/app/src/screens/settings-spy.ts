/**
 * Which Settings section the reader is in, from where each one starts.
 *
 * `tops` are each section's top edge relative to the top of the scrolling
 * area, in page order. The active section is the last one whose top has
 * reached `threshold` pixels from the top; before any has, the first. Once
 * the area is scrolled to the bottom the last section is active, because a
 * short last section can never scroll up to the threshold and would otherwise
 * never be highlighted at all.
 */
export function activeSection(
  tops: readonly { id: string; top: number }[],
  atBottom: boolean,
  threshold = 80,
): string {
  if (tops.length === 0) return '';
  if (atBottom) return tops[tops.length - 1].id;
  let active = tops[0].id;
  for (const { id, top } of tops) {
    if (top <= threshold) active = id;
  }
  return active;
}

/** Within a pixel of the end, which is where a fractional scroll stops. */
export function scrolledToBottom(area: {
  scrollTop: number;
  clientHeight: number;
  scrollHeight: number;
}): boolean {
  return area.scrollHeight > area.clientHeight
    && area.scrollTop + area.clientHeight >= area.scrollHeight - 1;
}

/**
 * The nearest ancestor that scrolls, walking out through slots and shadow
 * roots — a screen sits in the shell's content area, which is in another
 * tree. `null` when nothing does, in which case the document scrolls.
 */
export function scrollingAncestor(start: Element): HTMLElement | null {
  let node: Node | null = start;
  while (node) {
    const next: Node | null = (node instanceof Element && node.assignedSlot) || node.parentNode;
    const parent: Node | null = next instanceof ShadowRoot ? next.host : next;
    if (parent instanceof HTMLElement) {
      const overflow = getComputedStyle(parent).overflowY;
      if (overflow === 'auto' || overflow === 'scroll') return parent;
    }
    node = parent;
  }
  return null;
}
