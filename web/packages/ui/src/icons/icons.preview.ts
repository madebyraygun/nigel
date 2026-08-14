import { html } from 'lit';
import './icons.js';
import { ICON_TAGS } from './icons.js';
import type { Preview } from '../../preview/types.js';

const grid = (style = '') => html`
  <div
    style="display:grid;grid-template-columns:repeat(auto-fill,minmax(84px,1fr));gap:12px;${style}"
  >
    ${ICON_TAGS.map(
      (tag) => html`
        <div style="display:grid;justify-items:center;gap:6px;text-align:center;">
          ${document.createElement(tag)}
          <span style="font-size:10px;color:var(--wa-color-muted);"
            >${tag.replace('wc-icon-', '')}</span
          >
        </div>
      `,
    )}
  </div>
`;

/**
 * Every mark that stands in for a character IBM Plex Mono has no glyph for.
 * `wc-icon-dot` belongs here rather than in the status family: it is the
 * neutral mark, drawn for a status outside the six and for a send step not
 * started, and both of its uses are inline.
 */
const INLINE_TAGS = ICON_TAGS.filter(
  (tag) => tag.startsWith('wc-icon-status-') || tag === 'wc-icon-dot',
);

/** One icon sized to the text around it rather than to the token. */
function inlineIcon(tag: string) {
  const el = document.createElement(tag);
  el.setAttribute('inline', '');
  return el;
}

const preview: Preview = {
  id: 'icons',
  title: 'Icons',
  group: 'Foundations',
  description:
    'WcIconBase subclasses. Sized by --nc-icon-size, or by the surrounding type with the inline attribute; colored by currentColor. Decorative unless given a label.',
  layout: 'stack',
  states: [
    { name: 'default', render: () => grid() },
    { name: 'large', render: () => grid('--nc-icon-size:28px;') },
    { name: 'small', render: () => grid('--nc-icon-size:14px;') },
    {
      name: 'colored',
      render: () => grid('color:var(--wa-color-brand);'),
    },
    {
      name: 'labelled',
      render: () =>
        html`<wc-icon-flag label="Flagged transaction"></wc-icon-flag>`,
    },
    {
      // These marks stand in for characters IBM Plex Mono has no glyph for, so
      // what matters is how they read beside the mono text they accompany —
      // which is what `inline` is for, and the only way they are ever drawn.
      name: 'inline-with-text',
      render: () => html`
        <p style="max-width:60ch;">
          ${INLINE_TAGS.map(
            (tag) => html`
              <span style="display:inline-flex;align-items:center;gap:4px;margin-inline-end:12px;">
                ${inlineIcon(tag)}${tag.replace('wc-icon-status-', '').replace('wc-icon-', '')}
              </span>
            `,
          )}
        </p>
      `,
    },
    {
      // The same marks at two text sizes, which is the claim `inline` makes:
      // the mark tracks the type, and `--nc-icon-size` does not enter into it.
      name: 'inline-follows-the-type-size',
      render: () => html`
        <p style="font-size:11px;">${INLINE_TAGS.map((tag) => inlineIcon(tag))} small</p>
        <p style="font-size:22px;">${INLINE_TAGS.map((tag) => inlineIcon(tag))} large</p>
      `,
    },
  ],
};

export default preview;
