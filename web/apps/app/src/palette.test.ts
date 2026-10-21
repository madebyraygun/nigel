/** @vitest-environment jsdom */
import { describe, it, expect } from 'vitest';
import { isPaletteShortcut, paletteBooks, paletteScreens } from './palette.js';
import { UNLOCKED_STATUS } from './__mocks__/fake-api-client.js';

const key = (init: KeyboardEventInit) => new KeyboardEvent('keydown', init);

describe('the palette chord', () => {
  it.each([
    [{ key: 'k', metaKey: true }],
    [{ key: 'k', ctrlKey: true }],
    [{ key: 'K', metaKey: true }],
  ])('is %o', (init) => {
    expect(isPaletteShortcut(key(init))).toBe(true);
  });

  it.each([
    [{ key: 'k' }],
    [{ key: 'k', metaKey: true, shiftKey: true }],
    [{ key: 'k', ctrlKey: true, altKey: true }],
    [{ key: 'j', metaKey: true }],
  ])('is not %o', (init) => {
    expect(isPaletteShortcut(key(init))).toBe(false);
  });
});

describe('what the palette lists', () => {
  it('lists every sidebar screen by its full title', () => {
    const screens = paletteScreens();
    expect(screens.map((s) => s.id)).not.toContain('unlock');
    expect(screens.map((s) => s.id)).not.toContain('setup');
    expect(screens.find((s) => s.id === 'undo')?.label).toBe('Undo last import');
  });

  it('names the profile as the kind', () => {
    expect(paletteBooks({ ...UNLOCKED_STATUS, profile: 'personal' }, 'Household')[0].kind).toBe(
      'Personal',
    );
  });

  it('has no book before status arrives', () => {
    expect(paletteBooks(null, 'Nigel')).toEqual([]);
  });
});
