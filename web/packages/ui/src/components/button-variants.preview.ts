import { html, type TemplateResult } from 'lit';
import '@awesome.me/webawesome/dist/components/button/button.js';
import type { Preview } from '../../preview/types.js';

/**
 * The `wa-button` variants as the theme colours them.
 *
 * This one is deliberately not a `wc-*` component: it exists to show the
 * variant colour families in `wa-contract.ts`, which reach a button as
 * inherited custom properties and so render correctly wherever the button is.
 *
 * What it cannot show is anything from `controlsCss` — the brand gradient and
 * the hover/focus glow — because a preview state renders in the harness's own
 * shadow root, which does not adopt that sheet. `brand` is therefore left out
 * rather than shown as a flat purple it never is in the app; the confirm
 * dialog's danger state is where the glow and the fill are seen together.
 */
const VARIANTS = ['neutral', 'danger', 'success', 'warning'] as const;

function row(appearance: string): TemplateResult {
  return html`
    <div style="display: flex; gap: 0.75rem; flex-wrap: wrap; align-items: center;">
      ${VARIANTS.map(
        (variant) => html`
          <wa-button variant=${variant} appearance=${appearance}>
            ${variant[0].toUpperCase() + variant.slice(1)}
          </wa-button>
        `,
      )}
    </div>
  `;
}

const preview: Preview = {
  id: 'button-variants',
  title: 'Button Variants',
  group: 'Actions',
  description:
    'wa-button coloured by the variant families. Accent is what a confirmation dialog’s destructive action uses; outlined and plain read the same families at their quieter weights. The brand gradient and the hover glow come from controlsCss and are not visible here.',
  layout: 'stack',
  states: [
    { name: 'accent', render: () => row('accent') },
    { name: 'outlined', render: () => row('outlined') },
    { name: 'filled', render: () => row('filled') },
    { name: 'plain', render: () => row('plain') },
    {
      name: 'accent on a card',
      background: 'surface',
      render: () => row('accent'),
    },
  ],
};

export default preview;
