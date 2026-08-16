import { html } from 'lit';
import './wc-app-shell.js';
import './wc-nav-sidebar.js';
import './wc-empty-state.js';
import { NAV_ITEMS } from './__mocks__/nav.js';
import type { Preview } from '../../preview/types.js';

interface ShellAttrs {
  title?: string;
  collapsed?: boolean;
  /** Stands in for a phone-width viewport, which a desktop preview is not. */
  narrow?: boolean;
}

const shell = (extra = html``, attrs: ShellAttrs = {}) => html`
  <div style="height:420px;border:1px solid var(--wa-color-border);overflow:hidden;">
    <wc-app-shell
      screen-title=${attrs.title ?? 'Dashboard'}
      ?sidebar-collapsed=${attrs.collapsed ?? false}
      .narrowQuery=${attrs.narrow ? { matches: true } : undefined}
      style="height:100%;"
    >
      <wc-nav-sidebar
        slot="sidebar"
        ?collapsed=${attrs.collapsed ?? false}
        .items=${NAV_ITEMS}
        active="dashboard"
      ></wc-nav-sidebar>
      ${extra}
      <wc-empty-state
        heading="Dashboard"
        message="Screen content goes here."
      ></wc-empty-state>
    </wc-app-shell>
  </div>
`;

const preview: Preview = {
  id: 'wc-app-shell',
  title: 'App Shell',
  group: 'Layout',
  description:
    'Structural frame: sidebar slot, header, banner slot, content, and the single toast region.',
  layout: 'stack',
  states: [
    { name: 'default', render: () => shell() },
    {
      name: 'sidebar-collapsed',
      render: () => shell(html``, { collapsed: true }),
    },
    {
      name: 'drawer-open',
      render: () => shell(html``, { narrow: true }),
    },
    {
      name: 'drawer-closed',
      render: () => shell(html``, { narrow: true, collapsed: true }),
    },
    {
      name: 'with-header-actions',
      render: () =>
        shell(html`<button slot="header-actions" type="button">Export</button>`),
    },
    {
      name: 'with-banner',
      render: () =>
        shell(
          html`<span slot="banner"
            >Session expired — reopen the URL nigel serve printed.</span
          >`,
        ),
    },
  ],
};

export default preview;
