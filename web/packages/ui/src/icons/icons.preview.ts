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

const STATUS_TAGS = ICON_TAGS.filter((tag) => tag.startsWith('wc-icon-status-'));

/** One icon at the size of the text around it. */
function withSize(tag: string) {
  const el = document.createElement(tag);
  el.style.setProperty('--nc-icon-size', '1em');
  return el;
}

const preview: Preview = {
  id: 'icons',
  title: 'Icons',
  group: 'Foundations',
  description:
    'WcIconBase subclasses. Sized by --nc-icon-size, colored by currentColor. Decorative unless given a label.',
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
      // The status marks and the neutral dot stand in for characters IBM Plex
      // Mono has no glyph for, so what matters is how they read at 1em beside
      // the mono text they accompany.
      name: 'inline-with-text',
      render: () => html`
        <p style="max-width:60ch;">
          ${STATUS_TAGS.map(
            (tag) => html`
              <span style="display:inline-flex;align-items:center;gap:4px;margin-inline-end:12px;">
                ${withSize(tag)}${tag.replace('wc-icon-status-', '')}
              </span>
            `,
          )}
        </p>
      `,
    },
  ],
};

export default preview;
