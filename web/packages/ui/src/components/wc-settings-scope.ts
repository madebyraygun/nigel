import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-monogram.js';
import '../icons/icons.js';

/**
 * The header that opens a group of settings and says what they apply to:
 * `book` shows the open book's monogram and name, `device` a monitor tile
 * and what a device setting reaches.
 */
@customElement('wc-settings-scope')
export class WcSettingsScope extends LitElement {
  static styles = css`
    :host {
      display: block;
      padding-bottom: 14px;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    h2 {
      display: flex;
      align-items: center;
      gap: var(--wa-space-m);
      margin: 0;
      font-size: inherit;
      font-weight: inherit;
    }

    wc-monogram {
      --nc-monogram-size: 28px;
    }

    .tile {
      display: inline-grid;
      place-items: center;
      flex: none;
      width: 28px;
      height: 28px;
      border-radius: var(--wa-radius-sm);
      background: var(--nc-color-pastel-5);
      color: var(--nc-color-on-gradient);
      --nc-icon-size: 16px;
    }

    .text {
      display: grid;
      min-width: 0;
    }

    .label {
      font-size: 11px;
      font-weight: var(--wa-font-weight-medium);
      letter-spacing: var(--nc-letter-spacing-label);
      text-transform: uppercase;
      color: var(--wa-color-muted);
    }

    .title {
      font-size: 14px;
      font-weight: var(--wa-font-weight-medium);
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
  `;

  @property({ type: String, reflect: true })
  kind: 'book' | 'device' = 'book';

  /** The small uppercase line: "This book", "This device". */
  @property({ type: String })
  label = '';

  /** The line under it: the book's name, or what a device setting reaches. */
  @property({ type: String })
  heading = '';

  /** What picks the book's monogram colour; see `wc-monogram`. */
  @property({ type: String })
  seed = '';

  render() {
    const tile =
      this.kind === 'book'
        ? html`<wc-monogram name=${this.heading} seed=${this.seed}></wc-monogram>`
        : html`<span class="tile" aria-hidden="true"><wc-icon-monitor></wc-icon-monitor></span>`;
    return html`
      <h2>
        ${tile}
        <span class="text">
          <span class="label">${this.label}</span>
          <span class="title">${this.heading}</span>
        </span>
      </h2>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-settings-scope': WcSettingsScope;
  }
}
