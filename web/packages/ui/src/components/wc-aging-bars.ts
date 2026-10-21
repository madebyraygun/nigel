import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-money.js';
import { shortDate } from './short-date.js';

/** One aging bucket, in the shape `ar_aging` reports it. */
export interface AgingBucketView {
  label: string;
  count: number;
  total: number;
}

/** What the legend calls each of `ar_aging`'s bucket keys. */
const BUCKET_LABELS = new Map<string, string>([
  ['current', 'Current'],
  ['1-30', '1–30 days'],
  ['31-60', '31–60 days'],
  ['61-90', '61–90 days'],
  ['90+', 'Over 90'],
]);

/** A bucket key as the legend prints it; a key it does not know, as written. */
export function agingBucketLabel(label: string): string {
  return BUCKET_LABELS.get(label) ?? label;
}

/**
 * Each bucket's share of the bar, as a fraction of the buckets' sum.
 *
 * Measured against the buckets themselves rather than the `total` property,
 * so the segments always fill the track exactly once. An empty bucket draws
 * no segment, and an all-zero report draws only the track.
 */
export function agingShares(buckets: AgingBucketView[]): number[] {
  const sum = buckets.reduce((acc, bucket) => acc + Math.max(0, bucket.total), 0);
  if (sum === 0) return buckets.map(() => 0);
  return buckets.map((bucket) => Math.max(0, bucket.total) / sum);
}

/**
 * The Outstanding card: what is owed, across how many open invoices, and how
 * old it is — a stacked bar over the five aging buckets with a legend under it.
 *
 * The bar is decoration: a segment has no accessible value, so it rides along
 * `aria-hidden` and every figure lives in the legend's definition list, where
 * each bucket is a label and an amount. Every figure here is a second view of
 * the aging report, which is why the invoicing parity walk compares this card
 * against `nigel invoice aging` on its own.
 */
@customElement('wc-aging-bars')
export class WcAgingBars extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .card {
      display: grid;
      gap: 12px;
      padding: 16px 20px 18px;
      background: var(--wa-color-surface);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-lg);
    }

    .top {
      display: flex;
      flex-wrap: wrap;
      align-items: baseline;
      gap: 4px 12px;
    }

    h2 {
      margin: 0;
      font-size: var(--nc-font-size-control);
      font-weight: var(--wa-font-weight-normal);
    }

    .amount {
      font-size: var(--nc-font-size-figure);
      font-weight: var(--wa-font-weight-medium);
      letter-spacing: var(--nc-letter-spacing-tight);
    }

    .meta {
      color: var(--wa-color-muted);
      font-size: var(--nc-font-size-meta);
    }

    .link {
      margin-left: auto;
      color: var(--wa-color-brand);
      font-size: var(--nc-font-size-meta);
      text-decoration: none;
      white-space: nowrap;
    }

    .link:hover {
      text-decoration: underline;
    }

    .link:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
      border-radius: var(--wa-radius-xs);
    }

    .bar {
      display: flex;
      gap: 3px;
      height: 10px;
      border-radius: var(--wa-radius-pill);
      background: var(--wa-color-border-soft);
      overflow: hidden;
    }

    .segment {
      min-width: 3px;
    }

    [data-bucket='0'] {
      --nc-aging-fill: var(--nc-color-aging-1);
    }

    [data-bucket='1'] {
      --nc-aging-fill: var(--nc-color-aging-2);
    }

    [data-bucket='2'] {
      --nc-aging-fill: var(--nc-color-aging-3);
    }

    [data-bucket='3'] {
      --nc-aging-fill: var(--nc-color-aging-4);
    }

    [data-bucket='4'] {
      --nc-aging-fill: var(--nc-color-aging-5);
    }

    .segment,
    .swatch {
      background: var(--nc-aging-fill);
    }

    dl {
      display: grid;
      grid-template-columns: repeat(5, minmax(0, 1fr));
      gap: 8px 12px;
      margin: 0;
    }

    @media (max-width: 40rem) {
      dl {
        grid-template-columns: repeat(2, minmax(0, 1fr));
      }
    }

    dt {
      display: flex;
      align-items: center;
      gap: 6px;
      color: var(--wa-color-muted);
      font-size: var(--nc-font-size-meta);
    }

    .swatch {
      flex: none;
      width: 8px;
      height: 8px;
      border-radius: 2px;
    }

    dd {
      margin: 2px 0 0;
      font-size: var(--nc-font-size-body);
    }

    dd[data-zero] {
      color: var(--wa-color-muted);
    }

    .empty {
      margin: 0;
      color: var(--wa-color-muted);
      font-size: var(--nc-font-size-control);
    }
  `;

  @property({ attribute: false })
  buckets: AgingBucketView[] = [];

  /** The outstanding total across the buckets. */
  @property({ type: Number })
  total = 0;

  /** `YYYY-MM-DD`, the day the report is about. */
  @property({ type: String, attribute: 'as-of' })
  asOf = '';

  /** Where "Aging report →" points. Empty hides the link. */
  @property({ type: String })
  href = '';

  @property({ type: String })
  currency = 'USD';

  render() {
    if (this.buckets.length === 0) {
      return html`<section class="card" aria-label="Outstanding">
        <p class="empty">No open receivables.</p>
      </section>`;
    }

    const open = this.buckets.reduce((acc, bucket) => acc + bucket.count, 0);
    const shares = agingShares(this.buckets);
    const meta = [
      `across ${open} open invoice${open === 1 ? '' : 's'}`,
      this.asOf ? `as of ${shortDate(this.asOf)}` : '',
    ]
      .filter(Boolean)
      .join(' · ');

    return html`
      <section class="card" aria-labelledby="heading">
        <div class="top">
          <h2 id="heading">Outstanding</h2>
          <wc-money
            class="amount"
            data-total
            .amount=${this.total}
            .currency=${this.currency}
            variant="plain"
          ></wc-money>
          <span class="meta" data-meta>${meta}</span>
          ${this.href
            ? html`<a class="link" href=${this.href}>Aging report →</a>`
            : nothing}
        </div>

        <div class="bar" aria-hidden="true">
          ${shares.map((share, index) =>
            share > 0
              ? html`<span
                  class="segment"
                  data-bucket=${index}
                  style="flex: ${share} 1 0"
                ></span>`
              : nothing,
          )}
        </div>

        <dl>
          ${this.buckets.map(
            (bucket, index) => html`
              <div data-legend=${bucket.label}>
                <dt>
                  <span class="swatch" data-bucket=${index} aria-hidden="true"></span>
                  ${agingBucketLabel(bucket.label)}
                </dt>
                <dd ?data-zero=${bucket.total === 0}>
                  <wc-money
                    .amount=${bucket.total}
                    .currency=${this.currency}
                    variant="plain"
                  ></wc-money>
                </dd>
              </div>
            `,
          )}
        </dl>
      </section>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-aging-bars': WcAgingBars;
  }
}
