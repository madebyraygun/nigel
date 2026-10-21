/**
 * What the app shell remembers about this device, and how it was opened.
 *
 * Beside the app rather than in `@nigel/ui`: which host is running and what
 * this browser has stored are composition-layer business
 * (docs/native-feel.md's placement rule). The components are handed the
 * answers as properties.
 */

/** Where the sidebar's folded state lives. Per device, like the colour mode. */
export const SIDEBAR_STORAGE_KEY = 'nigel.sidebar-collapsed';

type PreferenceStorage = Pick<Storage, 'getItem' | 'setItem'>;

function defaultStorage(): PreferenceStorage | undefined {
  return typeof localStorage === 'undefined' ? undefined : localStorage;
}

/**
 * Whether the sidebar was last left folded to its rail. Every access is
 * wrapped: with storage disabled `localStorage` throws on access, and a
 * sidebar preference is not a reason for the app to fail to boot.
 */
export function readSidebarCollapsed(
  storage: PreferenceStorage | undefined = defaultStorage(),
): boolean {
  try {
    return storage?.getItem(SIDEBAR_STORAGE_KEY) === 'true';
  } catch {
    return false;
  }
}

/** Remember the folded state. A refusal is silent: it still holds for this page. */
export function writeSidebarCollapsed(
  collapsed: boolean,
  storage: PreferenceStorage | undefined = defaultStorage(),
): void {
  try {
    storage?.setItem(SIDEBAR_STORAGE_KEY, String(collapsed));
  } catch {
    // Storage disabled; the choice lasts until a reload.
  }
}

/** The query parameter a native host opens the page with. */
export const HOSTED_PARAM = 'shell';

/**
 * Whether a native window is hosting the SPA and drawing its own sidebar.
 *
 * The Swift shell says so with `shell=native`, either in the query
 * (`/?shell=native#/dashboard`) or in the hash route's own parameters
 * (`#/settings?shell=native`, how it opens a screen directly). Read once from
 * the URL the page was opened with: the hash form disappears on the first
 * navigation, and the host does not change for the life of the page.
 * `parseHash` strips the parameter, so no screen ever sees it.
 */
export function isHostedShell(
  search: string = window.location.search,
  hash: string = window.location.hash,
): boolean {
  const query = hash.includes('?') ? hash.slice(hash.indexOf('?') + 1) : '';
  return [search, query].some(
    (params) => new URLSearchParams(params).get(HOSTED_PARAM) === 'native',
  );
}

/**
 * The palette shortcut as the keycap shows it and as `aria-keyshortcuts`
 * spells it: Command on Apple platforms, Control everywhere else.
 */
export function paletteShortcut(platform: string = navigator.platform): {
  keycap: string;
  aria: string;
} {
  return /Mac|iPhone|iPad|iPod/.test(platform)
    ? { keycap: '⌘K', aria: 'Meta+K' }
    : { keycap: 'Ctrl K', aria: 'Control+K' };
}
