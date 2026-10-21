import { html } from 'lit';
import './wc-search-pill.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-search-pill',
  title: 'Search Pill',
  group: 'Layout',
  description:
    'The top bar search control. Opens the whole palette; the keycap names the shortcut.',
  states: [
    { name: 'mac', render: () => html`<wc-search-pill></wc-search-pill>` },
    {
      name: 'windows and linux',
      render: () =>
        html`<wc-search-pill shortcut="Ctrl K" key-shortcuts="Control+K"></wc-search-pill>`,
    },
  ],
};

export default preview;
