import { describe, it, expect } from 'vitest';
import { nigelTheme } from '../src/themes/nigel.js';

const text = nigelTheme.cssText;

describe('nigelTheme', () => {
  it('exposes a Lit CSSResult with the composed token sheet', () => {
    expect(nigelTheme).toBeDefined();
    expect(typeof text).toBe('string');
    expect(text.length).toBeGreaterThan(0);
  });

  it.each([
    '--wa-color-bg',
    '--wa-color-surface',
    '--wa-color-surface-alt',
    '--wa-color-border',
    '--wa-color-border-soft',
    '--wa-color-text',
    '--wa-color-muted',
    '--wa-color-brand',
    '--wa-color-brand-hover',
    '--wa-color-on-brand',
    '--wa-color-focus',
    '--wa-color-danger',
    '--wa-color-success',
    '--wa-color-warning',
    '--wa-color-info',
    '--wa-font-family-sans',
    '--wa-font-family-mono',
    '--wa-font-size-base',
    '--wa-line-height',
    '--wa-space-m',
    '--wa-radius-md',
    '--wa-shadow-sm',
  ])('defines the Web Awesome token %s', (token) => {
    expect(text).toContain(`${token}:`);
  });

  it.each([
    '--nc-color-income',
    '--nc-color-expense',
    '--nc-color-flagged',
    '--nc-color-selected-bg',
    '--nc-grad-brand',
    '--nc-grad-brand-soft',
    '--nc-font-money',
    '--nc-icon-size',
    '--nc-sidebar-width',
    '--nc-sidebar-collapsed-width',
    '--nc-header-height',
    '--nc-transition-fast',
    '--nc-transition-base',
  ])('defines the nigel token %s', (token) => {
    expect(text).toContain(`${token}:`);
  });

  it('pins color-scheme when a mode is forced, so native widgets follow', () => {
    // :root declares `color-scheme: light dark`, which lets the UA pick
    // scrollbars, date pickers and form-control defaults from the OS. An
    // explicit choice has to pin it, or the app is light with dark scrollbars.
    expect(text).toMatch(/:root\.light-mode\s*{[^}]*color-scheme:\s*light/);
    expect(text).toMatch(/:root\.dark-mode\s*{[^}]*color-scheme:\s*dark/);
  });

  it('supports system dark mode and both explicit overrides', () => {
    expect(text).toMatch(/prefers-color-scheme:\s*dark/);
    expect(text).toContain('.dark-mode');
    expect(text).toContain('.light-mode');
  });

  it('honours a reduced-motion preference', () => {
    expect(text).toMatch(/prefers-reduced-motion:\s*reduce/);
  });

  it('orders light tokens before dark overrides before print', () => {
    // Specificity alone does not settle this: the dark block and the light
    // block both target :root, so the later one wins. Order is the contract.
    const light = text.indexOf('--wa-color-bg: #f3f2f7');
    const dark = text.indexOf('.dark-mode');
    const print = text.indexOf('@media print');
    expect(light).toBeGreaterThan(-1);
    expect(light).toBeLessThan(dark);
    expect(dark).toBeLessThan(print);
  });
});
