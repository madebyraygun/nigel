import { html } from 'lit';
import './wc-review-pill.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-review-pill',
  title: 'Review pill',
  group: 'Data',
  description:
    'The flag-coloured "N to review" link at the right of a page title. It leads to the review screen.',
  states: [
    {
      name: 'default',
      render: () => html`<wc-review-pill .count=${84}></wc-review-pill>`,
    },
    {
      name: 'single',
      render: () => html`<wc-review-pill .count=${1}></wc-review-pill>`,
    },
    {
      name: 'large count',
      render: () => html`<wc-review-pill .count=${1280}></wc-review-pill>`,
    },
  ],
};

export default preview;
