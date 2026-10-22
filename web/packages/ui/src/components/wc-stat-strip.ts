import { LitElement, html, css, nothing } from 'lit';
import { controlsCss } from '@nigel/theme';
import '@awesome.me/webawesome/dist/components/button/button.js';
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
  static styles = [controlsCss, css`
    :host {
      display: block;
      background: var(--wa-color-surface);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-lg);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
      container-type: inline-size;
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

    /* The strip answers to its own width, not the viewport's: an open
       sidebar takes a fixed column from a wide window, and a 28px figure needs
       about 15rem a cell. Three cells fit from 46rem; a fourth needs 62rem. */
    @container (max-width: 46rem) {
      .cells {
        grid-auto-flow: row;
        grid-auto-columns: auto;
      }

      .cell + .cell {
        border-left: none;
        border-top: 1px solid var(--wa-color-border-soft);
      }
    }

    @container (max-width: 62rem) {
      .cells[data-count='4'],
      .cells[data-count='5'] {
        grid-auto-flow: row;
        grid-auto-columns: auto;
      }

      .cells[data-count='4'] .cell + .cell,
      .cells[data-count='5'] .cell + .cell {
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
    }
  `];

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
        <wa-button
          class="retry"
          size="s"
          @click=${() => this.retry(cell)}
        >
          Retry<span class="sr-only"> ${cell.label}</span>
        </wa-button>
      `;
    }

    return html`
      <div class="value"><wc-money .amount=${cell.amount}></wc-money></div>
      ${cell.hint ? html`<p class="hint">${cell.hint}</p>` : nothing}
    `;
  }

  render() {
    return html`<div class="cells" data-count=${this.cells.length}>
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
