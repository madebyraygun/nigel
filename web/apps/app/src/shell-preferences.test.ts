import { describe, it, expect } from 'vitest';
import {
  SIDEBAR_STORAGE_KEY,
  isHostedShell,
  paletteShortcut,
  readSidebarCollapsed,
  writeSidebarCollapsed,
} from './shell-preferences.js';

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

describe('hosted mode', () => {
  it('is on when the native shell opens the page with ?shell=native', () => {
    expect(isHostedShell('?shell=native')).toBe(true);
    expect(isHostedShell('?token=abc&shell=native')).toBe(true);
  });

  it('is off in a browser and in Tauri', () => {
    expect(isHostedShell('')).toBe(false);
    expect(isHostedShell('?shell=tauri')).toBe(false);
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
