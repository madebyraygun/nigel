import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-spinner.js';
import './wc-document-status.js';
import { stackedTableCss } from './stacked-table.js';

/** One document as the list shows it. */
export interface DocumentTableRow {
  id: number;
  title: string;
  kind: string;
  status: string;
  /** Null when the client row is gone. */
  clientName: string | null;
  version: number;
  updatedAt: string;
  /** Where the title links. Empty makes the title inert text. */
  href?: string;
}

/** The document list: title, status, kind, client, version and last update. */
@customElement('wc-document-table')
export class WcDocumentTable extends LitElement {
  static styles = [
    css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .scroll {
      overflow-x: auto;
    }

    table {
      width: 100%;
      border-collapse: collapse;
      font-size: var(--wa-font-size-s);
    }

    caption {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    th,
    td {
      padding: var(--wa-space-xs) var(--wa-space-s);
      border-bottom: 1px solid var(--wa-color-border);
      text-align: start;
      vertical-align: baseline;
      white-space: nowrap;
    }

    td {
      user-select: text;
    }

    th {
      color: var(--wa-color-muted);
      font-weight: var(--nc-font-weight-table-heading);
    }

    td.title,
    td.client {
      white-space: normal;
    }

    td.version {
      font-variant-numeric: tabular-nums;
    }

    .muted {
      color: var(--wa-color-muted);
    }

    a {
      color: inherit;
      text-decoration: none;
    }

    a:hover {
      text-decoration: underline;
    }

    a:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
    }

    .state {
      padding: var(--wa-space-l) 0;
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s);
    }
  `,
    stackedTableCss,
  ];

  @property({ attribute: false })
  rows: DocumentTableRow[] = [];

  @property({ type: Boolean, reflect: true })
  loading = false;

  @property({ type: String })
  caption = 'Documents';

  @property({ type: String, attribute: 'empty-message' })
  emptyMessage = 'No documents yet.';

  render() {
    if (this.loading) {
      return html`<div class="state">
        <wc-spinner show-label label="Loading documents"></wc-spinner>
      </div>`;
    }

    if (this.rows.length === 0) {
      return html`<p class="state" data-empty>${this.emptyMessage}</p>`;
    }

    return html`
      <div class="scroll stack">
        <table role="table">
          <caption>
            ${this.caption}
          </caption>
          <thead role="rowgroup">
            <tr role="row">
              <th scope="col" role="columnheader">Title</th>
              <th scope="col" role="columnheader">Status</th>
              <th scope="col" role="columnheader">Kind</th>
              <th scope="col" role="columnheader">Client</th>
              <th scope="col" role="columnheader">Version</th>
              <th scope="col" role="columnheader">Updated</th>
            </tr>
          </thead>
          <tbody role="rowgroup">
            ${this.rows.map((row) => this.renderRow(row))}
          </tbody>
        </table>
      </div>
    `;
  }

  private renderRow(row: DocumentTableRow) {
    const title = row.href ? html`<a href=${row.href}>${row.title}</a>` : html`${row.title}`;

    return html`
      <tr role="row" data-row=${row.id}>
        <td role="cell" class="title" data-stack="title">${title}</td>
        <td role="cell" data-stack="last">
          <wc-document-status status=${row.status}></wc-document-status>
        </td>
        <td role="cell" data-label="Kind">${row.kind}</td>
        <td
          role="cell"
          class="client ${row.clientName === null ? 'muted' : ''}"
          data-label="Client"
        >
          ${row.clientName ?? '—'}
        </td>
        <td role="cell" class="version" data-label="Version">v${row.version}</td>
        <td role="cell" data-label="Updated">${row.updatedAt}</td>
      </tr>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-document-table': WcDocumentTable;
  }
}
