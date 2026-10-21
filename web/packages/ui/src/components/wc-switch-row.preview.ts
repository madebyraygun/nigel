import { html } from 'lit';
import './wc-switch-row.js';
import type { Preview } from '../../preview/types.js';

const width = 'width:min(720px, calc(100vw - 32px));';
const label = 'Check for updates automatically';
const description = 'Checks GitHub for a newer version once a day when Nigel starts.';

const preview: Preview = {
  id: 'wc-switch-row',
  title: 'Switch Row',
  group: 'Forms',
  description:
    'A setting that is on or off, as a card with a switch. Fully controlled: the switch only moves when its owner sets checked.',
  layout: 'stack',
  states: [
    {
      name: 'on',
      render: () =>
        html`<div style=${width}>
          <wc-switch-row label=${label} description=${description} checked></wc-switch-row>
        </div>`,
    },
    {
      name: 'off',
      render: () =>
        html`<div style=${width}>
          <wc-switch-row label=${label} description=${description}></wc-switch-row>
        </div>`,
    },
    {
      name: 'disabled',
      render: () =>
        html`<div style=${width}>
          <wc-switch-row label=${label} description=${description} checked disabled></wc-switch-row>
        </div>`,
    },
  ],
};

export default preview;
