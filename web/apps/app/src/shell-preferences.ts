/**
 * What the app shell remembers about this device, and how it was opened.
 *
 * Beside the app rather than in `@nigel/ui`: which host is running and what
 * this browser has stored are composition-layer business
 * (docs/native-feel.md's placement rule). The components are handed the
 * answers as properties.
 */
import type { StatusLineInfo } from '@nigel/ui';
import type { StatusResponse } from './api/types.js';

/**
 * What the status line shows about the open book, from the server's status.
 * Absent while there is no status to show.
 */
export function statusLineOf(
  status: Pick<
    StatusResponse,
    'dataDir' | 'dataDirDisplay' | 'encrypted' | 'version' | 'distribution'
  > | null,
): StatusLineInfo | undefined {
  if (!status) return undefined;
  return {
    path: status.dataDirDisplay,
    fullPath: status.dataDir,
    encrypted: status.encrypted,
    version: status.version,
    distribution: status.distribution,
  };
}

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

/**
 * Where the register's editing style lives. Per device, like the colour mode:
 * set, a row is edited in place in the list; unset, in a dialog.
 */
export const REGISTER_INLINE_EDIT_KEY = 'nigel.register-inline-edit';

/** Whether this device edits register rows in place rather than in a dialog. */
export function readRegisterInlineEdit(
  storage: PreferenceStorage | undefined = defaultStorage(),
): boolean {
  try {
    return storage?.getItem(REGISTER_INLINE_EDIT_KEY) === 'true';
  } catch {
    return false;
  }
}

/** Remember the editing style. A refusal is silent: it still holds for this page. */
export function writeRegisterInlineEdit(
  inline: boolean,
  storage: PreferenceStorage | undefined = defaultStorage(),
): void {
  try {
    storage?.setItem(REGISTER_INLINE_EDIT_KEY, String(inline));
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

/** Whether this is an Apple platform, where Command rather than Control leads a shortcut. */
export function isApplePlatform(platform: string = navigator.platform): boolean {
  return /Mac|iPhone|iPad|iPod/.test(platform);
}

/**
 * The palette shortcut as the keycap shows it and as `aria-keyshortcuts`
 * spells it: Command on Apple platforms, Control everywhere else.
 */
export function paletteShortcut(platform: string = navigator.platform): {
  keycap: string;
  aria: string;
} {
  return isApplePlatform(platform)
    ? { keycap: '⌘K', aria: 'Meta+K' }
    : { keycap: 'Ctrl K', aria: 'Control+K' };
}
