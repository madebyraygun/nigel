/**
 * The writer for the light/dark contract `tokens/color.ts` already defines.
 *
 * The CSS side is three states expressed as two classes on `<html>`:
 *
 * - no class    — follow the system, via `@media (prefers-color-scheme: dark)`
 * - `light-mode` — opt out of that media query
 * - `dark-mode`  — force dark
 *
 * `system` therefore writes *nothing*, which is the whole reason the app
 * tracks an OS change live with no listener and no reload: the browser
 * re-evaluates the media query itself. Resolving `system` in JavaScript and
 * always writing an explicit class would make that media query dead code and
 * put OS tracking behind something that can fail.
 *
 * This lives in `@nigel/theme` rather than in `@nigel/ui` because the class
 * names, the media query and the print interaction are all defined here, and
 * splitting a writer from the contract it writes against is how the two drift.
 * It is the package's only behaviour module and it imports nothing — not Lit,
 * not the DOM lib beyond the two structural types below.
 */

export const COLOR_MODES = ['light', 'dark', 'system'] as const;
export type ColorMode = (typeof COLOR_MODES)[number];

/** What `system` currently resolves to. Never stored — only displayed. */
export type ResolvedMode = 'light' | 'dark';

export const COLOR_MODE_STORAGE_KEY = 'nigel.color-mode';
export const LIGHT_CLASS = 'light-mode';
export const DARK_CLASS = 'dark-mode';

const DARK_QUERY = '(prefers-color-scheme: dark)';

/**
 * The narrowest shape each function needs, so callers can pass a fake and the
 * app can pass the real thing.
 */
type ModeStorage = Pick<Storage, 'getItem' | 'setItem'>;
type ClassTarget = { classList: Pick<DOMTokenList, 'add' | 'remove'> };
type DarkQuery = Pick<MediaQueryList, 'matches'>;

function isColorMode(value: unknown): value is ColorMode {
  return COLOR_MODES.includes(value as ColorMode);
}

function defaultStorage(): ModeStorage | undefined {
  return typeof localStorage === 'undefined' ? undefined : localStorage;
}

/**
 * The stored preference, or `system`.
 *
 * Every access is wrapped: with storage disabled, `localStorage` throws on
 * access rather than answering null, and a colour preference is not a reason
 * for the application to fail to boot. An unrecognised value is treated as
 * `system` and left alone rather than persisted over — nothing else writes
 * this key, so a value we do not recognise is more likely a future version's
 * than garbage worth destroying.
 */
export function readMode(storage: ModeStorage | undefined = defaultStorage()): ColorMode {
  try {
    const stored = storage?.getItem(COLOR_MODE_STORAGE_KEY);
    return isColorMode(stored) ? stored : 'system';
  } catch {
    return 'system';
  }
}

/** Persist the preference. A refusal is silent: the mode still applies. */
export function writeMode(
  mode: ColorMode,
  storage: ModeStorage | undefined = defaultStorage(),
): void {
  try {
    storage?.setItem(COLOR_MODE_STORAGE_KEY, mode);
  } catch {
    // Storage disabled. The class is still applied, so the choice holds for
    // this page; it just will not survive a reload, which is the best a
    // browser refusing storage allows.
  }
}

/**
 * Put the mode on the root element.
 *
 * `classList.add`/`remove`, never `className =`: `<html>` is not ours alone.
 */
export function applyMode(
  mode: ColorMode,
  root: ClassTarget | undefined = typeof document === 'undefined'
    ? undefined
    : document.documentElement,
): void {
  if (!root) return;
  if (mode === 'light') {
    root.classList.remove(DARK_CLASS);
    root.classList.add(LIGHT_CLASS);
  } else if (mode === 'dark') {
    root.classList.remove(LIGHT_CLASS);
    root.classList.add(DARK_CLASS);
  } else {
    root.classList.remove(LIGHT_CLASS, DARK_CLASS);
  }
}

function defaultQuery(): DarkQuery | undefined {
  return typeof matchMedia === 'undefined' ? undefined : matchMedia(DARK_QUERY);
}

/**
 * What the user is actually looking at.
 *
 * Only ever used for the "System — currently dark" hint, so an environment
 * without `matchMedia` (jsdom answers nothing useful anyway) resolves to light
 * rather than throwing. The colours do not depend on this.
 */
export function resolveMode(
  mode: ColorMode,
  query: DarkQuery | undefined = defaultQuery(),
): ResolvedMode {
  if (mode !== 'system') return mode;
  return query?.matches ? 'dark' : 'light';
}

/** The `matchMedia` handle the app watches so the hint stays honest. */
export function darkModeQuery(): MediaQueryList | undefined {
  return typeof matchMedia === 'undefined' ? undefined : matchMedia(DARK_QUERY);
}

/**
 * Read the stored preference and apply it. Called once at boot.
 *
 * Idempotent, because `index.html` runs an inline copy of the same two lines
 * before first paint and this then takes over.
 */
export function initColorMode(
  options: { storage?: ModeStorage; root?: ClassTarget } = {},
): ColorMode {
  const mode = readMode(options.storage ?? defaultStorage());
  applyMode(
    mode,
    options.root ?? (typeof document === 'undefined' ? undefined : document.documentElement),
  );
  return mode;
}
