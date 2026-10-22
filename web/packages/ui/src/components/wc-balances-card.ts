import { LitElement, html, css, nothing } from 'lit';
import { controlsCss } from '@nigel/theme';
import '@awesome.me/webawesome/dist/components/button/button.js';
import { customElement, property } from 'lit/decorators.js';
import './wc-money.js';
import './wc-spinner.js';
import { accountTypeLabel } from './account-type.js';
import type { BalanceRow } from './wc-balance-list.js';

/**
 * Account balances in a titled card: a row per account with its type under
 * the name and the balance, coloured by sign, at the right; a Total row after
 * a heavier rule.
 *
 * A real `<table>`, so a screen reader reads a balance with its account.
 */
@customElement('wc-balances-card')
export class WcBalancesCard extends LitElement {
  static styles = [controlsCss, css`
    :host {
      display: block;
      min-width: 0;
      box-sizing: border-box;
      padding: 18px 22px;
      background: var(--wa-color-surface);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-lg);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .title {
      margin: 0 0 var(--wa-space-s);
      font-size: var(--nc-font-size-card-title);
      font-weight: var(--wa-font-weight-semibold);
    }

    /* Account names are user data, so no width budget survives a long one:
       the card scrolls to the table rather than cutting a balance off. */
    .scroller {
      overflow-x: auto;
    }

    table {
      width: 100%;
      border-collapse: collapse;
    }

    th[scope='row'],
    td {
      padding: 10px 0;
      text-align: left;
      font-weight: var(--wa-font-weight-normal);
      border-top: 1px solid var(--wa-color-border-soft);
    }

    tbody tr:first-child th,
    tbody tr:first-child td {
      border-top: none;
    }

    th[scope='row'] {
      user-select: text;
    }

    td.amount {
      text-align: right;
      width: 12ch;
    }

    .type {
      display: block;
      font-size: var(--nc-font-size-meta);
      color: var(--wa-color-muted);
    }

    tfoot th,
    tfoot td {
      border-top: 1px solid var(--wa-color-border);
      font-weight: var(--wa-font-weight-semibold);
    }

    .empty,
    .error {
      margin: 0;
      font-size: var(--nc-font-size-meta);
      color: var(--wa-color-muted);
    }

    .error {
      color: var(--wa-color-danger);
    }

    .retry {
      margin-top: var(--wa-space-xs);
    }

    .sr-only {
      position: absolute;
      width: 1px;
      height: 1px;
      padding: 0;
      margin: -1px;
      overflow: hidden;
      clip: rect(0, 0, 0, 0);
      white-space: nowrap;
      border: 0;
    }
  `];

  @property({ attribute: false })
  items: BalanceRow[] = [];

  /** Rendered as a Total row. Omit it and no total is shown. */
  @property({ type: Number })
  total?: number;

  @property({ type: String })
  heading = 'Balances';

  @property({ type: String, attribute: 'empty-message' })
  emptyMessage = 'No accounts yet.';

  @property({ type: Boolean, reflect: true })
  loading = false;

  @property({ type: String })
  error = '';

  private handleRetry = () => {
    this.dispatchEvent(
      new CustomEvent('nc-retry', { bubbles: true, composed: true }),
    );
  };

  private renderBody() {
    if (this.loading) {
      return html`<wc-spinner show-label label="Loading balances"></wc-spinner>`;
    }

    if (this.error) {
      return html`
        <p class="error">${this.error}</p>
        <wa-button class="retry" size="small" @click=${this.handleRetry}>
          Retry
        </wa-button>
      `;
    }

    if (this.items.length === 0) {
      return html`<p class="empty">${this.emptyMessage}</p>`;
    }

    return html`
      <div class="scroller">
        <table>
          <caption class="sr-only">
            ${this.heading}
          </caption>
          <thead class="sr-only">
            <tr>
              <th scope="col">Account</th>
              <th scope="col">Balance</th>
            </tr>
          </thead>
          <tbody>
            ${this.items.map(
              (item) => html`
                <tr>
                  <th scope="row">
                    ${item.name}
                    ${item.accountType
                      ? html`<span class="type"
                          >${accountTypeLabel(item.accountType).toLowerCase()}</span
                        >`
                      : nothing}
                  </th>
                  <td class="amount">
                    <wc-money .amount=${item.balance} align="end"></wc-money>
                  </td>
                </tr>
              `,
            )}
          </tbody>
          ${this.total === undefined
            ? nothing
            : html`
                <tfoot>
                  <tr>
                    <th scope="row">Total</th>
                    <td class="amount">
                      <wc-money .amount=${this.total} align="end"></wc-money>
                    </td>
                  </tr>
                </tfoot>
              `}
        </table>
      </div>
    `;
  }

  render() {
    return html`<h2 class="title">${this.heading}</h2>
      ${this.renderBody()}`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-balances-card': WcBalancesCard;
  }
}
