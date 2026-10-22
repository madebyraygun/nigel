import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import { controlsCss } from '@nigel/theme';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '../icons/icons.js';

export type WcNoticeVariant = 'info' | 'success' | 'warning' | 'danger';

/**
 * A persistent banner: something is true and stays true until acted on.
 *
 * The distinction from `wc-toast` is duration, not looks. A toast reports that
 * something *happened* and leaves; this reports that something *is the case* —
 * an update is available, a build lacks a feature — and stays until the reader
 * deals with it. `role="status"` rather than `alert` for that reason: it is
 * worth announcing, not worth interrupting.
 */
@customElement('wc-notice-bar')
export class WcNoticeBar extends LitElement {
  static styles = [controlsCss, css`
    :host {
      display: block;
      --nc-notice-accent: var(--wa-color-info);
    }

    :host([variant='success']) {
      --nc-notice-accent: var(--wa-color-success);
    }

    :host([variant='warning']) {
      --nc-notice-accent: var(--wa-color-warning);
    }

    :host([variant='danger']) {
      --nc-notice-accent: var(--wa-color-danger);
    }

    .bar {
      display: flex;
      align-items: center;
      gap: var(--wa-space-s);
      padding: var(--wa-space-s) var(--wa-space-m);
      background: var(--wa-color-surface);
      border: 1px solid var(--wa-color-border);
      border-left: 4px solid var(--nc-notice-accent);
      border-radius: var(--wa-radius-md);
      font-family: var(--wa-font-family-sans);
      font-size: var(--wa-font-size-base);
      color: var(--wa-color-text);
    }

    .icon {
      color: var(--nc-notice-accent);
      display: inline-flex;
    }

    .message {
      flex: 1 1 auto;
      min-width: 0;
    }

    wa-button {
      flex: 0 0 auto;
    }
  `];

  @property({ type: String, reflect: true })
  variant: WcNoticeVariant = 'info';

  /** The text. Anything richer goes in the default slot instead. */
  @property({ type: String })
  message = '';

  /** Tag name of a `wc-icon-*` element, or empty for no icon. */
  @property({ type: String })
  icon = '';

  /** Renders an action button when set. */
  @property({ type: String, attribute: 'action-label' })
  actionLabel = '';

  @property({ type: Boolean, reflect: true })
  dismissible = false;

  private handleAction = () => {
    this.dispatchEvent(
      new CustomEvent('nc-notice-action', { bubbles: true, composed: true }),
    );
  };

  private handleDismiss = () => {
    this.dispatchEvent(
      new CustomEvent('nc-notice-dismiss', { bubbles: true, composed: true }),
    );
  };

  render() {
    return html`
      <div class="bar" role="status">
        ${this.icon
          ? html`<span class="icon">${document.createElement(this.icon)}</span>`
          : nothing}
        <span class="message">${this.message}<slot></slot></span>
        ${this.actionLabel
          ? html`<wa-button class="action" size="s" @click=${this.handleAction}>
              ${this.actionLabel}
            </wa-button>`
          : nothing}
        ${this.dismissible
          ? html`<wa-button
              class="dismiss"
              appearance="plain"
              @click=${this.handleDismiss}
            >
              <wc-icon-close label="Dismiss"></wc-icon-close>
            </wa-button>`
          : nothing}
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-notice-bar': WcNoticeBar;
  }
}
