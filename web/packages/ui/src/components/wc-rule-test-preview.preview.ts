import { html } from 'lit';
import './wc-rule-test-preview.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-rule-test-preview',
  title: 'Rule Test Preview',
  group: 'Data',
  description: 'What a rule pattern would match today — the `nigel rules test` dry run.',
  layout: 'stack',
  states: [
    {
      name: 'idle',
      render: () => html`<wc-rule-test-preview></wc-rule-test-preview>`,
    },
    {
      name: 'busy',
      render: () => html`<wc-rule-test-preview busy></wc-rule-test-preview>`,
    },
    {
      name: 'matches',
      render: () =>
        html`<wc-rule-test-preview
          .result=${{
            total: 5,
            matches: [
              { description: 'ADOBE CREATIVE CLOUD', count: 3 },
              { description: 'ADOBE *STOCK', count: 2 },
            ],
          }}
        ></wc-rule-test-preview>`,
    },
    {
      name: 'single-match',
      render: () =>
        html`<wc-rule-test-preview
          .result=${{
            total: 1,
            matches: [{ description: 'RENT MARCH 2025', count: 1 }],
          }}
        ></wc-rule-test-preview>`,
    },
    {
      name: 'no-matches',
      render: () =>
        html`<wc-rule-test-preview
          .result=${{ total: 0, matches: [] }}
        ></wc-rule-test-preview>`,
    },
    {
      // The panel paints a wash rather than a plane, so it still stands off
      // a container that is already on the subtle fill.
      name: 'on-subtle-fill',
      render: () =>
        html`<div style="background:var(--wa-color-surface-alt);padding:var(--wa-space-m);">
          <wc-rule-test-preview
            .result=${{ total: 2, matches: [{ description: 'GLOBEX HOSTING', count: 2 }] }}
          ></wc-rule-test-preview>
        </div>`,
    },
    {
      name: 'error',
      render: () =>
        html`<wc-rule-test-preview
          error="Invalid regex: unclosed group"
        ></wc-rule-test-preview>`,
    },
  ],
};

export default preview;
