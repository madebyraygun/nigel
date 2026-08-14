import { html, type TemplateResult } from 'lit';
import { unsafeHTML } from 'lit/directives/unsafe-html.js';
import { controlsCss } from '@nigel/theme';
import '@awesome.me/webawesome/dist/components/button/button.js';
import type { Preview } from '../../preview/types.js';

/**
 * The hover and focus treatment: the brand button's gradient drifts, and every
 * button draws a 1px edge in its own colour that fades in and out.
 *
 * `button-variants.preview.ts` is the resting colours and says it cannot show
 * any of this, for a real reason — a preview state renders in the harness's own
 * shadow root, which adopts no `controlsCss`. This one injects that sheet into
 * the state itself, so what is on screen is the shipped rules rather than a
 * copy of them that can drift. It is the only place the hover treatment can be
 * looked at without running the app.
 */
const SHEET = `<style>${controlsCss.cssText}</style>`;

function row(appearance?: string): TemplateResult {
  const a = appearance ?? 'accent';
  return html`
    ${unsafeHTML(SHEET)}
    <div style="display: flex; gap: 0.75rem; flex-wrap: wrap; align-items: center;">
      <wa-button variant="brand" appearance=${a}>Send invoice</wa-button>
      <wa-button appearance=${a}>Preview</wa-button>
      <wa-button variant="danger" appearance=${a}>Void</wa-button>
      <wa-button variant="brand" appearance=${a} disabled>Disabled</wa-button>
    </div>
  `;
}

const preview: Preview = {
  id: 'button-hover',
  title: 'Button Hover',
  group: 'Actions',
  description:
    'Hover or tab to a button. The brand button scrolls its own ramp — the same seven colours, moved rather than recoloured — and every button fades in a 1px edge in its variant colour. Disabled, loading and plain buttons are excluded from both, and the drift stops under prefers-reduced-motion while the edge still draws.',
  layout: 'stack',
  states: [
    { name: 'accent', render: () => row() },
    { name: 'outlined', render: () => row('outlined') },
    { name: 'accent on a card', background: 'surface', render: () => row() },
  ],
};

export default preview;
