import { html, type TemplateResult } from 'lit';
import { unsafeHTML } from 'lit/directives/unsafe-html.js';
import { controlsCss } from '@nigel/theme';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '../icons/icons.js';
import type { Preview } from '../../preview/types.js';

/**
 * The standard button: `wa-button` as `controlsCss` draws it everywhere in the
 * app.
 *
 * A preview state renders in the harness's own shadow root, which adopts no
 * `controlsCss`, so each state injects that sheet itself. What is on screen is
 * the shipped rules rather than a copy that can drift, and hovering or tabbing
 * here exercises the same hover fills and focus ring a screen has.
 */
const SHEET = `<style>${controlsCss.cssText}</style>`;

const ROW = 'display: flex; gap: 0.75rem; flex-wrap: wrap; align-items: center;';

function row(content: TemplateResult): TemplateResult {
  return html`${unsafeHTML(SHEET)}
    <div style=${ROW}>${content}</div>`;
}

const standard = (): TemplateResult =>
  row(html`
    <wa-button variant="brand">
      <wc-icon-plus slot="start"></wc-icon-plus>
      New invoice
    </wa-button>
    <wa-button appearance="outlined">Sync now</wa-button>
    <wa-button>Duplicate</wa-button>
    <wa-button variant="danger" appearance="outlined">Void…</wa-button>
    <wa-button appearance="plain">Dismiss</wa-button>
    <wa-button appearance="plain"><wc-icon-refresh label="Refresh"></wc-icon-refresh></wa-button>
  `);

const preview: Preview = {
  id: 'button-variants',
  title: 'Standard Button',
  group: 'Actions',
  description:
    'Every wa-button is a pill. Brand is the flat primary with on-brand text; neutral, with or without appearance=outlined, is an outline in the border colour; danger outlined is the destructive trigger and danger filled the destructive confirm. 32px by default, 38px at size=large, 28px at size=small. A labelled icon alone in the button makes it a round icon button. Disabled sits at 40% with no hover change; keyboard focus draws a 2px brand outline offset 2px.',
  layout: 'stack',
  states: [
    { name: 'default', render: standard },
    { name: 'on a card', background: 'surface', render: standard },
    {
      name: 'large',
      render: () =>
        row(html`
          <wa-button size="large" appearance="outlined">Show me the demo</wa-button>
          <wa-button size="large" variant="brand">Set up my books</wa-button>
        `),
    },
    {
      name: 'small',
      render: () =>
        row(html`
          <wa-button size="small" appearance="outlined">Add row</wa-button>
          <wa-button size="small" variant="danger" appearance="outlined">Undo</wa-button>
          <wa-button size="small" appearance="plain">
            <wc-icon-close label="Dismiss"></wc-icon-close>
          </wa-button>
        `),
    },
    {
      name: 'destructive confirm',
      render: () =>
        row(html`
          <wa-button appearance="outlined">Cancel</wa-button>
          <wa-button variant="danger">Delete</wa-button>
        `),
    },
    {
      name: 'disabled',
      render: () =>
        row(html`
          <wa-button variant="brand" disabled>Send…</wa-button>
          <wa-button appearance="outlined" disabled>Edit</wa-button>
          <wa-button variant="danger" appearance="outlined" disabled>Void…</wa-button>
          <wa-button variant="danger" disabled>Delete</wa-button>
        `),
    },
    {
      name: 'loading',
      render: () => row(html`<wa-button variant="brand" loading>Reconcile</wa-button>`),
    },
  ],
};

export default preview;
