import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-money.js';
import './wc-invoice-status.js';
import { shortDate } from './short-date.js';

/**
 * The head of an invoice's detail view: the title with its status, the
 * actions beside it, and a four-column fact strip — Client, Issued, Due and
 * Balance.
 *
 * Balance is the figure a person chasing a receivable reads first, so it is
 * the large one, with the total it is out of underneath. The actions are
 * slotted (`slot="actions"`), because what they are and when they are enabled
 * is the screen's to decide.
 */
@customElement('wc-invoice-summary')
export class WcInvoiceSummary extends LitElement {
  static styles = css`
    :host {
      display: grid;
      gap: 16px;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .title {
      display: flex;
      flex-wrap: wrap;
      align-items: flex-start;
      gap: 12px 16px;
    }

    .heading {
      display: flex;
      align-items: center;
      gap: 12px;
      min-height: 36px;
    }

    h1 {
      margin: 0;
      font-size: var(--nc-font-size-display);
      font-weight: var(--wa-font-weight-medium);
      letter-spacing: var(--nc-letter-spacing-tight);
      line-height: 1.2;
      white-space: nowrap;
      user-select: text;
    }

    .actions {
      flex: 1 1 auto;
      display: flex;
      justify-content: flex-end;
      padding-top: 2px;
    }

    dl {
      display: grid;
      grid-template-columns: repeat(4, minmax(0, 1fr));
      margin: 0;
      background: var(--wa-color-surface);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-lg);
    }

    @media (max-width: 48rem) {
      dl {
        grid-template-columns: repeat(2, minmax(0, 1fr));
      }

      .fact:nth-child(3) {
        border-left: 0;
      }

      .fact:nth-child(n + 3) {
        border-top: 1px solid var(--wa-color-border-soft);
      }
    }

    .fact {
      display: grid;
      align-content: start;
      gap: 2px;
      min-width: 0;
      padding: 14px 16px;
    }

    .fact + .fact {
      border-left: 1px solid var(--wa-color-border-soft);
    }

    dt {
      margin-bottom: 4px;
      color: var(--wa-color-muted);
      font-size: var(--nc-font-size-meta);
    }

    dd {
      margin: 0;
      font-size: var(--nc-font-size-body);
      overflow-wrap: anywhere;
      user-select: text;
    }

    .sub {
      color: var(--wa-color-muted);
      font-size: var(--nc-font-size-meta);
    }

    [data-overdue] {
      color: var(--nc-color-expense);
    }

    .balance {
      font-size: var(--nc-font-size-heading-sm);
      font-weight: var(--wa-font-weight-medium);
      letter-spacing: var(--nc-letter-spacing-tight);
      line-height: 1.2;
    }
  `;

  @property({ type: Number })
  number = 0;

  @property({ type: String })
  status = 'draft';

  /** Null renders as an em dash — an invoice can outlive its client row. */
  @property({ type: String, attribute: false })
  clientName: string | null = null;

  /** The billing address the invoice goes to; null prints nothing. */
  @property({ type: String, attribute: false })
  clientEmail: string | null = null;

  @property({ type: Number })
  total = 0;

  /**
   * Outstanding. Null renders as an em dash, which is what a void invoice
   * gets: it owes nothing and never will, and a figure under "Balance" would
   * report a receivable that no longer exists.
   */
  @property({ type: Number })
  balance: number | null = 0;

  @property({ type: String })
  currency = 'USD';

  @property({ type: String, attribute: false })
  issueDate = '';

  /** Null is an invoice due on receipt, which can never go overdue. */
  @property({ type: String, attribute: false })
  dueDate: string | null = null;

  /** A line under the due date: "19 days overdue", "Due tomorrow". Empty prints none. */
  @property({ type: String, attribute: false })
  dueNote = '';

  render() {
    const overdue = this.status === 'overdue';

    return html`
      <div class="title">
        <div class="heading">
          <h1>Invoice #${this.number}</h1>
          <wc-invoice-status status=${this.status} size="l"></wc-invoice-status>
        </div>
        <div class="actions"><slot name="actions"></slot></div>
      </div>

      <dl>
        <div class="fact">
          <dt>Client</dt>
          <dd data-client>${this.clientName ?? '—'}</dd>
          ${this.clientEmail
            ? html`<dd class="sub" data-client-email>${this.clientEmail}</dd>`
            : nothing}
        </div>
        <div class="fact">
          <dt>Issued</dt>
          <dd data-issued>${this.issueDate ? shortDate(this.issueDate) : '—'}</dd>
        </div>
        <div class="fact">
          <dt>Due</dt>
          <dd data-due ?data-overdue=${overdue}>
            ${this.dueDate === null ? 'On receipt' : shortDate(this.dueDate)}
          </dd>
          ${this.dueNote
            ? html`<dd class="sub" data-due-note ?data-overdue=${overdue}>${this.dueNote}</dd>`
            : nothing}
        </div>
        <div class="fact">
          <dt>Balance</dt>
          <dd class="balance">
            ${this.balance === null
              ? html`<span data-balance>—</span>`
              : html`<wc-money
                  .amount=${this.balance}
                  .currency=${this.currency}
                  variant="plain"
                  data-balance
                ></wc-money>`}
          </dd>
          <dd class="sub">
            of
            <wc-money
              .amount=${this.total}
              .currency=${this.currency}
              variant="plain"
              data-total
            ></wc-money>
          </dd>
        </div>
      </dl>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-invoice-summary': WcInvoiceSummary;
  }
}
