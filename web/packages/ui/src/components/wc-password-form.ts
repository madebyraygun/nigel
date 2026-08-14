import { LitElement, html, css, nothing } from 'lit';
import { customElement, property, state, query } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import { controlsCss } from '@nigel/theme';

export type WcPasswordMode = 'set' | 'change' | 'remove';

/** What the form hands back. Only the fields its mode actually collects. */
export interface NcPasswordSubmitDetail {
  mode: WcPasswordMode;
  currentPassword?: string;
  newPassword?: string;
}

const SUBMIT_LABELS: Record<WcPasswordMode, string> = {
  set: 'Encrypt database',
  change: 'Change password',
  remove: 'Remove password',
};

const HEADINGS: Record<WcPasswordMode, string> = {
  set: 'Encrypt this database',
  change: 'Change password',
  remove: 'Remove password',
};

const DESCRIPTIONS: Record<WcPasswordMode, string> = {
  set: 'Choose a password. Nigel asks for it every time it opens these books.',
  change: 'Enter the password in use now, then the one that replaces it.',
  remove:
    'Decrypts the database. Anyone who can read the file can then read the books.',
};

/**
 * One password operation — set, change, or remove — as a named group.
 *
 * "New password plus confirmation, with a mismatch message" is visual behavior,
 * so it lives here rather than being retyped into every screen that needs it.
 * The confirmation value never leaves the component — it exists to catch a typo,
 * and the server has no use for it.
 *
 * The operation is a `fieldset` under a `legend`, because an encrypted database
 * puts change and remove on screen together and each collects a field called
 * "Current password". Positional order is the only thing that tells them apart
 * in a flat stack, and positional order is not something a screen reader
 * conveys — the legend names the owner of every field inside it. The heading
 * sits inside the legend rather than above it, which `legend`'s content model
 * allows, so the operation is one thing to both heading navigation and form
 * grouping instead of two elements that have to be kept in step.
 */
@customElement('wc-password-form')
export class WcPasswordForm extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: block;
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
      }

      form {
        max-width: 24rem;
      }

      fieldset {
        display: grid;
        gap: var(--wa-space-m, 12px);
        margin: 0;
        padding: var(--wa-space-m, 12px);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-m, 8px);
      }

      legend {
        padding: 0 var(--wa-space-2xs, 4px);
      }

      legend h3 {
        margin: 0;
        font-size: var(--wa-font-size-m, 14px);
        font-weight: var(--wa-font-weight-semibold, 600);
      }

      .description {
        margin: 0;
        color: var(--wa-color-muted);
        font-size: var(--wa-font-size-s, 13px);
        max-width: 68ch;
      }

      :host([mode='remove']) fieldset {
        border-color: var(--wa-color-danger);
      }

      :host([mode='remove']) legend h3,
      :host([mode='remove']) .description {
        color: var(--wa-color-danger);
      }

      .message {
        margin: 0;
        font-size: var(--wa-font-size-s, 13px);
        min-height: 1.25rem;
      }

      .error {
        color: var(--wa-color-danger);
      }

      .actions {
        display: flex;
        gap: var(--wa-space-s, 8px);
      }
    `,
  ];

  @property({ type: String, reflect: true })
  mode: WcPasswordMode = 'set';

  /** Overrides the operation's own name. Empty means the name for its mode. */
  @property({ type: String })
  heading = '';

  /** Overrides the sentence under the name. Empty means the one for its mode. */
  @property({ type: String })
  description = '';

  @property({ type: Boolean, reflect: true })
  busy = false;

  /** A failure from the server, shown alongside anything found locally. */
  @property({ type: String })
  error = '';

  /** Validation the form found itself, cleared on every new attempt. */
  @state()
  private localError = '';

  @query('[data-current]')
  private currentInput?: HTMLElement & { value: string };

  @query('[data-new]')
  private newInput?: HTMLElement & { value: string };

  @query('[data-confirm]')
  private confirmInput?: HTMLElement & { value: string };

  private get needsCurrent(): boolean {
    return this.mode !== 'set';
  }

  private get needsNew(): boolean {
    return this.mode !== 'remove';
  }

  private clearFields(): void {
    for (const field of [this.currentInput, this.newInput, this.confirmInput]) {
      if (field) field.value = '';
    }
  }

  private handleSubmit = (event: Event) => {
    event.preventDefault();
    if (this.busy) return;

    this.localError = '';
    const currentPassword = this.currentInput?.value ?? '';
    const newPassword = this.newInput?.value ?? '';
    const confirmation = this.confirmInput?.value ?? '';

    if (this.needsCurrent && currentPassword.length === 0) {
      this.localError = 'Enter the current password.';
      return;
    }
    if (this.needsNew) {
      if (newPassword.trim().length === 0) {
        // The wording the terminal prompt uses, so the two agree.
        this.localError = 'Password cannot be empty.';
        return;
      }
      if (newPassword !== confirmation) {
        this.localError = 'Passwords do not match.';
        return;
      }
    }

    const detail: NcPasswordSubmitDetail = { mode: this.mode };
    if (this.needsCurrent) detail.currentPassword = currentPassword;
    if (this.needsNew) detail.newPassword = newPassword;

    this.clearFields();
    this.dispatchEvent(
      new CustomEvent<NcPasswordSubmitDetail>('nc-password-submit', {
        detail,
        bubbles: true,
        composed: true,
      }),
    );
  };

  render() {
    const message = this.localError || this.error;
    return html`
      <form @submit=${this.handleSubmit}>
        <fieldset>
          <legend><h3>${this.heading || HEADINGS[this.mode]}</h3></legend>
          <p class="description">${this.description || DESCRIPTIONS[this.mode]}</p>
          ${this.needsCurrent
            ? html`<wa-input
                data-current
                type="password"
                label="Current password"
                autocomplete="current-password"
                ?disabled=${this.busy}
              ></wa-input>`
            : nothing}
          ${this.needsNew
            ? html`
                <wa-input
                  data-new
                  type="password"
                  label="New password"
                  autocomplete="new-password"
                  ?disabled=${this.busy}
                ></wa-input>
                <wa-input
                  data-confirm
                  type="password"
                  label="Confirm new password"
                  autocomplete="new-password"
                  ?disabled=${this.busy}
                ></wa-input>
              `
            : nothing}
          <p class="message ${message ? 'error' : ''}" role="status" aria-live="polite">
            ${message}
          </p>
          <div class="actions">
            <wa-button
              type="submit"
              variant=${this.mode === 'remove' ? 'danger' : 'brand'}
              ?disabled=${this.busy}
            >
              ${this.busy ? 'Working…' : SUBMIT_LABELS[this.mode]}
            </wa-button>
          </div>
        </fieldset>
      </form>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-password-form': WcPasswordForm;
  }
}
