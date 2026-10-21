import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-spinner.js';
import { barHeights, type BarBucket } from './wc-bar-chart.js';
import { roundHalfEven } from './round-half-even.js';

/**
 * Monthly income against expenses in a titled card: the title, the period it
 * covers and a legend in the header, paired bars with rounded tops under it.
 *
 * Drawn in CSS like `wc-bar-chart`, and scaled with the same `barHeights`, so
 * the two charts agree on what a bar means. Bars are a picture, so the chart
 * names itself with `role="img"` and repeats the figures in a table that only
 * assistive tech reads.
 */
@customElement('wc-cashflow-card')
export class WcCashflowCard extends LitElement {
  static styles = css`
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

    header {
      display: flex;
      align-items: baseline;
      flex-wrap: wrap;
      gap: var(--wa-space-s);
      margin-bottom: var(--wa-space-m);
    }

    .title {
      margin: 0;
      font-size: var(--nc-font-size-card-title);
      font-weight: var(--wa-font-weight-semibold);
    }

    .range {
      font-size: var(--nc-font-size-meta);
      color: var(--wa-color-muted);
    }

    .legend {
      display: flex;
      gap: var(--wa-space-m);
      margin-left: auto;
      font-size: var(--nc-font-size-meta);
      color: var(--wa-color-muted);
    }

    .key {
      display: inline-flex;
      align-items: center;
      gap: 5px;
    }

    .dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
    }

    .dot.income,
    .bar.income {
      background: var(--nc-color-income-fill);
    }

    .dot.expense,
    .bar.expense {
      background: var(--nc-color-expense-fill);
    }

    /* A grid item's automatic minimum is its content, so a chart that cannot
       shrink would widen the page on a phone. A scroll container's minimum is
       zero, so the chart scrolls inside the card instead. */
    .chart {
      overflow-x: auto;
    }

    .plot {
      display: flex;
      align-items: flex-end;
      gap: 10px;
      height: 220px;
      border-bottom: 1px solid var(--wa-color-border);
    }

    .bucket {
      flex: 1 1 0;
      min-width: var(--nc-bar-bucket-min-width, 24px);
      display: flex;
      align-items: flex-end;
      justify-content: center;
      gap: 3px;
      height: 100%;
    }

    .bar {
      width: 38%;
      min-height: 1px;
      border-radius: 999px 999px 0 0;
      transition: height var(--nc-transition-fast);
    }

    .labels {
      display: flex;
      gap: 10px;
      margin-top: 8px;
    }

    .tick {
      flex: 1 1 0;
      min-width: var(--nc-bar-bucket-min-width, 24px);
      text-align: center;
      font-size: var(--nc-font-size-meta);
      color: var(--wa-color-muted);
    }

    .error {
      margin: 0;
      font-size: var(--nc-font-size-meta);
      color: var(--wa-color-danger);
    }

    .retry {
      margin-top: var(--wa-space-xs);
      font: inherit;
      font-size: var(--nc-font-size-meta);
      color: var(--wa-color-brand);
      background: none;
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-sm);
      padding: var(--wa-space-2xs) var(--wa-space-s);
      cursor: default;
    }

    .retry:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
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
  `;

  @property({ attribute: false })
  buckets: BarBucket[] = [];

  /** The period the chart covers, e.g. "2025 - 26". */
  @property({ type: String })
  range = '';

  @property({ type: String })
  heading = 'Cash flow';

  @property({ type: String })
  currency = 'USD';

  /** Undefined uses the runtime's default locale. */
  @property({ type: String })
  locale?: string;

  @property({ type: Boolean, reflect: true })
  loading = false;

  @property({ type: String })
  error = '';

  private handleRetry = () => {
    this.dispatchEvent(
      new CustomEvent('nc-retry', { bubbles: true, composed: true }),
    );
  };

  private money(amount: number): string {
    const format = new Intl.NumberFormat(this.locale, {
      style: 'currency',
      currency: this.currency,
    });
    return format.format(
      roundHalfEven(amount, format.resolvedOptions().maximumFractionDigits ?? 2),
    );
  }

  private get summary(): string {
    const income = this.buckets.reduce((sum, b) => sum + b.income, 0);
    const expense = this.buckets.reduce((sum, b) => sum + b.expense, 0);
    const period = this.range ? ` for ${this.range}` : '';
    return `Income and expenses by month${period}: ${this.money(
      income,
    )} in, ${this.money(expense)} out across ${this.buckets.length} months.`;
  }

  private renderBody() {
    if (this.loading) {
      return html`<wc-spinner show-label label="Loading cash flow"></wc-spinner>`;
    }

    if (this.error) {
      return html`
        <p class="error">${this.error}</p>
        <button class="retry" type="button" @click=${this.handleRetry}>
          Retry
        </button>
      `;
    }

    if (this.buckets.length === 0) {
      return html`<slot name="empty"></slot>`;
    }

    const heights = barHeights(this.buckets);

    return html`
      <div class="chart" role="img" aria-label=${this.summary}>
        <div class="plot">
          ${this.buckets.map(
            (bucket, i) => html`
              <div class="bucket">
                <div
                  class="bar income"
                  style="height:${heights[i]?.income ?? 0}%"
                  title="${bucket.label} income ${this.money(bucket.income)}"
                ></div>
                <div
                  class="bar expense"
                  style="height:${heights[i]?.expense ?? 0}%"
                  title="${bucket.label} expenses ${this.money(bucket.expense)}"
                ></div>
              </div>
            `,
          )}
        </div>
        <div class="labels">
          ${this.buckets.map((b) => html`<span class="tick">${b.label}</span>`)}
        </div>
      </div>
      <table class="sr-only">
        <caption>
          ${this.range || 'Monthly income and expenses'}
        </caption>
        <thead>
          <tr>
            <th scope="col">Month</th>
            <th scope="col">Income</th>
            <th scope="col">Expenses</th>
          </tr>
        </thead>
        <tbody>
          ${this.buckets.map(
            (b) => html`
              <tr>
                <th scope="row">${b.label}</th>
                <td>${this.money(b.income)}</td>
                <td>${this.money(b.expense)}</td>
              </tr>
            `,
          )}
        </tbody>
      </table>
    `;
  }

  render() {
    return html`
      <header>
        <h2 class="title">${this.heading}</h2>
        ${this.range ? html`<span class="range">${this.range}</span>` : nothing}
        <span class="legend" aria-hidden="true">
          <span class="key"><span class="dot income"></span>Income</span>
          <span class="key"><span class="dot expense"></span>Expenses</span>
        </span>
      </header>
      ${this.renderBody()}
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-cashflow-card': WcCashflowCard;
  }
}
