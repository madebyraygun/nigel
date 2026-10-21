/**
 * What the ⌘K palette lists, and the chord that toggles it.
 *
 * Beside the app rather than in `@nigel/ui`: the screens come from the
 * registry and the book from `/api/status`, and `wc-command-palette` is handed
 * both.
 */
import type { PaletteBook, PaletteScreen } from '@nigel/ui';
import type { StatusResponse } from './api/index.js';
import { SCREENS } from './screens/registry.js';

/** Every screen a person can choose, in sidebar order, by its full title. */
export function paletteScreens(): PaletteScreen[] {
  return [...SCREENS.values()]
    .filter((def) => def.inNav)
    .map((def) => ({ id: def.id, label: def.title, icon: def.icon }));
}

/**
 * The open book, the only one there is until books can be switched. Its id is
 * the data directory, which is also what seeds the breadcrumb's monogram, so
 * the two tiles share a colour.
 */
export function paletteBooks(status: StatusResponse | null, name: string): PaletteBook[] {
  if (!status) return [];
  return [
    {
      id: status.dataDir,
      name,
      kind: status.profile === 'personal' ? 'Personal' : 'Business',
      path: status.dataDir,
      encrypted: status.encrypted,
      open: true,
    },
  ];
}

/**
 * The chord the keycap advertises: ⌘K on Apple platforms, Ctrl+K elsewhere,
 * with no other modifier. Only one per platform, so Ctrl+K in a Mac text
 * field keeps its system meaning. A held key and a keystroke inside an IME
 * composition are not the chord.
 */
export function isPaletteShortcut(event: KeyboardEvent, apple: boolean): boolean {
  if (event.repeat || event.isComposing || event.keyCode === 229) return false;
  const primary = apple ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
  return primary && !event.altKey && !event.shiftKey && event.key.toLowerCase() === 'k';
}
