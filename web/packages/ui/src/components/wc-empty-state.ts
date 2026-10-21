import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '../icons/icons.js';

/**
 * The "nothing here yet" panel: an empty result set, an unbuilt screen, a
 * filter that matched no rows.
 *
 * It centres its content in the box it is given, on both axes, and asks a
 * flex-column parent for the space left over. Screens are flex columns and
 * `wc-app-shell` stretches them to the whole content area, so an empty state
 * that is the only thing on a screen is centred in that area without the
 * screen saying anything about it. Where the box is content-sized — a table's
 * own empty row, a panel body — `flex-grow` finds nothing to take and the
 * centring is a no-op, which is why the same element serves both.
 */
@customElement('wc-empty-state')
export class WcEmptyState extends LitElement {
  static styles = css`
    :host {
      display: grid;
      place-items: center;
      align-content: center;
      flex: 1 1 auto;
      gap: var(--wa-space-s);
      padding: var(--wa-space-2xl);
      text-align: center;
      color: var(--wa-color-muted);
      font-family: var(--wa-font-family-sans);
    }

    :host([compact]) {
      padding: var(--wa-space-l);
    }

    .icon {
      --nc-icon-size: 32px;
      color: var(--wa-color-border);
    }

    .heading {
      font-size: var(--wa-font-size-lg);
      font-weight: var(--wa-font-weight-medium);
      color: var(--wa-color-text);
      margin: 0;
    }

    .message {
      margin: 0;
      max-width: 44ch;
    }

    .actions {
      margin-top: var(--wa-space-s);
      display: flex;
      gap: var(--wa-space-s);
    }
  `;

  @property({ type: String })
  heading = '';

  @property({ type: String })
  message = '';

  /** Tag name of a `wc-icon-*` element, e.g. "wc-icon-register". */
  @property({ type: String })
  icon = '';

  @property({ type: Boolean, reflect: true })
  compact = false;

  private renderIcon() {
    if (!this.icon) return nothing;
    // The icon names a custom element at runtime, so it is created
    // imperatively rather than through a static template tag.
    const el = document.createElement(this.icon);
    el.classList.add('icon');
    return el;
  }

  render() {
    return html`
      ${this.renderIcon()}
      ${this.heading ? html`<p class="heading">${this.heading}</p>` : nothing}
      ${this.message ? html`<p class="message">${this.message}</p>` : nothing}
      <slot></slot>
      <div class="actions"><slot name="actions"></slot></div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-empty-state': WcEmptyState;
  }
}
