import type { ShellChrome } from './api/client.js';

/**
 * Keeps a native window's chrome in step with the SPA.
 *
 * Lives beside the app rather than in a package: which shell is running is
 * this app's business (docs/native-feel.md's placement rule), and both ends
 * of the wire — the api client and the document the color-mode classes land
 * on — are already here.
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
export function resolvedMode(root: Element, prefersDark: boolean): 'light' | 'dark' {
  if (root.classList.contains('dark-mode')) return 'dark';
  if (root.classList.contains('light-mode')) return 'light';
  return prefersDark ? 'dark' : 'light';
}

function osDarkPreference(): DarkPreference {
  // jsdom has no matchMedia; tests inject, and the fallback never listens.
  const media = globalThis.matchMedia?.('(prefers-color-scheme: dark)');
  return (
    media ?? { matches: false, addEventListener: () => {}, removeEventListener: () => {} }
  );
}

/**
 * Report the resolved palette to the shell now and on every change, and show
 * the window once the first frame exists. Returns the unwire.
 */
export function wireShellChrome(
  chrome: ShellChrome,
  doc: Document = document,
  prefersDark: DarkPreference = osDarkPreference(),
): () => void {
  const report = () => chrome.background(resolvedMode(doc.documentElement, prefersDark.matches));
  report();

  const observer = new MutationObserver(report);
  observer.observe(doc.documentElement, { attributes: true, attributeFilter: ['class'] });
  prefersDark.addEventListener('change', report);

  // Two frames: the first can still be the blank sheet the elements mounted
  // into; by the second, the SPA has painted.
  requestAnimationFrame(() => requestAnimationFrame(() => chrome.ready()));

  return () => {
    observer.disconnect();
    prefersDark.removeEventListener('change', report);
  };
}
