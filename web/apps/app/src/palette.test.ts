/** @vitest-environment jsdom */
import { describe, it, expect } from 'vitest';
import { isPaletteShortcut, paletteBooks, paletteScreens } from './palette.js';
import { UNLOCKED_STATUS } from './__mocks__/fake-api-client.js';

const key = (init: KeyboardEventInit) => new KeyboardEvent('keydown', init);

describe('the palette chord', () => {
  it.each([
    [false, { key: 'k', ctrlKey: true }],
    [false, { key: 'K', ctrlKey: true }],
    [true, { key: 'k', metaKey: true }],
  ])('is, with apple %s, %o', (apple, init) => {
    expect(isPaletteShortcut(key(init), apple)).toBe(true);
  });

  it.each([
    [false, { key: 'k' }],
    [false, { key: 'k', metaKey: true }],
    [true, { key: 'k', ctrlKey: true }],
    [false, { key: 'k', ctrlKey: true, shiftKey: true }],
    [false, { key: 'k', ctrlKey: true, altKey: true }],
    [false, { key: 'k', ctrlKey: true, metaKey: true }],
    [false, { key: 'j', ctrlKey: true }],
    [false, { key: 'k', ctrlKey: true, repeat: true }],
    [false, { key: 'k', ctrlKey: true, isComposing: true }],
    [false, { key: 'k', ctrlKey: true, keyCode: 229 }],
  ])('is not, with apple %s, %o', (apple, init) => {
    expect(isPaletteShortcut(key(init), apple)).toBe(false);
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
