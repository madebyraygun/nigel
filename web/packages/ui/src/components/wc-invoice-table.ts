import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-money.js';
import './wc-spinner.js';
import './wc-invoice-status.js';
import '../icons/icons.js';
import { shortDate } from './short-date.js';

/** One invoice as the list shows it. */
export interface InvoiceTableRow {
  number: number;
  status: string;
  /** Null when the client row is gone — the join is a LEFT JOIN. */
  clientName: string | null;
  total: number;
  /**
   * Outstanding. Null renders as an em dash, which is what a void invoice
   * gets: it owes nothing and it never will, and `$0.00` would read as settled.
   */
  balance: number | null;
  /** `YYYY-MM-DD`. Absent renders as an em dash. */
  issueDate?: string;
  /** Null is an invoice due on receipt, and says so. */
  dueDate: string | null;
  /**
   * The row's own currency, since a list spans clients and need not be in one.
   * Absent falls back to `wc-money`'s default.
   */
  currency?: string;
  /** Where the row links. Empty makes the row inert text. */
  href?: string;
}

/**
 * The invoice list: Number · Client · Issued · Due · Status · Total · Balance.
 *
 * A row with an address is a link in two ways. The number is a real `<a>`, so
 * the keyboard, a screen reader and a middle click all have something to land
 * on; and a click anywhere else on the row follows that same link, because the
 * row is the target a pointer aims at. The chevron says so and is decoration.
 *
 * Not `wc-report-table`: that renders section and total bands for read-only
 * financial output. Not `wc-manager-table` either — an invoice is edited on
 * its own screen, never from a row button, because void and send are not
 * row-level affordances.
 */
@customElement('wc-invoice-table')
export class WcInvoiceTable extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .card {
      background: var(--wa-color-surface);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-lg);
      overflow: hidden;
    }

    .scroll {
      overflow-x: auto;
    }

    table {
      width: 100%;
      min-width: 52rem;
      border-collapse: collapse;
      table-layout: fixed;
      font-size: var(--nc-font-size-body);
    }

    caption,
    .sr-only {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    col.number {
      width: 84px;
    }
    col.date {
      width: 120px;
    }
    col.status {
      width: 112px;
    }
    col.money {
      width: 120px;
    }
    col.chevron {
      width: 36px;
    }

    th,
    td {
      padding: 0 8px;
      text-align: start;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }

    th {
      height: 36px;
      background: var(--wa-color-surface-alt);
      border-bottom: 1px solid var(--wa-color-border);
      color: var(--wa-color-muted);
      font-size: var(--nc-font-size-meta);
      font-weight: var(--nc-font-weight-table-heading);
    }

    td {
      height: 44px;
      border-bottom: 1px solid var(--wa-color-border-soft);
      user-select: text;
    }

    tbody tr:last-child td {
      border-bottom: 0;
    }

    tr[data-href]:hover td {
      background: var(--nc-color-highlight);
    }

    th:first-child,
    td:first-child {
      padding-left: 16px;
    }

    .end {
      text-align: end;
    }

    td.number {
      font-weight: var(--wa-font-weight-medium);
    }

    td.client {
      overflow: hidden;
      text-overflow: ellipsis;
    }

    .muted {
      color: var(--wa-color-muted);
    }

    td[data-overdue] {
      color: var(--nc-color-expense);
    }

    td[data-void] wc-money {
      text-decoration: line-through;
      color: var(--wa-color-muted);
    }

    td.balance {
      font-weight: var(--wa-font-weight-medium);
    }

    td.chevron {
      padding: 0 8px 0 0;
      color: var(--wa-color-muted);
      text-align: center;
    }

    a {
      color: inherit;
      text-decoration: none;
    }

    a:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
      border-radius: var(--wa-radius-xs);
    }

    .state {
      margin: 0;
      padding: 28px 16px;
      color: var(--wa-color-muted);
      font-size: var(--nc-font-size-control);
      text-align: center;
    }
  `;

  @property({ attribute: false })
  rows: InvoiceTableRow[] = [];

  @property({ type: Boolean, reflect: true })
  loading = false;

  @property({ type: String })
  caption = 'Invoices';

  @property({ type: String, attribute: 'empty-message' })
  emptyMessage = 'No invoices yet.';

  /** Follow the row's own link, so a click on the row and on the number agree. */
  private handleRowClick(event: MouseEvent): void {
    if (event.defaultPrevented || event.button !== 0) return;
    const target = event.target as Element | null;
    if (target?.closest('a')) return;
    const row = (event.currentTarget as HTMLElement).querySelector<HTMLAnchorElement>('a[href]');
    row?.click();
  }

  render() {
    if (this.loading) {
      return html`<div class="card">
        <div class="state"><wc-spinner show-label label="Loading invoices"></wc-spinner></div>
      </div>`;
    }

    if (this.rows.length === 0) {
      return html`<div class="card"><p class="state" data-empty>${this.emptyMessage}</p></div>`;
    }

    return html`
      <div class="card">
        <div class="scroll">
          <table>
            <caption>
              ${this.caption}
            </caption>
            <colgroup>
              <col class="number" />
              <col />
              <col class="date" />
              <col class="date" />
              <col class="status" />
              <col class="money" />
              <col class="money" />
              <col class="chevron" />
            </colgroup>
            <thead>
              <tr>
                <th scope="col">Number</th>
                <th scope="col">Client</th>
                <th scope="col">Issued</th>
                <th scope="col">Due</th>
                <th scope="col">Status</th>
                <th scope="col" class="end">Total</th>
                <th scope="col" class="end">Balance</th>
                <th scope="col"><span class="sr-only">Open</span></th>
              </tr>
            </thead>
            <tbody>
              ${this.rows.map((row) => this.renderRow(row))}
            </tbody>
          </table>
        </div>
      </div>
    `;
  }

  private renderRow(row: InvoiceTableRow) {
    const label = `#${row.number}`;
    const number = row.href ? html`<a href=${row.href}>${label}</a>` : label;
    const isVoid = row.status === 'void';
    const outstanding = row.balance !== null && row.balance > 0;

    return html`
      <tr
        data-row=${row.number}
        data-href=${row.href || nothing}
        @click=${row.href ? this.handleRowClick : nothing}
      >
        <td class="number">${number}</td>
        <td class="client ${row.clientName === null ? 'muted' : ''}">
          ${row.clientName ?? '—'}
        </td>
        <td data-issued class=${row.issueDate ? nothing : 'muted'}>
          ${row.issueDate ? shortDate(row.issueDate) : '—'}
        </td>
        <td data-due ?data-overdue=${row.status === 'overdue'}>
          ${row.dueDate === null ? 'On receipt' : shortDate(row.dueDate)}
        </td>
        <td><wc-invoice-status status=${row.status}></wc-invoice-status></td>
        <td class="end" ?data-void=${isVoid}>
          <wc-money
            .amount=${row.total}
            .currency=${row.currency ?? 'USD'}
            variant="plain"
            align="end"
          ></wc-money>
        </td>
        <td class="end balance ${outstanding ? '' : 'muted'}" data-balance>
          ${row.balance === null
            ? '—'
            : html`<wc-money
                .amount=${row.balance}
                .currency=${row.currency ?? 'USD'}
                variant="plain"
                align="end"
              ></wc-money>`}
        </td>
        <td class="chevron" aria-hidden="true">
          ${row.href ? html`<wc-icon-chevron-right inline></wc-icon-chevron-right>` : nothing}
        </td>
      </tr>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-invoice-table': WcInvoiceTable;
  }
}
