import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import { controlsCss } from '@nigel/theme';

/**
 * The bar that holds unsaved edits: a flag dot, what is unsaved, Discard and
 * Save. It sticks to the bottom of the scrolling area, so the way out of an
 * edit stays in reach wherever the reader has scrolled to.
 *
 * It saves nothing itself. Discard asks with `nc-save-bar-discard` and Save
 * with `nc-save-bar-save`; the screen that owns the draft answers both and
 * stops rendering the bar when nothing differs from what is stored.
 */
@customElement('wc-save-bar')
export class WcSaveBar extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: block;
        position: sticky;
        bottom: var(--wa-space-m);
        z-index: 2;
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
      }

      .bar {
        display: flex;
        align-items: center;
        flex-wrap: wrap;
        gap: var(--wa-space-m);
        padding: var(--wa-space-s) var(--wa-space-s) var(--wa-space-s) var(--wa-space-l);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-pill);
        background: var(--wa-color-surface);
        box-shadow: var(--wa-shadow-lg);
      }

      .dot {
        flex: none;
        width: 8px;
        height: 8px;
        border-radius: 50%;
        background: var(--nc-color-flagged);
      }

      .message {
        flex: 1;
        min-width: 10rem;
        font-size: var(--nc-font-size-control);
      }

      .actions {
        display: flex;
        gap: var(--wa-space-s);
      }

      @media print {
        :host {
          display: none;
        }
      }
    `,
  ];

  @property({ type: String })
  message = 'Unsaved changes';

  /** A save is in flight: both buttons wait for it. */
  @property({ type: Boolean, reflect: true })
  busy = false;

  private emit(name: 'nc-save-bar-discard' | 'nc-save-bar-save'): void {
    this.dispatchEvent(new CustomEvent(name, { bubbles: true, composed: true }));
  }

  render() {
    return html`
      <div class="bar" role="region" aria-label="Unsaved changes">
        <span class="dot" aria-hidden="true"></span>
        <span class="message" role="status">${this.message}</span>
        <span class="actions">
          <wa-button
            size="small"
            appearance="outlined"
            ?disabled=${this.busy}
            @click=${() => this.emit('nc-save-bar-discard')}
            >Discard</wa-button
          >
          <wa-button
            size="small"
            variant="brand"
            ?disabled=${this.busy}
            @click=${() => this.emit('nc-save-bar-save')}
            >${this.busy ? 'Saving…' : 'Save'}</wa-button
          >
        </span>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-save-bar': WcSaveBar;
  }
  interface HTMLElementEventMap {
    'nc-save-bar-discard': CustomEvent<void>;
    'nc-save-bar-save': CustomEvent<void>;
  }
}
