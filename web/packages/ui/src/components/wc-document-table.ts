import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-spinner.js';
import './wc-document-status.js';

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
  static styles = css`
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
      font-size: var(--wa-font-size-s, 13px);
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
      padding: var(--wa-space-xs, 6px) var(--wa-space-s, 8px);
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
      font-weight: var(--wa-font-weight-medium, 500);
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
      padding: var(--wa-space-l, 16px) 0;
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
    }
  `;

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
      <div class="scroll">
        <table>
          <caption>
            ${this.caption}
          </caption>
          <thead>
            <tr>
              <th scope="col">Title</th>
              <th scope="col">Status</th>
              <th scope="col">Kind</th>
              <th scope="col">Client</th>
              <th scope="col">Version</th>
              <th scope="col">Updated</th>
            </tr>
          </thead>
          <tbody>
            ${this.rows.map((row) => this.renderRow(row))}
          </tbody>
        </table>
      </div>
    `;
  }

  private renderRow(row: DocumentTableRow) {
    const title = row.href ? html`<a href=${row.href}>${row.title}</a>` : html`${row.title}`;

    return html`
      <tr data-row=${row.id}>
        <td class="title">${title}</td>
        <td><wc-document-status status=${row.status}></wc-document-status></td>
        <td>${row.kind}</td>
        <td class="client ${row.clientName === null ? 'muted' : ''}">
          ${row.clientName ?? '—'}
        </td>
        <td class="version">v${row.version}</td>
        <td>${row.updatedAt}</td>
      </tr>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-document-table': WcDocumentTable;
  }
}
