import { html } from 'lit';
import './wc-settings-section.js';
import type { Preview } from '../../preview/types.js';

const body = () => html`<p style="margin:0;">The section's controls go here.</p>`;

const preview: Preview = {
  id: 'wc-settings-section',
  title: 'Settings Section',
  group: 'Layout',
  description:
    'One section of Settings under a hairline: title, what it is for, and its controls. Saved shows a confirmation beside the title.',
  layout: 'stack',
  states: [
    {
      name: 'default',
      render: () =>
        html`<wc-settings-section
          heading="Letterhead"
          description="Your name and details as they appear on invoices."
          >${body()}</wc-settings-section
        >`,
    },
    {
      name: 'saved',
      render: () =>
        html`<wc-settings-section
          heading="Letterhead"
          description="Your name and details as they appear on invoices."
          saved
          >${body()}</wc-settings-section
        >`,
    },
    {
      name: 'no description',
      render: () => html`<wc-settings-section heading="Security">${body()}</wc-settings-section>`,
    },
  ],
};

export default preview;
