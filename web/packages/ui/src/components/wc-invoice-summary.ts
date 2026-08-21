import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-money.js';
import './wc-invoice-status.js';

/**
 * The header of an invoice detail view: who, how much, how much is left, and
 * the two dates.
 *
 * Total and outstanding are the two figures a person chasing a receivable
 * reads first, so they are the two the header carries; everything else the
 * invoice knows is in the panels below it.
 */
@customElement('wc-invoice-summary')
export class WcInvoiceSummary extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    /* The dates and the currency code, set like the amounts above them. */
    dd.figure {
      font-family: var(--nc-font-figures);
      font-variant-numeric: tabular-nums;
    }

    .title {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: var(--wa-space-s, 8px);
    }

    h2 {
      margin: 0;
      font-size: var(--wa-font-size-l, 18px);
      user-select: text;
    }

    .client {
      color: var(--wa-color-muted);
      user-select: text;
    }

    .facts {
      display: flex;
      flex-wrap: wrap;
      align-items: baseline;
      gap: var(--wa-space-xs, 6px) var(--wa-space-m, 12px);
      margin: var(--wa-space-xs, 6px) 0 0;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }

    .facts .figure {
      color: var(--wa-color-text);
      font-size: var(--wa-font-size-m, 15px);
    }

    dl {
      display: contents;
    }

    dt {
      color: var(--wa-color-muted);
    }

    dd {
      margin: 0;
      color: var(--wa-color-text);
      user-select: text;
    }

    .pair {
      display: flex;
      align-items: baseline;
      gap: var(--wa-space-2xs, 4px);
    }
  `;

  @property({ type: Number })
  number = 0;

  @property({ type: String })
  status = 'draft';

  /** Null renders as an em dash — an invoice can outlive its client row. */
  @property({ type: String, attribute: false })
  clientName: string | null = null;

  @property({ type: Number })
  total = 0;

  /**
   * Outstanding. Null renders as an em dash, which is what a void invoice
   * gets: it owes nothing and never will, and its total under "Outstanding"
   * would report a receivable that no longer exists.
   */
  @property({ type: Number })
  balance: number | null = 0;

  @property({ type: String })
  currency = 'USD';

  @property({ type: String, attribute: false })
  issueDate = '';

  /** Null means the invoice can never go overdue, which is worth showing. */
  @property({ type: String, attribute: false })
  dueDate: string | null = null;

  render() {
    return html`
      <div class="title">
        <h2>Invoice #${this.number}</h2>
        <wc-invoice-status status=${this.status}></wc-invoice-status>
        <span class="client">${this.clientName ?? '—'}</span>
      </div>

      <dl class="facts">
        <div class="pair">
          <dt>Total</dt>
          <dd class="figure">
            <wc-money
              .amount=${this.total}
              .currency=${this.currency}
              variant="plain"
              data-total
            ></wc-money>
          </dd>
        </div>
        <div class="pair">
          <dt>Outstanding</dt>
          <dd class="figure">
            ${this.balance === null
              ? html`<span data-balance>—</span>`
              : html`<wc-money
                  .amount=${this.balance}
                  .currency=${this.currency}
                  variant="plain"
                  data-balance
                ></wc-money>`}
          </dd>
        </div>
        <div class="pair">
          <dt>Issued</dt>
          <dd class="figure">${this.issueDate || '—'}</dd>
        </div>
        <div class="pair">
          <dt>Due</dt>
          <dd class="figure" data-due>${this.dueDate ?? '—'}</dd>
        </div>
        ${this.currency
          ? html`<div class="pair">
              <dt>Currency</dt>
              <dd class="figure">${this.currency}</dd>
            </div>`
          : nothing}
      </dl>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-invoice-summary': WcInvoiceSummary;
  }
}
