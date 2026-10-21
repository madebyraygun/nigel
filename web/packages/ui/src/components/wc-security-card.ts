import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import { controlsCss } from '@nigel/theme';
import '../icons/icons.js';

/** Which password panel is open on an encrypted database. */
export type SecurityPanel = 'change' | 'remove' | null;

export interface NcSecurityPanelDetail {
  panel: SecurityPanel;
}

/**
 * The database's encryption state and what can be done about it.
 *
 * A status card says whether the file is encrypted. Under it, an encrypted
 * database gets two rows — Change password and Remove password — each opening
 * its own panel inline, one at a time; a plaintext one gets the form that
 * encrypts it. The forms themselves are slotted (`change`, `remove`, `set`),
 * so the screen keeps the submit handling and the errors, and a closed
 * panel's form stays in the document with whatever it was last told.
 *
 * `panel` is set here when a row's button is pressed and announced with
 * `nc-security-panel`; the screen sets it back to `null` to close a panel
 * once its operation succeeds.
 */
@customElement('wc-security-card')
export class WcSecurityCard extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: grid;
        gap: var(--wa-space-m);
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
      }

      .card {
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-lg);
        background: var(--wa-color-surface);
      }

      .status {
        display: flex;
        align-items: center;
        gap: 14px;
        padding: 14px var(--wa-space-l);
      }

      :host(:not([encrypted])) .status {
        border-color: var(--nc-color-flagged);
      }

      .tile {
        display: grid;
        place-items: center;
        flex: none;
        width: 36px;
        height: 36px;
        border-radius: var(--nc-radius-tile);
        background: var(--nc-color-highlight);
        color: var(--wa-color-brand);
        --nc-icon-size: 18px;
      }

      :host(:not([encrypted])) .tile {
        background: var(--wa-color-surface-alt);
        color: var(--nc-color-flagged);
      }

      .title {
        margin: 0;
        font-size: var(--nc-font-size-card-title);
        font-weight: var(--wa-font-weight-semibold);
      }

      .sub {
        margin: 2px 0 0;
        font-size: var(--nc-font-size-meta);
        color: var(--wa-color-muted);
      }

      .row {
        display: flex;
        align-items: center;
        gap: var(--wa-space-m);
        padding: var(--wa-space-m) var(--wa-space-l);
      }

      .row + .panel,
      .panel + .row,
      .row + .row {
        border-top: 1px solid var(--wa-color-border-soft);
      }

      .row .text {
        flex: 1;
        min-width: 0;
      }

      .row-title {
        margin: 0;
        font-size: var(--nc-font-size-body);
        font-weight: var(--wa-font-weight-medium);
      }

      .panel {
        padding: var(--wa-space-m) var(--wa-space-l) var(--wa-space-l);
        background: var(--wa-color-surface-alt);
      }

      .panel:last-child {
        border-radius: 0 0 var(--wa-radius-lg) var(--wa-radius-lg);
      }

      .panel[hidden] {
        display: none;
      }

      .set {
        padding: var(--wa-space-l);
      }
    `,
  ];

  @property({ type: Boolean, reflect: true })
  encrypted = false;

  @property({ type: String })
  panel: SecurityPanel = null;

  /** An operation is in flight: the row buttons wait for it. */
  @property({ type: Boolean, reflect: true })
  busy = false;

  private toggle(which: 'change' | 'remove'): void {
    this.panel = this.panel === which ? null : which;
    this.dispatchEvent(
      new CustomEvent<NcSecurityPanelDetail>('nc-security-panel', {
        detail: { panel: this.panel },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private renderStatus() {
    return this.encrypted
      ? html`<div class="card status">
          <span class="tile" aria-hidden="true"><wc-icon-lock></wc-icon-lock></span>
          <div>
            <p class="title">Encrypted</p>
            <p class="sub">
              This database is encrypted with a password. Lose the password and the books
              cannot be opened.
            </p>
          </div>
        </div>`
      : html`<div class="card status">
          <span class="tile" aria-hidden="true"><wc-icon-lock-open></wc-icon-lock-open></span>
          <div>
            <p class="title">Not encrypted</p>
            <p class="sub">
              Anyone with access to the file can read it. Set a password and store it
              somewhere safe.
            </p>
          </div>
        </div>`;
  }

  private renderRow(
    which: 'change' | 'remove',
    title: string,
    sub: string,
    open: string,
  ) {
    const isOpen = this.panel === which;
    return html`
      <div class="row">
        <div class="text">
          <p class="row-title" id="${which}-title">${title}</p>
          <p class="sub">${sub}</p>
        </div>
        <wa-button
          size="small"
          appearance="outlined"
          variant=${which === 'remove' && !isOpen ? 'danger' : 'neutral'}
          aria-expanded=${isOpen ? 'true' : 'false'}
          aria-controls="${which}-panel"
          ?disabled=${this.busy}
          @click=${() => this.toggle(which)}
          >${isOpen ? 'Cancel' : open}</wa-button
        >
      </div>
      <div
        class="panel"
        id="${which}-panel"
        role="region"
        aria-labelledby="${which}-title"
        ?hidden=${!isOpen}
      >
        <slot name=${which}></slot>
      </div>
    `;
  }

  render() {
    return html`
      ${this.renderStatus()}
      ${this.encrypted
        ? html`<div class="card">
            ${this.renderRow(
              'change',
              'Change password',
              'You will need the current one.',
              'Change…',
            )}
            ${this.renderRow('remove', 'Remove password', 'Decrypts the database file.', 'Remove…')}
          </div>`
        : html`<div class="card set"><slot name="set"></slot></div>`}
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-security-card': WcSecurityCard;
  }
  interface HTMLElementEventMap {
    'nc-security-panel': CustomEvent<NcSecurityPanelDetail>;
  }
}
