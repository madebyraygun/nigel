import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-money.js';
import './wc-spinner.js';

/** One figure in the strip. */
export interface StatStripCell {
  /** Names the cell in a retry event, so a screen can reload just its source. */
  id: string;
  label: string;
  amount: number;
  /** A line under the figure — a period, a caveat. */
  hint?: string;
  loading?: boolean;
  error?: string;
}

/**
 * A row of headline figures in one card, divided by hairlines.
 *
 * Cells flow in a single row whatever their number, so adding a fourth is a
 * matter of appending to `cells`. Each cell carries its own loading and error
 * state because the figures can come from different requests, and a failed one
 * should not blank its neighbours. The figure takes its colour from its sign
 * through `wc-money`.
 */
@customElement('wc-stat-strip')
export class WcStatStrip extends LitElement {
  static styles = css`
    :host {
      display: block;
      background: var(--wa-color-surface);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-lg);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .cells {
      display: grid;
      grid-auto-flow: column;
      grid-auto-columns: minmax(0, 1fr);
    }

    .cell {
      min-width: 0;
      padding: 18px 22px;
    }

    .cell + .cell {
      border-left: 1px solid var(--wa-color-border-soft);
    }

    @media (max-width: 40rem) {
      .cells {
        grid-auto-flow: row;
        grid-auto-columns: auto;
      }

      .cell + .cell {
        border-left: none;
        border-top: 1px solid var(--wa-color-border-soft);
      }
    }

    .label,
    .hint {
      margin: 0;
      font-size: var(--nc-font-size-meta);
      color: var(--wa-color-muted);
    }

    .value {
      margin: 6px 0;
      font-size: var(--nc-font-size-display);
      font-weight: var(--wa-font-weight-medium);
      letter-spacing: var(--nc-letter-spacing-tight);
      line-height: 1.2;
      font-variant-numeric: tabular-nums;
    }

    .error {
      margin: 6px 0 0;
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
  `;

  @property({ attribute: false })
  cells: StatStripCell[] = [];

  private retry(cell: StatStripCell) {
    this.dispatchEvent(
      new CustomEvent('nc-retry', {
        detail: { id: cell.id },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private renderBody(cell: StatStripCell) {
    if (cell.loading) {
      return html`<div class="value">
        <wc-spinner label="Loading ${cell.label}"></wc-spinner>
      </div>`;
    }

    if (cell.error) {
      return html`
        <p class="error">${cell.error}</p>
        <button class="retry" type="button" @click=${() => this.retry(cell)}>
          Retry
        </button>
      `;
    }

    return html`
      <div class="value"><wc-money .amount=${cell.amount}></wc-money></div>
      ${cell.hint ? html`<p class="hint">${cell.hint}</p>` : nothing}
    `;
  }

  render() {
    return html`<div class="cells">
      ${this.cells.map(
        (cell) => html`
          <div class="cell">
            <p class="label">${cell.label}</p>
            ${this.renderBody(cell)}
          </div>
        `,
      )}
    </div>`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-stat-strip': WcStatStrip;
  }
}
