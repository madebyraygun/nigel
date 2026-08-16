import { html } from 'lit';
import './wc-shortcut-help.js';
import type { ShortcutHint } from './wc-shortcut-help.js';
import { REGISTER_SHORTCUTS } from './wc-register-table.js';
import type { Preview } from '../../preview/types.js';

const registerHints: ShortcutHint[] = [...REGISTER_SHORTCUTS];

const twoHints: ShortcutHint[] = [
  { keys: ['ArrowLeft', 'ArrowRight'], display: '← →', description: 'Previous or next period' },
  { keys: ['m'], display: 'm', description: 'Month or year' },
];

/**
 * The open states sit in a tall box on purpose: the panel is out of flow, so
 * the point of the state is that the paragraph under it does not move.
 */
const inContext = (content: unknown) => html`
  <div style="min-height: 15rem">
    ${content}
    <p style="margin: 0.5rem 0 0; max-width: 24rem">
      Text below the trigger. An anchored popover leaves this line exactly
      where it was; the inline disclosure it replaced pushed it down the page.
    </p>
  </div>
`;

const preview = {
  id: 'wc-shortcut-help',
  title: 'Shortcut help',
  group: 'Navigation',
  layout: 'stack',
  description:
    'A screen keyboard legend behind a trigger. The panel is absolutely positioned, so opening it moves nothing; Escape and an outside click both close it and return focus to the trigger.',
  states: [
    {
      name: 'closed',
      render: () =>
        inContext(html`<wc-shortcut-help .shortcuts=${registerHints}></wc-shortcut-help>`),
    },
    {
      name: 'open',
      render: () =>
        inContext(
          html`<wc-shortcut-help
            open
            heading="Register shortcuts"
            .shortcuts=${registerHints}
          ></wc-shortcut-help>`,
        ),
    },
    {
      name: 'open-short-list',
      render: () =>
        inContext(
          html`<wc-shortcut-help
            open
            label="Keys"
            heading="Report shortcuts"
            .shortcuts=${twoHints}
          ></wc-shortcut-help>`,
        ),
    },
    {
      name: 'no-shortcuts',
      render: () => inContext(html`<wc-shortcut-help open></wc-shortcut-help>`),
    },
  ],
} satisfies Preview;

export default preview;
