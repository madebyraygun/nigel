import { html } from 'lit';
import './wc-app-shell.js';
import './wc-nav-sidebar.js';
import './wc-empty-state.js';
import { NAV_ITEMS } from './__mocks__/nav.js';
import type { BreadcrumbItem } from './wc-breadcrumb.js';
import type { Preview } from '../../preview/types.js';

interface ShellAttrs {
  title?: string;
  active?: string;
  trail?: BreadcrumbItem[];
  book?: string;
  collapsed?: boolean;
  hosted?: boolean;
  /** Stands in for a phone-width viewport, which a desktop preview is not. */
  narrow?: boolean;
}

const shell = (extra = html``, attrs: ShellAttrs = {}) => {
  const title = attrs.title ?? 'Dashboard';
  const width = attrs.narrow ? '390px' : 'min(1280px, calc(100vw - 32px))';
  return html`
    <div
      style="width:${width};height:720px;border:1px solid var(--wa-color-border);overflow:hidden;"
    >
      <wc-app-shell
        screen-title=${title}
        book-name=${attrs.book ?? 'Acme Consulting LLC'}
        book-key="acme-consulting"
        .trail=${attrs.trail ?? [{ label: title }]}
        ?sidebar-collapsed=${attrs.collapsed ?? false}
        ?hide-sidebar=${attrs.hosted ?? false}
        review-count="84"
        .statusLine=${attrs.hosted
          ? undefined
          : { path: '~/Documents/Nigel/acme-consulting', encrypted: true, version: '1.0' }}
        .narrowQuery=${attrs.narrow ? { matches: true } : undefined}
        style="height:100%;"
      >
        ${attrs.hosted
          ? html``
          : html`<wc-nav-sidebar
              slot="sidebar"
              ?collapsed=${attrs.collapsed ?? false}
              .items=${NAV_ITEMS}
              active=${attrs.active ?? 'dashboard'}
            ></wc-nav-sidebar>`}
        ${extra}
        <wc-empty-state heading=${title} message="Screen content goes here."></wc-empty-state>
      </wc-app-shell>
    </div>
  `;
};

const preview: Preview = {
  id: 'wc-app-shell',
  title: 'App Shell',
  group: 'Layout',
  description:
    'The frame: floating sidebar slot, the top bar (breadcrumb, search pill, refresh), a banner slot, the page heading, the content, and the single toast region.',
  layout: 'stack',
  states: [
    { name: 'default', render: () => shell() },
    { name: 'sidebar-collapsed', render: () => shell(html``, { collapsed: true }) },
    {
      name: 'invoice-detail',
      render: () =>
        shell(html``, {
          title: 'Invoices',
          active: 'invoices',
          trail: [{ label: 'Invoices', id: 'invoices' }, { label: '#1254' }],
        }),
    },
    {
      name: 'settings',
      render: () => shell(html``, { title: 'Settings', active: 'settings' }),
    },
    { name: 'hosted', render: () => shell(html``, { hosted: true }) },
    {
      name: 'long book name',
      render: () =>
        shell(html``, { book: 'Harbor & Vale Architecture and Interiors Partnership LLP' }),
    },
    { name: 'drawer-open', render: () => shell(html``, { narrow: true }) },
    {
      name: 'drawer-closed',
      render: () => shell(html``, { narrow: true, collapsed: true }),
    },
    {
      name: 'with-page-actions',
      render: () => shell(html`<button slot="page-actions" type="button">Export</button>`),
    },
    {
      name: 'with-banner',
      render: () =>
        shell(
          html`<span slot="banner">Session expired — reopen the URL nigel serve printed.</span>`,
        ),
    },
  ],
};

export default preview;
