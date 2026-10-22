import { LitElement, html, css, nothing } from 'lit';
import { controlsCss } from '@nigel/theme';
import '@awesome.me/webawesome/dist/components/button/button.js';
import { customElement, property } from 'lit/decorators.js';
import './wc-money.js';
import './wc-spinner.js';

/**
 * A single headline number: label, big money value, optional hint.
 *
 * The dashboard's year-to-date row is three of these. Loading and error are
 * states of the card rather than of the screen, because the four dashboard
 * fetches are independent — a failed balance query should not blank the P&L
 * beside it.
 */
@customElement('wc-stat-card')
export class WcStatCard extends LitElement {
  static styles = [controlsCss, css`
    :host {
      display: block;
      background: var(--wa-color-surface);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-lg);
      padding: var(--wa-space-l);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .label {
      margin: 0;
      font-size: var(--wa-font-size-s);
      font-weight: var(--wa-font-weight-medium);
      color: var(--wa-color-muted);
      text-transform: uppercase;
      letter-spacing: 0.04em;
    }

    .value {
      margin-top: var(--wa-space-xs);
      font-size: var(--wa-font-size-2xl);
      font-weight: var(--wa-font-weight-bold);
    }

    .hint {
      margin: var(--wa-space-2xs) 0 0;
      font-size: var(--wa-font-size-s);
      color: var(--wa-color-muted);
    }

    .error {
      margin: var(--wa-space-xs) 0 0;
      font-size: var(--wa-font-size-s);
      color: var(--wa-color-danger);
    }

    .retry {
      margin-top: var(--wa-space-xs);
    }
  `];

  @property({ type: String })
  label = '';

  @property({ type: Number })
  amount = 0;

  /** `plain` drops `wc-money`'s income/expense colouring. */
  @property({ type: String })
  variant: 'signed' | 'plain' = 'signed';

  /** A secondary line under the value — a period, a count, a caveat. */
  @property({ type: String })
  hint = '';

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
      return html`<div class="value">
        <wc-spinner label="Loading ${this.label}"></wc-spinner>
      </div>`;
    }

    if (this.error) {
      return html`
        <p class="error">${this.error}</p>
        <wa-button class="retry" size="s" @click=${this.handleRetry}>
          Retry
        </wa-button>
      `;
    }

    return html`
      <div class="value">
        <wc-money .amount=${this.amount} variant=${this.variant}></wc-money>
      </div>
      ${this.hint ? html`<p class="hint">${this.hint}</p>` : nothing}
    `;
  }

  render() {
    return html`<p class="label">${this.label}</p>
      ${this.renderBody()}`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-stat-card': WcStatCard;
  }
}
