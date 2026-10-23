import { html } from 'lit';
import './wc-settings-index.js';
import type { Preview } from '../../preview/types.js';
import type { SettingsIndexGroup } from './wc-settings-index.js';

const groups: SettingsIndexGroup[] = [
  {
    label: 'This book',
    items: [
      { id: 'letterhead', label: 'Letterhead' },
      { id: 'security', label: 'Security' },
      { id: 'location', label: 'Location' },
    ],
  },
  {
    label: 'This device',
    items: [
      { id: 'appearance', label: 'Appearance' },
      { id: 'updates', label: 'Updates' },
    ],
  },
];

const column = (active: string, list = groups) =>
  html`<div style="width:184px;">
    <wc-settings-index .groups=${list} active=${active}></wc-settings-index>
  </div>`;

const preview: Preview = {
  id: 'wc-settings-index',
  title: 'Settings Index',
  group: 'Layout',
  description:
    "Settings' sticky left column: the page heading over its sections, grouped by what they apply to. The active item follows the scroll position.",
  layout: 'grid',
  states: [
    { name: 'first section', render: () => column('letterhead') },
    { name: 'device section', render: () => column('appearance') },
    {
      name: 'store build (no updates)',
      render: () =>
        column('appearance', [groups[0], { ...groups[1], items: groups[1].items.slice(0, 1) }]),
    },
  ],
};

export default preview;
