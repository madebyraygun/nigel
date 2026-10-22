import { html, type TemplateResult } from 'lit';
import { ref } from 'lit/directives/ref.js';
import { unsafeHTML } from 'lit/directives/unsafe-html.js';
import { controlsCss } from '@nigel/theme';
import '@awesome.me/webawesome/dist/components/switch/switch.js';
import type { Preview } from '../../preview/types.js';

/**
 * The standard switch: `wa-switch` as `controlsCss` draws it everywhere in the
 * app. Like the standard button's preview, each state injects that sheet,
 * because the harness's own shadow root adopts none.
 */
const SHEET = `<style>${controlsCss.cssText}</style>`;

const ROW = 'display: flex; gap: 1.5rem; flex-wrap: wrap; align-items: center;';

function row(content: TemplateResult): TemplateResult {
  return html`${unsafeHTML(SHEET)}
    <div style=${ROW}>${content}</div>`;
}

/** Focus the switch once it is in the document, so the ring shows. */
const focusOnMount = ref((el) => {
  if (el) queueMicrotask(() => (el as HTMLElement).focus());
});

const preview: Preview = {
  id: 'switch',
  title: 'Standard Switch',
  group: 'Actions',
  description:
    'Every wa-switch is a 38×22 pill. Off is a light track with a mid-grey edge and a white knob with the same edge; on fills the track with the brand and puts a check in the knob, so the state is never colour alone. Keyboard focus rings the knob; disabled sits at 40%.',
  layout: 'stack',
  states: [
    { name: 'off', render: () => row(html`<wa-switch>Show archived</wa-switch>`) },
    { name: 'on', render: () => row(html`<wa-switch checked>Show archived</wa-switch>`) },
    {
      name: 'on a card',
      background: 'surface',
      render: () =>
        row(html`
          <wa-switch>Check for updates automatically</wa-switch>
          <wa-switch checked>Check for updates automatically</wa-switch>
        `),
    },
    {
      name: 'focus',
      render: () => row(html`<wa-switch ${focusOnMount}>Show archived</wa-switch>`),
    },
    {
      name: 'disabled',
      render: () =>
        row(html`
          <wa-switch disabled>Show archived</wa-switch>
          <wa-switch checked disabled>Show archived</wa-switch>
        `),
    },
  ],
};

export default preview;
