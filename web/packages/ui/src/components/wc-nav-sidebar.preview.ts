import { html } from 'lit';
import './wc-nav-sidebar.js';
import { NAV_ITEMS, NAV_ITEMS_WITH_DISABLED } from './__mocks__/nav.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-nav-sidebar',
  title: 'Nav Sidebar',
  group: 'Layout',
  description:
    'Primary navigation. Presentational — the app screen registry supplies the items and the active id.',
  layout: 'grid-wide',
  states: [
    {
      name: 'expanded',
      render: () =>
        html`<wc-nav-sidebar
          .items=${NAV_ITEMS}
          active="register"
        ></wc-nav-sidebar>`,
    },
    {
      name: 'collapsed',
      render: () =>
        html`<wc-nav-sidebar
          .items=${NAV_ITEMS}
          active="register"
          collapsed
        ></wc-nav-sidebar>`,
    },
    {
      // The rail slides between the two states above, and a still frame
      // cannot show that. This one is here to be clicked.
      name: 'toggling',
      render: () => {
        const flip = (event: Event) => {
          const sidebar = (event.currentTarget as HTMLElement)
            .previousElementSibling as HTMLElement & { collapsed: boolean };
          sidebar.collapsed = !sidebar.collapsed;
        };
        return html`
          <wc-nav-sidebar .items=${NAV_ITEMS} active="register"></wc-nav-sidebar>
          <button type="button" @click=${flip}>Collapse and expand the rail</button>
        `;
      },
    },
    {
      name: 'with-disabled',
      render: () =>
        html`<wc-nav-sidebar
          .items=${NAV_ITEMS_WITH_DISABLED}
          active="dashboard"
        ></wc-nav-sidebar>`,
    },
    {
      name: 'no-icons',
      render: () =>
        html`<wc-nav-sidebar
          .items=${NAV_ITEMS.map(({ id, label }) => ({ id, label }))}
          active="dashboard"
        ></wc-nav-sidebar>`,
    },
  ],
};

export default preview;
