import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';

/**
 * A muted marker beside a row's name — "Archived" on a client, and whatever a
 * later list needs to say about a row without spending a column on it.
 *
 * A word, never a colour or an icon alone: a fixed-width list has no room for
 * a column that is empty on nearly every row, and colour cannot be the only
 * channel (WCAG 1.4.1), which is why `wc-invoice-status` prints its word too.
 */
@customElement('wc-row-badge')
export class WcRowBadge extends LitElement {
  static styles = css`
    :host {
      display: inline-block;
    }

    .badge {
      display: inline-block;
      padding: 0 var(--wa-space-xs, 6px);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-pill, 999px);
      font-family: var(--wa-font-family-sans);
      font-size: var(--wa-font-size-xs, 11px);
      line-height: 1.7;
      white-space: nowrap;
      color: var(--wa-color-muted);
    }
  `;

  /** The word. An empty label renders nothing at all. */
  @property({ type: String })
  label = '';

  render() {
    if (this.label.trim() === '') return html``;
    return html`<span class="badge" part="badge">${this.label}</span>`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-row-badge': WcRowBadge;
  }
}
