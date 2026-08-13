import { html } from 'lit';
import './wc-row-badge.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-row-badge',
  title: 'Row badge',
  group: 'Managers',
  description:
    "A muted word beside a row's name, for a state that does not deserve a column of its own.",
  states: [
    {
      name: 'archived',
      render: () => html`<wc-row-badge label="Archived"></wc-row-badge>`,
    },
    {
      name: 'empty label renders nothing',
      render: () => html`<wc-row-badge label=""></wc-row-badge>`,
    },
    {
      name: 'beside a name',
      render: () => html`
        <div style="display:flex;align-items:center;gap:8px;">
          <span>Umbrella Corp</span>
          <wc-row-badge label="Archived"></wc-row-badge>
        </div>
      `,
    },
    {
      name: 'a long label does not wrap',
      render: () =>
        html`<wc-row-badge label="Archived on 2026-03-01"></wc-row-badge>`,
    },
  ],
};

export default preview;
