import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import { controlsCss } from '@nigel/theme';
import '../icons/icons.js';

/**
 * The "N to review" pill that sits at the right of a page title. It is a link
 * to the review screen, outlined in the flag colour so the count reads as a
 * to-do rather than as another figure.
 */
@customElement('wc-review-pill')
export class WcReviewPill extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: inline-flex;
        flex: none;
      }

      a {
        display: inline-flex;
        align-items: center;
        gap: var(--wa-space-xs);
        height: 30px;
        padding: 0 14px 0 11px;
        box-sizing: border-box;
        border: 1px solid var(--nc-color-flagged);
        border-radius: var(--wa-radius-pill);
        background: transparent;
        color: var(--wa-color-text);
        font-family: var(--wa-font-family-sans);
        font-size: var(--nc-font-size-control);
        text-decoration: none;
        white-space: nowrap;
        cursor: default;
        transition: background var(--nc-transition-fast);
      }

      a:hover {
        background: var(--wa-color-surface-alt);
      }

      a:focus-visible {
        outline: 2px solid var(--wa-color-focus);
        outline-offset: 2px;
      }

      wc-icon-flag {
        --nc-icon-size: 15px;
        color: var(--nc-color-flagged);
      }

      .count {
        font-weight: var(--wa-font-weight-semibold);
        font-variant-numeric: tabular-nums;
        color: var(--nc-color-flagged);
      }
    `,
  ];

  @property({ type: Number })
  count = 0;

  @property({ type: String })
  href = '#/review';

  render() {
    return html`<a href=${this.href}>
      <wc-icon-flag></wc-icon-flag>
      <span class="count">${this.count}</span>
      <span>to review</span>
    </a>`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-review-pill': WcReviewPill;
  }
}
