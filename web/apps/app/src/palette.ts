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

/** ⌘K or Ctrl+K, on any platform, with no other modifier. */
export function isPaletteShortcut(event: KeyboardEvent): boolean {
  return (
    (event.metaKey || event.ctrlKey) &&
    !event.altKey &&
    !event.shiftKey &&
    event.key.toLowerCase() === 'k'
  );
}
