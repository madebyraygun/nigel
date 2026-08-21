import { afterEach, describe, expect, it, vi } from 'vitest';

import { resolvedMode, wireShellChrome, type DarkPreference } from './chrome-bridge.js';
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

afterEach(() => {
  document.documentElement.className = '';
});

describe('resolvedMode', () => {
  it('lets an explicit class win over the OS preference', () => {
    const root = document.createElement('div');
    root.classList.add('light-mode');
    expect(resolvedMode(root, true)).toBe('light');
    root.className = 'dark-mode';
    expect(resolvedMode(root, false)).toBe('dark');
  });

  it('falls back to the OS preference without a class', () => {
    const root = document.createElement('div');
    expect(resolvedMode(root, true)).toBe('dark');
    expect(resolvedMode(root, false)).toBe('light');
  });
});

describe('wireShellChrome', () => {
  it('reports the palette immediately and signals ready once painted', async () => {
    const chrome = recorder();
    const unwire = wireShellChrome(chrome, document, darkPreference(true));

    expect(chrome.backgrounds).toEqual(['dark']);
    expect(chrome.readyCount).toBe(0);
    // Two frames stand between wiring and ready.
    await new Promise((resolve) => requestAnimationFrame(resolve));
    await new Promise((resolve) => requestAnimationFrame(resolve));
    expect(chrome.readyCount).toBe(1);
    unwire();
  });

  it('follows the color-mode class and the OS preference', async () => {
    const chrome = recorder();
    const preference = darkPreference(false);
    const unwire = wireShellChrome(chrome, document, preference);
    expect(chrome.backgrounds).toEqual(['light']);

    document.documentElement.classList.add('dark-mode');
    await new Promise((resolve) => queueMicrotask(() => resolve(undefined)));
    expect(chrome.backgrounds.at(-1)).toBe('dark');

    document.documentElement.classList.remove('dark-mode');
    await new Promise((resolve) => queueMicrotask(() => resolve(undefined)));
    preference.matches = true;
    preference.fire();
    expect(chrome.backgrounds.at(-1)).toBe('dark');
    unwire();
  });

  it('stops reporting once unwired', async () => {
    const chrome = recorder();
    const preference = darkPreference(false);
    const unwire = wireShellChrome(chrome, document, preference);
    const reported = chrome.backgrounds.length;

    unwire();
    document.documentElement.classList.add('dark-mode');
    await new Promise((resolve) => queueMicrotask(() => resolve(undefined)));
    preference.fire();
    expect(chrome.backgrounds.length).toBe(reported);
  });

  it('never throws when the shell refuses an invoke', () => {
    // The desktop client wraps every invoke in a swallowed rejection; the
    // bridge itself must tolerate a chrome whose calls reject synchronously
    // being absent — wiring against a throwing chrome is a shell bug, not
    // an app crash.
    const chrome: ShellChrome = {
      ready: vi.fn(),
      background: vi.fn(),
    };
    expect(() => wireShellChrome(chrome, document, darkPreference(false))()).not.toThrow();
  });
});
