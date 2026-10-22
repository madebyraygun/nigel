import { LitElement, html, css } from 'lit';
import { customElement, property, query } from 'lit/decorators.js';
import { live } from 'lit/directives/live.js';
import '@awesome.me/webawesome/dist/components/switch/switch.js';
import { controlsCss } from '@nigel/theme';

export interface NcSwitchChangeDetail {
  checked: boolean;
}

/**
 * A setting that is on or off: a card with what it is, a line on what it
 * does, and a switch.
 *
 * **Fully controlled.** A flip emits `nc-switch-change` and the switch goes
 * straight back to `checked`; it only moves for good when the owner sets
 * `checked`. So an owner whose write fails just leaves `checked` alone, and
 * the switch cannot go on claiming a value nobody stored.
 */
@customElement('wc-switch-row')
export class WcSwitchRow extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: block;
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
      }

      .card {
        display: flex;
        align-items: center;
        gap: var(--wa-space-l);
        padding: 14px var(--wa-space-l);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-lg);
        background: var(--wa-color-surface);
      }

      .text {
        flex: 1;
        min-width: 0;
      }

      .label {
        margin: 0;
        font-size: var(--nc-font-size-body);
        font-weight: var(--wa-font-weight-medium);
      }

      .description {
        margin: 2px 0 0;
        font-size: var(--nc-font-size-meta);
        color: var(--wa-color-muted);
      }

      .visually-hidden {
        position: absolute;
        width: 1px;
        height: 1px;
        overflow: hidden;
        clip: rect(0, 0, 0, 0);
        white-space: nowrap;
      }
    `,
  ];

  @property({ type: String })
  label = '';

  @property({ type: String })
  description = '';

  @property({ type: Boolean, reflect: true })
  checked = false;

  @property({ type: Boolean, reflect: true })
  disabled = false;

  @query('wa-switch')
  private control?: HTMLElement & { checked: boolean };

  private onChange = () => {
    const wanted = this.control?.checked ?? !this.checked;
    if (this.control) this.control.checked = this.checked;
    this.dispatchEvent(
      new CustomEvent<NcSwitchChangeDetail>('nc-switch-change', {
        detail: { checked: wanted },
        bubbles: true,
        composed: true,
      }),
    );
  };

  render() {
    return html`
      <div class="card">
        <div class="text" aria-hidden="true">
          <p class="label">${this.label}</p>
          ${this.description ? html`<p class="description">${this.description}</p>` : ''}
        </div>
        <wa-switch
          .checked=${live(this.checked)}
          ?disabled=${this.disabled}
          @change=${this.onChange}
          ><span class="visually-hidden"
            >${this.label}${this.description ? `. ${this.description}` : ''}</span
          ></wa-switch
        >
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-switch-row': WcSwitchRow;
  }
  interface HTMLElementEventMap {
    'nc-switch-change': CustomEvent<NcSwitchChangeDetail>;
  }
}
