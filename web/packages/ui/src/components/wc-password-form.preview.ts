import { html } from 'lit';
import './wc-password-form.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-password-form',
  title: 'Password Form',
  group: 'Forms',
  description:
    'One password operation per form, named by its own legend. The confirmation field never leaves the component.',
  layout: 'stack',
  states: [
    { name: 'set', render: () => html`<wc-password-form mode="set"></wc-password-form>` },
    {
      name: 'change',
      render: () => html`<wc-password-form mode="change"></wc-password-form>`,
    },
    {
      name: 'remove',
      render: () => html`<wc-password-form mode="remove"></wc-password-form>`,
    },
    {
      // What an encrypted database shows: two operations, each collecting a
      // field called "Current password". The state this component exists to
      // keep readable.
      name: 'encrypted database',
      render: () => html`
        <div style="display: grid; gap: 1.5rem;">
          <wc-password-form mode="change"></wc-password-form>
          <wc-password-form mode="remove"></wc-password-form>
        </div>
      `,
    },
    {
      name: 'overridden wording',
      render: () =>
        html`<wc-password-form
          mode="change"
          heading="Re-key these books"
          description="The database is rewritten under the new password."
        ></wc-password-form>`,
    },
    {
      name: 'error',
      render: () =>
        html`<wc-password-form
          mode="change"
          error="Wrong password."
        ></wc-password-form>`,
    },
    {
      name: 'busy',
      render: () => html`<wc-password-form mode="set" busy></wc-password-form>`,
    },
  ],
};

export default preview;
