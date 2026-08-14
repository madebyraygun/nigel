import { describe, it, expect } from 'vitest';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './button-variants.preview.js';

/**
 * The preview has no component of its own — it is the theme's variant families
 * shown on the primitive that reads them — so this file exists to give its
 * states the same axe coverage every other preview's component test provides.
 */
describePreviewA11y(preview);

describe('button variants preview', () => {
  it('shows every appearance a button in this app is rendered with', () => {
    // accent is the confirmation dialog's destructive action, outlined its
    // Cancel, and plain the row actions in the client and line-item forms.
    const names = preview.states.map((s) => s.name);
    for (const appearance of ['accent', 'outlined', 'plain']) {
      expect(names.some((name) => name.includes(appearance)), appearance).toBe(true);
    }
  });

  it('leaves brand out, whose fill is the gradient controlsCss paints', () => {
    // A preview host does not adopt controlsCss, so a brand button would show
    // a flat purple it never has in the app.
    const rendered = preview.states.map((s) => JSON.stringify(s.render().values)).join();
    expect(rendered).not.toContain('brand');
  });
});
