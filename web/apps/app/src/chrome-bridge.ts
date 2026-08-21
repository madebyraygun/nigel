import {
  DARK_CLASS,
  LIGHT_CLASS,
  darkModeQuery,
  resolveMode,
  type ResolvedMode,
} from '@nigel/theme';

import type { ShellChrome } from './api/client.js';

/**
 * Keeps a native window's chrome in step with the SPA.
 *
 * Lives beside the app rather than in a package: which shell is running is
 * this app's business (docs/native-feel.md's placement rule), and both ends
 * of the wire — the api client and the document the color-mode classes land
 * on — are already here. The classes and the system-preference fallback come
 * from `@nigel/theme`, which owns that contract.
 */

/** The subset of `MediaQueryList` this module reads, injectable for tests. */
export interface DarkPreference {
  matches: boolean;
  addEventListener(type: 'change', handler: () => void): void;
  removeEventListener(type: 'change', handler: () => void): void;
}

/**
 * The palette the SPA resolved, by the same rules as the pre-paint script in
 * index.html: an explicit class wins, otherwise the OS preference decides.
 */
export function resolvedMode(root: Element, prefersDark: boolean): ResolvedMode {
  if (root.classList.contains(DARK_CLASS)) return 'dark';
  if (root.classList.contains(LIGHT_CLASS)) return 'light';
  return resolveMode('system', { matches: prefersDark });
}

function osDarkPreference(): DarkPreference {
  // jsdom has no matchMedia; tests inject, and the fallback never listens.
  return (
    darkModeQuery() ?? {
      matches: false,
      addEventListener: () => {},
      removeEventListener: () => {},
    }
  );
}

/**
 * Report the resolved palette to the shell now and on every change it can
 * see. Returns the unwire.
 */
export function wireShellChrome(
  chrome: ShellChrome,
  doc: Document = document,
  prefersDark: DarkPreference = osDarkPreference(),
): () => void {
  let reported: ResolvedMode | undefined;
  const report = () => {
    const mode = resolvedMode(doc.documentElement, prefersDark.matches);
    // Class churn that lands on the same palette is not a change the
    // window's own color needs to hear about.
    if (mode === reported) return;
    reported = mode;
    chrome.background(mode);
  };
  report();

  const observer = new MutationObserver(report);
  observer.observe(doc.documentElement, { attributes: true, attributeFilter: ['class'] });
  prefersDark.addEventListener('change', report);

  return () => {
    observer.disconnect();
    prefersDark.removeEventListener('change', report);
  };
}

/**
 * Ask the shell to show the window once `settled` resolves — or rejects: a
 * boot that failed still has an error state the user must be able to see.
 *
 * Never a rendering-opportunity signal: a hidden webview gets no rendering
 * opportunities at all (the HTML spec gives hidden documents none), so a
 * `requestAnimationFrame` handshake would wait forever for the very show it
 * is supposed to trigger. Settled DOM paints as the window shows, and the
 * window's own theme-canvas background covers the instant before it.
 */
export function signalReadyWhenSettled(chrome: ShellChrome, settled: Promise<unknown>): void {
  const ready = () => chrome.ready();
  void settled.then(ready, ready);
}
