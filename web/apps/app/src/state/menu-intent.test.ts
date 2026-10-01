import { describe, it, expect, beforeEach } from 'vitest';
import {
  requestMenuIntent,
  consumeMenuIntent,
  dropMenuIntentUnlessFor,
  resetMenuIntent,
} from './menu-intent.js';

describe('menu intent', () => {
  beforeEach(() => {
    resetMenuIntent();
  });

  it('hands a requested intent to the matching consumer exactly once', () => {
    requestMenuIntent('find');
    expect(consumeMenuIntent('find')).toBe(true);
    expect(consumeMenuIntent('find')).toBe(false);
  });

  it('leaves an intent in place for a consumer that asks for another', () => {
    requestMenuIntent('pick-import');
    expect(consumeMenuIntent('find')).toBe(false);
    expect(consumeMenuIntent('pick-import')).toBe(true);
  });

  it('re-fires on a repeated request — the chord pressed twice is two deliveries', () => {
    requestMenuIntent('find');
    expect(consumeMenuIntent('find')).toBe(true);
    requestMenuIntent('find');
    expect(consumeMenuIntent('find')).toBe(true);
  });

  it('answers nothing when nothing was requested', () => {
    expect(consumeMenuIntent('find')).toBe(false);
    expect(consumeMenuIntent('pick-import')).toBe(false);
  });

  it('survives a route change to its own screen and dies on any other', () => {
    requestMenuIntent('pick-import');
    dropMenuIntentUnlessFor('import');
    expect(consumeMenuIntent('pick-import')).toBe(true);

    requestMenuIntent('pick-import');
    dropMenuIntentUnlessFor('reports');
    expect(consumeMenuIntent('pick-import')).toBe(false);

    requestMenuIntent('find');
    dropMenuIntentUnlessFor('dashboard');
    expect(consumeMenuIntent('find')).toBe(false);
  });
});
