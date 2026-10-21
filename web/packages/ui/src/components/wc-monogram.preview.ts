import { html } from 'lit';
import './wc-monogram.js';
import type { Preview } from '../../preview/types.js';

const CAST = [
  'Acme Consulting LLC',
  'Cedar Systems',
  'Juniper Labs',
  'Harbor & Vale',
  'Globex',
  'Initech, Inc.',
];

const row = (size: 's' | 'l') => html`
  <div style="display:flex;flex-wrap:wrap;gap:16px;">
    ${CAST.map(
      (name) =>
        html`<span style="display:inline-flex;align-items:center;gap:8px;">
          <wc-monogram name=${name} seed=${name} size=${size}></wc-monogram>
          <span>${name}</span>
        </span>`,
    )}
  </div>
`;

const preview: Preview = {
  id: 'wc-monogram',
  title: 'Monogram',
  group: 'Layout',
  description:
    'A book’s initials on a pastel tile. The colour is picked from the ramp by hashing a stable key, so a book keeps it.',
  states: [
    { name: 'breadcrumb size', render: () => row('s') },
    { name: 'card size', render: () => row('l') },
  ],
};

export default preview;
