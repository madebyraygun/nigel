import '@nigel/theme/css/nigel.css';
import { initColorMode } from '@nigel/theme';
import '@nigel/ui';
import './components/nigel-app.js';
import { signalReadyWhenSettled, wireShellChrome } from './chrome-bridge.js';

// index.html has already done this inline, before first paint. This is the
// same work through the module that owns the contract, so the HTML copy stays
// a pure optimisation rather than the only writer.
initColorMode();

// Native window chrome is boot business, not the app component's: the app
// renders the same everywhere, and only this entry point knows it is the
// page of a whole window that starts hidden.
const app = document.querySelector('nigel-app');
const chrome = app?.client.shellChrome() ?? null;
if (app && chrome) {
  // Ready must fire whatever else fails, or the window never shows: it is
  // scheduled first and survives a throwing background wire.
  signalReadyWhenSettled(chrome, app.updateComplete);
  try {
    wireShellChrome(chrome);
  } catch {
    // The window keeps its boot-time OS-theme color; cosmetic.
  }
}
