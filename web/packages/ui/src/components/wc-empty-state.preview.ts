import { html } from 'lit';
import './wc-empty-state.js';
import type { Preview } from '../../preview/types.js';

const preview = {
  id: 'wc-empty-state',
  title: 'Empty State',
  group: 'Feedback',
  description:
    'Nothing-here panel for empty result sets and unbuilt screens. It centres itself in the box it is given: the whole content area when it is all a screen has to show, the panel or table cell it sits in otherwise.',
  states: [
    {
      name: 'default',
      render: () =>
        html`<wc-empty-state
          heading="No transactions"
          message="Import a statement to get started."
        ></wc-empty-state>`,
    },
    {
      name: 'with-icon',
      render: () =>
        html`<wc-empty-state
          icon="wc-icon-register"
          heading="No transactions"
          message="Nothing matches the current filters."
        ></wc-empty-state>`,
    },
    {
      name: 'with-action',
      render: () =>
        html`<wc-empty-state
          icon="wc-icon-import"
          heading="No imports yet"
          message="Bring in a bank CSV or XLSX to populate the register."
        >
          <button slot="actions" type="button">Import a statement</button>
        </wc-empty-state>`,
    },
    {
      name: 'compact',
      render: () =>
        html`<wc-empty-state compact message="No results."></wc-empty-state>`,
    },
    {
      // A screen is a flex column filling the content area, and this is what an
      // empty state does with one: takes the height nothing else claimed and
      // centres itself in it. The dashed box stands in for the content area.
      name: 'filling-a-screen',
      render: () =>
        html`<div
          style="display:flex; flex-direction:column; block-size:20rem; border:1px dashed var(--wa-color-border);"
        >
          <wc-empty-state
            icon="wc-icon-review"
            heading="Nothing to review"
            message="Every transaction has a category. Importing a statement is what puts new ones here."
          >
            <button slot="actions" type="button">Open the register</button>
          </wc-empty-state>
        </div>`,
    },
  ],
} satisfies Preview;

export default preview;
