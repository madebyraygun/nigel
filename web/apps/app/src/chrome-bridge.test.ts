import { afterEach, describe, expect, it, vi } from 'vitest';
import { DARK_CLASS, LIGHT_CLASS } from '@nigel/theme';

import {
  resolvedMode,
  signalReadyWhenSettled,
  wireShellChrome,
  type DarkPreference,
} from './chrome-bridge.js';
import type { ShellChrome } from './api/client.js';

function recorder(): ShellChrome & { readyCount: number; backgrounds: string[] } {
  const record = {
    readyCount: 0,
    backgrounds: [] as string[],
    ready() {
      record.readyCount += 1;
    },
    background(mode: 'light' | 'dark') {
      record.backgrounds.push(mode);
    },
  };
  return record;
}

function darkPreference(matches: boolean): DarkPreference & { fire(): void } {
  const handlers = new Set<() => void>();
  return {
    matches,
    addEventListener: (_type, handler) => handlers.add(handler),
    removeEventListener: (_type, handler) => handlers.delete(handler),
    fire: () => handlers.forEach((handler) => handler()),
  };
}

const settle = () => new Promise((resolve) => queueMicrotask(() => resolve(undefined)));

afterEach(() => {
  document.documentElement.className = '';
  vi.unstubAllGlobals();
});

describe('resolvedMode', () => {
  it('lets an explicit class win over the OS preference', () => {
    const root = document.createElement('div');
    root.classList.add(LIGHT_CLASS);
    expect(resolvedMode(root, true)).toBe('light');
    root.className = DARK_CLASS;
    expect(resolvedMode(root, false)).toBe('dark');
  });

  it('falls back to the OS preference without a class', () => {
    const root = document.createElement('div');
    expect(resolvedMode(root, true)).toBe('dark');
    expect(resolvedMode(root, false)).toBe('light');
  });
});

describe('wireShellChrome', () => {
  it('reports the palette immediately', () => {
    const chrome = recorder();
    const unwire = wireShellChrome(chrome, document, darkPreference(true));
    expect(chrome.backgrounds).toEqual(['dark']);
    unwire();
  });

  it('follows the color-mode class and the OS preference', async () => {
    const chrome = recorder();
    const preference = darkPreference(false);
    const unwire = wireShellChrome(chrome, document, preference);
    expect(chrome.backgrounds).toEqual(['light']);

    document.documentElement.classList.add(DARK_CLASS);
    await settle();
    expect(chrome.backgrounds.at(-1)).toBe('dark');

    document.documentElement.classList.remove(DARK_CLASS);
    await settle();
    preference.matches = true;
    preference.fire();
    expect(chrome.backgrounds.at(-1)).toBe('dark');
    unwire();
  });

  it('skips class churn that lands on the same palette', async () => {
    const chrome = recorder();
    const unwire = wireShellChrome(chrome, document, darkPreference(false));
    expect(chrome.backgrounds).toEqual(['light']);

    // An unrelated class toggling notifies the observer but changes no
    // palette; the shell hears nothing.
    document.documentElement.classList.add('some-app-state');
    await settle();
    document.documentElement.classList.remove('some-app-state');
    await settle();
    expect(chrome.backgrounds).toEqual(['light']);
    unwire();
  });

  it('stops reporting once unwired', async () => {
    const chrome = recorder();
    const preference = darkPreference(false);
    const unwire = wireShellChrome(chrome, document, preference);
    const reported = chrome.backgrounds.length;

    unwire();
    document.documentElement.classList.add(DARK_CLASS);
    await settle();
    preference.fire();
    expect(chrome.backgrounds.length).toBe(reported);
  });
});

describe('signalReadyWhenSettled', () => {
  it('signals exactly once, after the app settles', async () => {
    const chrome = recorder();
    let resolve!: () => void;
    signalReadyWhenSettled(
      chrome,
      new Promise<void>((r) => {
        resolve = r;
      }),
    );
    await settle();
    expect(chrome.readyCount).toBe(0);
    resolve();
    await settle();
    expect(chrome.readyCount).toBe(1);
  });

  it('still signals when the boot promise rejects: an error state must be visible', async () => {
    const chrome = recorder();
    signalReadyWhenSettled(chrome, Promise.reject(new Error('boot failed')));
    await settle();
    expect(chrome.readyCount).toBe(1);
  });

  it('needs no rendering opportunities: a hidden webview never gets one', async () => {
    // The HTML spec gives hidden documents no rendering opportunities, so
    // in the real launch sequence requestAnimationFrame never fires before
    // the show this signal triggers. jsdom's timer-driven rAF would mask a
    // regression; making it throw refuses one instead.
    vi.stubGlobal('requestAnimationFrame', () => {
      throw new Error('rendering opportunity requested before the window was shown');
    });
    const chrome = recorder();
    signalReadyWhenSettled(chrome, Promise.resolve());
    await settle();
    expect(chrome.readyCount).toBe(1);
  });
});
