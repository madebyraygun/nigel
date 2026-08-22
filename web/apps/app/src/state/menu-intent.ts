import { signal, type Signal } from '../mixins/signal-watcher.js';

/**
 * What a menu selection asks of the screen it lands on.
 *
 * `find` — put the caret in the register's search box.
 * `pick-import` — open the import screen's native file picker.
 */
export type MenuIntent = 'find' | 'pick-import';

/**
 * The one intent in flight, or nothing.
 *
 * A signal rather than a route parameter so a repeated chord re-fires: the
 * root container sets it on every selection, the target screen consumes it,
 * and consuming clears it — so the same intent set twice is two deliveries,
 * where a `?focus=` parameter would be one hashchange and then silence.
 */
const intent: Signal.State<MenuIntent | null> = signal<MenuIntent | null>(null);

/** Ask the screen the app is navigating to (or already on) to act. */
export function requestMenuIntent(request: MenuIntent): void {
  intent.set(request);
}

/**
 * Take the intent if it matches, clearing it. A screen calls this from its
 * update cycle; reading through a signal watcher is what re-runs the cycle
 * when a new intent lands.
 */
export function consumeMenuIntent(request: MenuIntent): boolean {
  if (intent.get() !== request) return false;
  intent.set(null);
  return true;
}

/** Test seam: start a test with nothing in flight. */
export function resetMenuIntent(): void {
  intent.set(null);
}
