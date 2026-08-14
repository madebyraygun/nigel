import { describe, it, expect } from 'vitest';
import { render } from 'lit';
import { controlsCss } from '@nigel/theme';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './button-hover.preview.js';

/**
 * The preview has no component of its own — it is the theme's hover treatment
 * shown on the primitive that carries it — so this file exists to give its
 * states the same axe coverage every other preview's component test provides.
 */
describePreviewA11y(preview);

describe('button hover preview', () => {
  /** The states as DOM, which is the only place the injected sheet appears. */
  const rendered = preview.states
    .map((state) => {
      const host = document.createElement('div');
      render(state.render(), host);
      return host.innerHTML;
    })
    .join();

  it('shows the real sheet rather than a copy of the rules', () => {
    // A copy is what drifts: the whole point of this preview is that hovering
    // it exercises what a screen ships.
    expect(rendered).toContain('--nc-hover-border');
    expect(rendered).toContain(controlsCss.cssText.trim().slice(0, 120));
  });

  it('includes the brand button, which is the one that drifts', () => {
    expect(rendered).toContain('brand');
  });

  it('shows a button excluded from the treatment beside the ones that carry it', () => {
    // Disabled is the readable one of the three exclusions in a static
    // preview: plain has no edge to lose and loading needs a click.
    expect(rendered).toContain('disabled');
  });
});
