import { html } from 'lit';
import './wc-save-bar.js';
import type { Preview } from '../../preview/types.js';

const width = 'width:min(720px, calc(100vw - 32px));';

const preview: Preview = {
  id: 'wc-save-bar',
  title: 'Save Bar',
  group: 'Forms',
  description:
    'Holds unsaved edits at the bottom of the scrolling area: what is unsaved, Discard and Save.',
  layout: 'stack',
  states: [
    {
      name: 'unsaved',
      render: () =>
        html`<div style=${width}>
          <wc-save-bar message="Unsaved changes to the letterhead"></wc-save-bar>
        </div>`,
    },
    {
      name: 'saving',
      render: () =>
        html`<div style=${width}>
          <wc-save-bar message="Unsaved changes to the letterhead" busy></wc-save-bar>
        </div>`,
    },
    {
      name: 'narrow',
      render: () =>
        html`<div style="width:320px;">
          <wc-save-bar message="Unsaved changes to the letterhead"></wc-save-bar>
        </div>`,
    },
  ],
};

export default preview;
