import { html } from 'lit';
import './wc-appearance-picker.js';
import type { Preview } from '../../preview/types.js';

const width = 'width:min(720px, calc(100vw - 32px));';

const preview: Preview = {
  id: 'wc-appearance-picker',
  title: 'Appearance Picker',
  group: 'Forms',
  description:
    'System, Light or Dark as miniature windows drawn in each mode. Controlled: it emits nc-color-mode-change and never touches the page, so picking here does not change the harness.',
  layout: 'stack',
  states: [
    {
      name: 'system',
      render: () => html`<div style=${width}><wc-appearance-picker mode="system"></wc-appearance-picker></div>`,
    },
    {
      name: 'light',
      render: () => html`<div style=${width}><wc-appearance-picker mode="light"></wc-appearance-picker></div>`,
    },
    {
      name: 'dark',
      render: () => html`<div style=${width}><wc-appearance-picker mode="dark"></wc-appearance-picker></div>`,
    },
  ],
};

export default preview;
