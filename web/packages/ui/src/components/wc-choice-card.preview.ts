import { html } from 'lit';
import './wc-choice-card.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-choice-card',
  title: 'Choice Card',
  group: 'Layout',
  description:
    'A card for one choice in Setup. Interactive, it is a single button that picks; plain, it holds a small form.',
  layout: 'grid-wide',
  states: [
    {
      name: 'interactive',
      render: () => html`
        <wc-choice-card
          interactive
          heading="A business"
          description="Schedule C or 1120-S chart of accounts, with tax lines already mapped."
        ></wc-choice-card>
      `,
    },
    {
      name: 'hover',
      render: () => html`
        <wc-choice-card
          interactive
          preview-state="hover"
          heading="A business"
          description="Schedule C or 1120-S chart of accounts, with tax lines already mapped."
        ></wc-choice-card>
      `,
    },
    {
      name: 'focus',
      render: () => html`
        <wc-choice-card
          interactive
          preview-state="focus"
          heading="A business"
          description="Schedule C or 1120-S chart of accounts, with tax lines already mapped."
        ></wc-choice-card>
      `,
    },
    {
      name: 'short description',
      render: () => html`
        <wc-choice-card
          interactive
          heading="Personal finances"
          description="A household chart. No tax mapping or invoices to chase."
        ></wc-choice-card>
      `,
    },
    {
      name: 'container',
      render: () => html`
        <wc-choice-card
          heading="Start from scratch"
          description="An empty ledger and a chart of accounts."
        >
          <p style="margin: 0">Anything goes in the slot.</p>
        </wc-choice-card>
      `,
    },
  ],
};

export default preview;
