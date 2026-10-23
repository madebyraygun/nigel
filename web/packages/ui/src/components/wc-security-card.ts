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

  /**
   * Move focus to what acts on the database now: the Change… toggle when it
   * is encrypted, the status line when it is not. For an owner whose
   * operation just removed the form that had focus.
   */
  focusFirstAction(): void {
    const target = this.encrypted
      ? this.shadowRoot?.querySelector<HTMLElement>('.row wa-button')
      : this.shadowRoot?.querySelector<HTMLElement>('.status .title');
    target?.focus();
  }

  protected willUpdate(changed: Map<string, unknown>): void {
    if (!changed.has('panel')) return;
    const closing = changed.get('panel') as SecurityPanel | undefined;
    if (!closing || closing === this.panel) return;
    // A closed panel keeps nothing typed into it: a password left in a
    // hidden field is one somebody else can submit by opening it again. Every
    // form outside the panel now open is hidden, so every one is emptied.
    for (const form of this.querySelectorAll('[slot="change"], [slot="remove"]')) {
      if (form.slot !== this.panel) (form as HTMLElement & { reset?: () => void }).reset?.();
    }
    const forms = this.formsIn(closing);
    // Focus inside the panel would go with it; its toggle is where it was
    // opened from.
    if (forms.some((form) => form.matches(':focus-within'))) {
      this.refocus = closing;
    }
  }

  protected updated(): void {
    if (!this.refocus) return;
    const which = this.refocus;
    this.refocus = null;
    this.shadowRoot
      ?.querySelector<HTMLElement>(`wa-button[aria-controls="${which}-panel"]`)
      ?.focus();
  }

  private refocus: 'change' | 'remove' | null = null;

  private formsIn(which: 'change' | 'remove'): Element[] {
    return [...this.querySelectorAll(`[slot="${which}"]`)];
  }

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
            <p class="title" tabindex="-1">Encrypted</p>
            <p class="sub">
              Nigel asks for the password whenever it opens these books. If you lose it,
              the books cannot be opened.
            </p>
          </div>
        </div>`
      : html`<div class="card status">
          <span class="tile" aria-hidden="true"><wc-icon-lock-open></wc-icon-lock-open></span>
          <div>
            <p class="title" tabindex="-1">Not encrypted</p>
            <p class="sub">
              Anyone who can open the file can read your books. Set a password to lock
              them, and keep it somewhere safe.
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
              'You need the current password to change it.',
              'Change…',
            )}
            ${this.renderRow('remove', 'Remove password', 'Open these books without a password.', 'Remove…')}
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
