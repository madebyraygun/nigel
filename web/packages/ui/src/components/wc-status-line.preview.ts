import { html, type TemplateResult } from 'lit';
import './wc-status-line.js';
import type { Preview } from '../../preview/types.js';

const wide = (inner: TemplateResult) =>
  html`<div style="width:min(1100px, calc(100vw - 32px));">${inner}</div>`;

const preview: Preview = {
  id: 'wc-status-line',
  title: 'Status Line',
  group: 'Layout',
  description:
    'The floating bar under the content: book, folder, lock state, review count, palette hint and version. Hidden in hosted mode and below the mobile breakpoint.',
  layout: 'stack',
  states: [
    {
      name: 'encrypted',
      render: () =>
        wide(
          html`<wc-status-line
            book-name="Acme Consulting LLC"
            book-key="acme"
            path="~/Documents/Nigel/acme-consulting"
            encrypted
            review-count="84"
            version="1.0"
          ></wc-status-line>`,
        ),
    },
    {
      name: 'not encrypted',
      render: () =>
        wide(
          html`<wc-status-line
            book-name="Juniper Labs"
            book-key="juniper"
            path="~/Documents/Nigel/juniper-labs"
            review-count="12"
            version="1.0"
          ></wc-status-line>`,
        ),
    },
    {
      name: 'nothing to review',
      render: () =>
        wide(
          html`<wc-status-line
            book-name="Acme Consulting LLC"
            book-key="acme"
            path="~/Documents/Nigel/acme-consulting"
            encrypted
            review-count="0"
            version="1.0"
          ></wc-status-line>`,
        ),
    },
    {
      name: 'long path',
      render: () =>
        html`<div style="width:640px;">
          <wc-status-line
            book-name="Harbor & Vale Architecture and Interiors Partnership"
            book-key="harbor"
            path="~/Documents/Nigel/clients/harbor-and-vale/architecture-and-interiors/books/current-year"
            encrypted
            review-count="3"
            version="1.0"
          ></wc-status-line>
        </div>`,
    },
    {
      name: 'store build',
      render: () =>
        wide(
          html`<wc-status-line
            book-name="Globex"
            book-key="globex"
            path="~/Documents/Nigel/globex"
            encrypted
            review-count="5"
            version="1.0"
            distribution="store"
            shortcut="Ctrl K"
            key-shortcuts="Control+K"
          ></wc-status-line>`,
        ),
    },
  ],
};

export default preview;
