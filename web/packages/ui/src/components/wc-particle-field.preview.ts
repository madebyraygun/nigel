import { html } from 'lit';
import './wc-particle-field.js';
import type { Preview } from '../../preview/types.js';

const box = (content: unknown) =>
  html`<div style="position: relative; height: 240px; background: var(--nc-color-arcade-bg, #17171d);">
    ${content}
  </div>`;

const preview: Preview = {
  id: 'wc-particle-field',
  title: 'Particle Field',
  group: 'Brand',
  description:
    "The TUI's drifting specks, capped at the same twenty. Decorative, aria-hidden, still and silent under reduced motion.",
  layout: 'stack',
  states: [
    {
      name: 'default',
      render: () => box(html`<wc-particle-field style="position:absolute;inset:0"></wc-particle-field>`),
    },
    {
      name: 'sparse',
      render: () =>
        box(html`<wc-particle-field density="6" style="position:absolute;inset:0"></wc-particle-field>`),
    },
    {
      name: 'reduced-motion',
      render: () =>
        box(
          html`<wc-particle-field reduced-motion style="position:absolute;inset:0"></wc-particle-field>`,
        ),
    },
  ],
};

export default preview;
