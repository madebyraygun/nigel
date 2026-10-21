import { html } from 'lit';
import './wc-command-palette.js';
import type { PaletteBook, PaletteScreen } from './wc-command-palette.js';
import type { PaletteScope } from './wc-breadcrumb.js';
import { NAV_ITEMS } from './__mocks__/nav.js';
import type { Preview } from '../../preview/types.js';

const BOOKS: PaletteBook[] = [
  {
    id: '~/Documents/Nigel/acme-consulting',
    name: 'Acme Consulting LLC',
    kind: 'Business',
    path: '~/Documents/Nigel/acme-consulting',
    encrypted: true,
    open: true,
  },
];

const HOUSEHOLD: PaletteBook[] = [
  {
    id: '~/Documents/Nigel/household',
    name: 'Household',
    kind: 'Personal',
    path: '~/Documents/Nigel/household',
    encrypted: false,
    open: true,
  },
];

const SCREENS: PaletteScreen[] = NAV_ITEMS.map((item) => ({
  id: item.id,
  label: item.id === 'undo' ? 'Undo last import' : item.label,
  icon: item.icon ?? 'wc-icon-dashboard',
}));

/**
 * The palette fixes itself over the viewport. The frame's transform makes it
 * the containing block instead, so each state draws inside its own box.
 */
const frame = (open: boolean, query = '', scope: PaletteScope = 'all', books = BOOKS) => html`
  <div
    style="position:relative;width:min(1100px, calc(100vw - 32px));height:600px;overflow:hidden;transform:translateZ(0);background:var(--wa-color-bg);border:1px solid var(--wa-color-border);"
  >
    <wc-command-palette
      ?open=${open}
      scope=${scope}
      query=${query}
      .books=${books}
      .screens=${SCREENS}
    ></wc-command-palette>
  </div>
`;

const preview: Preview = {
  id: 'wc-command-palette',
  title: 'Command Palette',
  group: 'Overlays',
  description:
    'The ⌘K palette: books and screens in one list. Arrow keys move through it, Enter opens, Esc or the backdrop closes.',
  layout: 'stack',
  states: [
    { name: 'closed', render: () => frame(false) },
    { name: 'open', render: () => frame(true) },
    { name: 'typing with matches', render: () => frame(true, 'ac') },
    { name: 'empty', render: () => frame(true, 'zebra') },
    { name: 'books scope', render: () => frame(true, '', 'books') },
    { name: 'unencrypted book', render: () => frame(true, '', 'books', HOUSEHOLD) },
  ],
};

export default preview;
