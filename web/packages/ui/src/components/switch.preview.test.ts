import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { render } from 'lit';
import { controlsCss } from '@nigel/theme';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './switch.preview.js';

/**
 * The preview has no component of its own — it is the theme's standard switch
 * shown on the primitive that carries it — so this file gives its states the
 * axe coverage every other preview's component test provides, under both
 * colour classes.
 */
describePreviewA11y(preview);

describe('in dark mode', () => {
  beforeAll(() => document.documentElement.classList.add('dark-mode'));
  afterAll(() => document.documentElement.classList.remove('dark-mode'));
  describePreviewA11y(preview);
});

describe('standard switch preview', () => {
  it('shows off, on, focus and disabled', () => {
    const names = preview.states.map((s) => s.name);
    for (const state of ['off', 'on', 'focus', 'disabled']) {
      expect(names).toContain(state);
    }
  });

  it('shows the real sheet rather than a copy of the rules', () => {
    const host = document.createElement('div');
    render(preview.states[0].render(), host);
    expect(host.innerHTML).toContain(controlsCss.cssText.trim().slice(0, 120));
  });
});
