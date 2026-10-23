import { html } from 'lit';
import './wc-logo-field.js';
import type { Preview } from '../../preview/types.js';

const HELP =
  'Up to 128 KB. Shown at the top of your invoices.';

/** A 1×1 lavender PNG, so the filled state needs no asset. */
const LOGO =
  'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mN89+5/PQAJbwPU2w0yAgAAAABJRU5ErkJggg==';

const width = 'width:min(720px, calc(100vw - 32px));';

const preview: Preview = {
  id: 'wc-logo-field',
  title: 'Logo Field',
  group: 'Forms',
  description:
    "The letterhead's logo: a preview on the documents' white, what the file may be, Choose… and Remove. A dropped file counts as chosen.",
  layout: 'stack',
  states: [
    {
      name: 'empty',
      render: () =>
        html`<div style=${width}>
          <wc-logo-field name="Acme Consulting LLC" help=${HELP}></wc-logo-field>
        </div>`,
    },
    {
      name: 'with logo',
      render: () =>
        html`<div style=${width}>
          <wc-logo-field name="Acme Consulting LLC" logo=${LOGO} help=${HELP}></wc-logo-field>
        </div>`,
    },
    {
      name: 'desktop (no drop)',
      render: () =>
        html`<div style=${width}>
          <wc-logo-field
            name="Juniper Labs"
            prompt="Choose a PNG or JPEG"
            help=${HELP}
          ></wc-logo-field>
        </div>`,
    },
    {
      name: 'disabled',
      render: () =>
        html`<div style=${width}>
          <wc-logo-field name="Acme Consulting LLC" help=${HELP} disabled></wc-logo-field>
        </div>`,
    },
  ],
};

export default preview;
