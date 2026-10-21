import { html } from 'lit';
import './wc-nav-sidebar.js';
import { NAV_ITEMS, NAV_ITEMS_WITH_DISABLED } from './__mocks__/nav.js';
import type { NavItem } from './wc-nav-sidebar.js';
import type { Preview } from '../../preview/types.js';

/** The panel floats in a window-high column, so it gets one to float in. */
const frame = (items: NavItem[], active: string, collapsed = false) => html`
  <div style="display:flex;height:620px;background:var(--wa-color-bg);">
    <wc-nav-sidebar .items=${items} active=${active} ?collapsed=${collapsed}></wc-nav-sidebar>
  </div>
`;

const preview: Preview = {
  id: 'wc-nav-sidebar',
  title: 'Nav Sidebar',
  group: 'Layout',
  description:
    'The floating sidebar panel: hamburger and wordmark, grouped nav, the Review count and Settings pinned in the footer. Presentational — the app screen registry supplies the items and the active id.',
  layout: 'grid-wide',
  states: [
    { name: 'expanded', render: () => frame(NAV_ITEMS, 'dashboard') },
    { name: 'collapsed', render: () => frame(NAV_ITEMS, 'dashboard', true) },
    { name: 'active in a group', render: () => frame(NAV_ITEMS, 'invoices') },
    { name: 'active in the footer', render: () => frame(NAV_ITEMS, 'settings') },
    {
      name: 'collapsed, active in a group',
      render: () => frame(NAV_ITEMS, 'invoices', true),
    },
    {
      name: 'nothing to review',
      render: () =>
        frame(
          NAV_ITEMS.map((item) => (item.id === 'review' ? { ...item, count: 0 } : item)),
          'review',
        ),
    },
    { name: 'with-disabled', render: () => frame(NAV_ITEMS_WITH_DISABLED, 'dashboard') },
    {
      name: 'no-icons',
      render: () =>
        frame(
          NAV_ITEMS.map((item) => ({ ...item, icon: undefined })),
          'dashboard',
        ),
    },
  ],
};

export default preview;
