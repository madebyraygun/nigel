import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { render } from 'lit';
import { controlsCss } from '@nigel/theme';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './button-variants.preview.js';

/**
 * The preview has no component of its own — it is the theme's standard button
 * shown on the primitive that carries it — so this file exists to give its
 * states the same axe coverage every other preview's component test provides,
 * under both colour classes.
 */
describePreviewA11y(preview);

describe('in dark mode', () => {
  beforeAll(() => document.documentElement.classList.add('dark-mode'));
  afterAll(() => document.documentElement.classList.remove('dark-mode'));
  describePreviewA11y(preview);
});

describe('standard button preview', () => {
  /** The states as DOM, which is the only place the injected sheet appears. */
  const rendered = preview.states
    .map((state) => {
      const host = document.createElement('div');
      render(state.render(), host);
      return host.innerHTML;
    })
    .join();

  it('shows the real sheet rather than a copy of the rules', () => {
    expect(rendered).toContain(controlsCss.cssText.trim().slice(0, 120));
  });

  it.each([
    ['the brand primary', 'variant="brand"'],
    ['the outline', 'appearance="outlined"'],
    ['the danger outline', 'variant="danger" appearance="outlined"'],
    ['the large size', 'size="l"'],
    ['the small size', 'size="s"'],
    ['a disabled button', 'disabled'],
    ['an icon button', 'label="Refresh"'],
  ])('shows %s', (_label, marker) => {
    expect(rendered).toContain(marker);
  });

  it('shows the filled destructive confirm', () => {
    const confirm = preview.states.find((s) => s.name === 'destructive confirm')!;
    const host = document.createElement('div');
    render(confirm.render(), host);
    const danger = host.querySelector('wa-button[variant="danger"]');
    expect(danger?.getAttribute('appearance') ?? 'accent').toBe('accent');
  });
});
