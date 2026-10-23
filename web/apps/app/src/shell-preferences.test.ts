import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import {
  REGISTER_INLINE_EDIT_KEY,
  SIDEBAR_STORAGE_KEY,
  isHostedShell,
  paletteShortcut,
  readRegisterInlineEdit,
  readSidebarCollapsed,
  statusLineOf,
  writeRegisterInlineEdit,
  writeSidebarCollapsed,
} from './shell-preferences.js';

describe('the status line', () => {
  const status = {
    dataDir: '/home/you/Documents/Nigel/acme-consulting',
    dataDirDisplay: '~/Documents/Nigel/acme-consulting',
    encrypted: true,
    version: '1.0',
  };

  it('reads the path, lock state and version off the status', () => {
    expect(statusLineOf(status)).toEqual({
      path: '~/Documents/Nigel/acme-consulting',
      fullPath: '/home/you/Documents/Nigel/acme-consulting',
      encrypted: true,
      version: '1.0',
      distribution: undefined,
    });
  });

  it('carries the distribution so a store build can drop the version', () => {
    expect(statusLineOf({ ...status, distribution: 'store' })?.distribution).toBe('store');
  });

  it('is absent without a status', () => {
    expect(statusLineOf(null)).toBeUndefined();
  });
});

function memoryStorage(): Pick<Storage, 'getItem' | 'setItem'> & { data: Map<string, string> } {
  const data = new Map<string, string>();
  return {
    data,
    getItem: (key) => data.get(key) ?? null,
    setItem: (key, value) => void data.set(key, value),
  };
}

const refusing: Pick<Storage, 'getItem' | 'setItem'> = {
  getItem: () => {
    throw new Error('SecurityError');
  },
  setItem: () => {
    throw new Error('SecurityError');
  },
};

describe('the sidebar preference', () => {
  it('starts open on a device that has never folded it', () => {
    expect(readSidebarCollapsed(memoryStorage())).toBe(false);
  });

  it('remembers a fold across a reload', () => {
    const storage = memoryStorage();
    writeSidebarCollapsed(true, storage);
    expect(storage.data.get(SIDEBAR_STORAGE_KEY)).toBe('true');
    expect(readSidebarCollapsed(storage)).toBe(true);

    writeSidebarCollapsed(false, storage);
    expect(readSidebarCollapsed(storage)).toBe(false);
  });

  it('survives storage that refuses', () => {
    expect(readSidebarCollapsed(refusing)).toBe(false);
    expect(() => writeSidebarCollapsed(true, refusing)).not.toThrow();
  });
});

describe('the register editing preference', () => {
  it('edits in a dialog on a device that has never chosen', () => {
    expect(readRegisterInlineEdit(memoryStorage())).toBe(false);
  });

  it('remembers inline editing across a reload', () => {
    const storage = memoryStorage();
    writeRegisterInlineEdit(true, storage);
    expect(storage.data.get(REGISTER_INLINE_EDIT_KEY)).toBe('true');
    expect(readRegisterInlineEdit(storage)).toBe(true);

    writeRegisterInlineEdit(false, storage);
    expect(readRegisterInlineEdit(storage)).toBe(false);
  });

  it('survives storage that refuses', () => {
    expect(readRegisterInlineEdit(refusing)).toBe(false);
    expect(() => writeRegisterInlineEdit(true, refusing)).not.toThrow();
  });
});

describe('with blocked site data, where reaching localStorage itself throws', () => {
  let original: PropertyDescriptor | undefined;

  beforeEach(() => {
    original = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
    Object.defineProperty(globalThis, 'localStorage', {
      configurable: true,
      get() {
        throw new DOMException('The operation is insecure.', 'SecurityError');
      },
    });
  });

  afterEach(() => {
    if (original) Object.defineProperty(globalThis, 'localStorage', original);
    else Reflect.deleteProperty(globalThis, 'localStorage');
  });

  it('reads the defaults and writes nothing, without throwing', () => {
    expect(readSidebarCollapsed()).toBe(false);
    expect(readRegisterInlineEdit()).toBe(false);
    expect(() => writeSidebarCollapsed(true)).not.toThrow();
    expect(() => writeRegisterInlineEdit(true)).not.toThrow();
  });
});

describe('hosted mode', () => {
  it('is on when the native shell opens the page with ?shell=native', () => {
    expect(isHostedShell('?shell=native', '#/dashboard')).toBe(true);
    expect(isHostedShell('?token=abc&shell=native', '')).toBe(true);
  });

  it('is on when the flag rides in the hash route, as when Settings is opened directly', () => {
    expect(isHostedShell('', '#/settings?shell=native')).toBe(true);
    expect(isHostedShell('', '#/invoices?number=1254&shell=native')).toBe(true);
  });

  it('is off in a browser and in Tauri', () => {
    expect(isHostedShell('', '')).toBe(false);
    expect(isHostedShell('?shell=tauri', '#/dashboard')).toBe(false);
    expect(isHostedShell('', '#/settings?shell=tauri')).toBe(false);
    expect(isHostedShell('', '#/settings')).toBe(false);
  });
});

describe('the palette shortcut', () => {
  it('is Command on Apple platforms', () => {
    expect(paletteShortcut('MacIntel')).toEqual({ keycap: '⌘K', aria: 'Meta+K' });
    expect(paletteShortcut('iPad')).toEqual({ keycap: '⌘K', aria: 'Meta+K' });
  });

  it('is Control everywhere else', () => {
    expect(paletteShortcut('Win32')).toEqual({ keycap: 'Ctrl K', aria: 'Control+K' });
    expect(paletteShortcut('Linux x86_64')).toEqual({ keycap: 'Ctrl K', aria: 'Control+K' });
  });
});
