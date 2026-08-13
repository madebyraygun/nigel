import { describe, it, expect, vi } from 'vitest';
import {
  COLOR_MODES,
  COLOR_MODE_STORAGE_KEY,
  DARK_CLASS,
  LIGHT_CLASS,
  applyMode,
  initColorMode,
  readMode,
  resolveMode,
  writeMode,
} from '../src/color-mode.js';

/**
 * The theme package runs its tests in a node environment, so nothing here
 * touches a real DOM or a real Storage. Both are narrow structural types on
 * purpose — the module needs `getItem`/`setItem` and a `classList`, and taking
 * only that is what lets these tests pass plain objects.
 */
function fakeRoot(initial: string[] = []) {
  const classes = new Set(initial);
  return {
    classList: {
      add: (...names: string[]) => names.forEach((n) => classes.add(n)),
      remove: (...names: string[]) => names.forEach((n) => classes.delete(n)),
      contains: (name: string) => classes.has(name),
    },
    get classes() {
      return [...classes];
    },
  };
}

function fakeStorage(initial: Record<string, string> = {}) {
  const map = new Map(Object.entries(initial));
  return {
    getItem: (k: string) => map.get(k) ?? null,
    setItem: (k: string, v: string) => void map.set(k, v),
    removeItem: (k: string) => void map.delete(k),
    get entries() {
      return Object.fromEntries(map);
    },
  };
}

const denied = {
  getItem() {
    throw new DOMException('The operation is insecure.');
  },
  setItem() {
    throw new DOMException('The operation is insecure.');
  },
  removeItem() {
    throw new DOMException('The operation is insecure.');
  },
};

describe('the contract', () => {
  it('offers exactly light, dark and system', () => {
    expect([...COLOR_MODES]).toEqual(['light', 'dark', 'system']);
  });

  it('names the storage key and both classes once, for the html bootstrap to match', () => {
    expect(COLOR_MODE_STORAGE_KEY).toBe('nigel.color-mode');
    expect(LIGHT_CLASS).toBe('light-mode');
    expect(DARK_CLASS).toBe('dark-mode');
  });
});

describe('readMode', () => {
  it('defaults to system when nothing is stored', () => {
    expect(readMode(fakeStorage())).toBe('system');
  });

  it('defaults to system for an unrecognised value', () => {
    expect(readMode(fakeStorage({ [COLOR_MODE_STORAGE_KEY]: 'sepia' }))).toBe('system');
  });

  it.each(['light', 'dark', 'system'] as const)('returns a stored %s', (mode) => {
    expect(readMode(fakeStorage({ [COLOR_MODE_STORAGE_KEY]: mode }))).toBe(mode);
  });

  it('returns system when storage throws', () => {
    // Safari in private mode throws on getItem rather than returning null, and
    // a theme helper is not a reason for the whole app to fail to boot.
    expect(readMode(denied)).toBe('system');
  });
});

describe('writeMode', () => {
  it('stores the mode', () => {
    const storage = fakeStorage();
    writeMode('dark', storage);
    expect(storage.entries).toEqual({ [COLOR_MODE_STORAGE_KEY]: 'dark' });
  });

  it('does not throw when storage refuses', () => {
    expect(() => writeMode('dark', denied)).not.toThrow();
  });
});

describe('applyMode', () => {
  it('adds light-mode and removes dark-mode for light', () => {
    const root = fakeRoot([DARK_CLASS]);
    applyMode('light', root);
    expect(root.classes).toEqual([LIGHT_CLASS]);
  });

  it('adds dark-mode and removes light-mode for dark', () => {
    const root = fakeRoot([LIGHT_CLASS]);
    applyMode('dark', root);
    expect(root.classes).toEqual([DARK_CLASS]);
  });

  it('removes both for system — the media query does the tracking, not us', () => {
    const root = fakeRoot([DARK_CLASS]);
    applyMode('system', root);
    expect(root.classes).toEqual([]);
  });

  it('leaves unrelated classes alone', () => {
    // classList.add/remove, never className =, because <html> is not ours.
    const root = fakeRoot(['js-enabled', DARK_CLASS]);
    applyMode('light', root);
    expect(root.classes).toEqual(['js-enabled', LIGHT_CLASS]);
  });

  it.each(COLOR_MODES)('is idempotent for %s', (mode) => {
    const root = fakeRoot();
    applyMode(mode, root);
    const once = root.classes;
    applyMode(mode, root);
    expect(root.classes).toEqual(once);
  });
});

describe('resolveMode', () => {
  it.each(['light', 'dark'] as const)('answers %s for itself, whatever the query says', (mode) => {
    expect(resolveMode(mode, { matches: true })).toBe(mode);
    expect(resolveMode(mode, { matches: false })).toBe(mode);
  });

  it('answers dark for system when the query matches', () => {
    expect(resolveMode('system', { matches: true })).toBe('dark');
  });

  it('answers light for system when the query does not', () => {
    expect(resolveMode('system', { matches: false })).toBe('light');
  });

  it('answers light for system with no query available', () => {
    // jsdom has no real matchMedia, and a hint is not worth a crash.
    expect(resolveMode('system')).toBe('light');
  });
});

describe('initColorMode', () => {
  it('applies the stored mode and answers it', () => {
    const root = fakeRoot();
    const storage = fakeStorage({ [COLOR_MODE_STORAGE_KEY]: 'dark' });
    expect(initColorMode({ storage, root })).toBe('dark');
    expect(root.classes).toEqual([DARK_CLASS]);
  });

  it('applies nothing for system, leaving the media query in charge', () => {
    const root = fakeRoot([LIGHT_CLASS]);
    expect(initColorMode({ storage: fakeStorage(), root })).toBe('system');
    expect(root.classes).toEqual([]);
  });

  it('is safe to call twice — the html bootstrap has already run', () => {
    const root = fakeRoot();
    const storage = fakeStorage({ [COLOR_MODE_STORAGE_KEY]: 'light' });
    initColorMode({ storage, root });
    initColorMode({ storage, root });
    expect(root.classes).toEqual([LIGHT_CLASS]);
  });

  it('degrades to system when storage is denied', () => {
    const root = fakeRoot();
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(initColorMode({ storage: denied, root })).toBe('system');
    expect(root.classes).toEqual([]);
    expect(spy).not.toHaveBeenCalled();
    spy.mockRestore();
  });
});
