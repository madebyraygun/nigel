import { html } from 'lit';
import './wc-step-indicator.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-step-indicator',
  title: 'Step Indicator',
  group: 'Layout',
  description: 'Setup progress: completed and current pills brand, future pills border.',
  states: [
    {
      name: 'first step',
      render: () => html`<wc-step-indicator .current=${0}></wc-step-indicator>`,
    },
    {
      name: 'second step',
      render: () => html`<wc-step-indicator .current=${1}></wc-step-indicator>`,
    },
    {
      name: 'third step',
      render: () => html`<wc-step-indicator .current=${2}></wc-step-indicator>`,
    },
    {
      name: 'last step',
      render: () => html`<wc-step-indicator .current=${3}></wc-step-indicator>`,
    },
  ],
};

export default preview;
