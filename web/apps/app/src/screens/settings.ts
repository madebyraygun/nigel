import { LitElement, html, css, nothing, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/textarea/textarea.js';
import '@awesome.me/webawesome/dist/components/switch/switch.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '@nigel/ui';
import {
  confirmDialog,
  dispatchNcToast,
  type NcPasswordSubmitDetail,
  type WcPasswordMode,
} from '@nigel/ui';

import { SignalWatcher } from '../mixins/signal-watcher.js';
import { ApiError, type ApiClient } from '../api/index.js';
import { getAppStore, type AppStore } from '../state/app-store.js';
import type { AppSettings, Company } from '../api/types.js';

/**
 * The cap `document::parse_logo` enforces, in the one place the browser needs
 * it: `MAX_LOGO_BYTES` in `crates/nigel-core/src/invoicing/document.rs`.
 */
const MAX_LOGO_BYTES = 128 * 1024;
import type { ScreenContext } from './context.js';
import {
  applyMode,
  controlsCss,
  darkModeQuery,
  readMode,
  resolveMode,
  writeMode,
  type ColorMode,
  type ResolvedMode,
} from '@nigel/theme';

interface PasswordFailure {
  mode: WcPasswordMode;
  message: string;
}

/**
 * What a failure says when the server did not say anything usable. One
 * sentence per operation: an encrypted database shows change and remove
 * together, so a single "Could not change the password." would put the wrong
 * verb under the Remove form.
 */
const PASSWORD_FAILURE: Record<WcPasswordMode, string> = {
  set: 'Could not encrypt the database.',
  change: 'Could not change the password.',
  remove: 'Could not remove the password.',
};

/**
 * Settings: business name, auto-update check, data directory, and the database
 * password — what `settings_manager.rs` covers, plus `nigel load`.
 *
 * A screen with state is a custom element under `screens/`; visual primitives
 * still live in `@nigel/ui`. This is the first of them, and the convention the
 * remaining screen tasks follow.
 */
@customElement('nigel-settings-screen')
export class NigelSettingsScreen extends SignalWatcher(LitElement) {
  static styles = [
    controlsCss,
    css`
      :host {
        display: flex;
        flex-direction: column;
        gap: var(--wa-space-l, 16px);
        padding: var(--wa-space-l, 16px);
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
      }

      /* The reading width belongs to what is being read. The screen itself is
         the whole content area, so an empty state on it centres in the area
         rather than in a column down its left. */
      wc-panel {
        max-width: 48rem;
      }

      .row {
        display: flex;
        gap: var(--wa-space-s, 8px);
        align-items: flex-end;
        flex-wrap: wrap;
      }

      .row wa-input {
        flex: 1 1 18rem;
      }

      .path {
        font-family: var(--wa-font-family-mono, monospace);
        font-size: var(--wa-font-size-s, 13px);
        background: var(--wa-color-surface-alt);
        border-radius: var(--wa-radius-s, 6px);
        padding: var(--wa-space-2xs, 4px) var(--wa-space-xs, 6px);
        overflow-wrap: anywhere;
      }

      .note {
        color: var(--wa-color-muted);
        font-size: var(--wa-font-size-s, 13px);
        margin: var(--wa-space-xs, 6px) 0 0;
      }

      .error {
        color: var(--wa-color-danger);
        font-size: var(--wa-font-size-s, 13px);
        margin: var(--wa-space-xs, 6px) 0 0;
      }

      wa-textarea {
        display: block;
        margin-top: var(--wa-space-s, 8px);
      }

      .operations {
        display: grid;
        gap: var(--wa-space-l, 16px);
      }

      .logo-field {
        display: grid;
        gap: var(--wa-space-2xs, 4px);
        font-size: var(--wa-font-size-s, 13px);
      }

      .logo-preview {
        max-height: 3rem;
        max-width: 12rem;
        /* The stored image may be transparent, and both documents flatten it
           onto white. Showing it on the card would misrepresent it in dark
           mode, which is why this token is deliberately mode-independent. */
        background: var(--nc-color-document-bg);
        border-radius: var(--wa-radius-s, 6px);
        padding: var(--wa-space-2xs, 4px);
      }
    `,
  ];

  /** Supplied by the registry from the screen context. */
  @property({ attribute: false })
  client!: ApiClient;

  @state() private appSettings: AppSettings | null = null;
  @state() private company: Company | null = null;
  @state() private companyError = '';
  /** A failed *load*, which is a different state from a failed save. */
  @state() private companyLoadError = '';
  @state() private dataDirDraft = '';
  @state() private busy: string | null = null;
  /**
   * A failed password operation and which operation it was. An encrypted
   * database has two password forms on screen, both collecting a field called
   * "Current password", so a message without its operation lands under
   * whichever form happens to be first. The two travel as one object because
   * a message filed against nothing is not a state worth being able to write.
   */
  @state() private passwordFailure: PasswordFailure | null = null;
  @state() private dataDirError = '';
  @state() private colorMode: ColorMode = 'system';
  @state() private resolvedMode: ResolvedMode = 'light';

  private store: AppStore = getAppStore();

  /**
   * Only there to keep the "currently dark" hint honest while System is
   * selected. The colours themselves need no listener — System writes no
   * class and the CSS media query does the tracking.
   */
  private darkQuery?: MediaQueryList;
  private readonly onSystemChange = () => {
    this.resolvedMode = resolveMode(this.colorMode, this.darkQuery);
  };

  connectedCallback(): void {
    super.connectedCallback();
    void this.loadAppSettings();
    void this.loadCompany();

    this.colorMode = readMode();
    this.darkQuery = darkModeQuery();
    this.resolvedMode = resolveMode(this.colorMode, this.darkQuery);
    this.darkQuery?.addEventListener('change', this.onSystemChange);
  }

  disconnectedCallback(): void {
    this.darkQuery?.removeEventListener('change', this.onSystemChange);
    super.disconnectedCallback();
  }

  private handleColorMode(event: CustomEvent<{ mode: ColorMode }>): void {
    const mode = event.detail.mode;
    this.colorMode = mode;
    // The container owns persistence; the switcher stays controlled. Nothing
    // is sent to the server: every /api/settings/* route is behind the locked
    // guard, so a server-stored mode could not be honoured on the unlock
    // screen, and the preference is per-browser by nature anyway.
    writeMode(mode);
    applyMode(mode);
    this.resolvedMode = resolveMode(mode, this.darkQuery);
  }

  private async loadAppSettings(): Promise<void> {
    try {
      this.appSettings = await this.client.getAppSettings();
    } catch (error) {
      this.toastError(error, 'Could not load settings.');
    }
  }

  private toastError(error: unknown, fallback: string): void {
    const message = error instanceof ApiError ? error.message : fallback;
    dispatchNcToast(this, { message, variant: 'danger' });
  }

  private toastOk(message: string): void {
    dispatchNcToast(this, { message, variant: 'success' });
  }

  // -- the letterhead -------------------------------------------------------

  /**
   * Loaded once and then edited in place. The draft is the loaded object, not a
   * projection of the store, so nothing re-seeds it under someone who is
   * halfway through typing an address.
   */
  private async loadCompany(): Promise<void> {
    this.companyLoadError = '';
    try {
      this.company = await this.client.getCompany();
    } catch (error) {
      // A failed load is its own state, never a form the data could not fill:
      // an empty letterhead form would save five blank fields over whatever is
      // actually stored.
      this.companyLoadError =
        error instanceof ApiError ? error.message : 'Could not load the letterhead.';
      this.toastError(error, 'Could not load the letterhead.');
    }
  }

  private retryCompany = () => {
    void this.loadCompany();
  };

  private editCompany(field: keyof Company, value: string): void {
    if (!this.company) return;
    this.company = { ...this.company, [field]: value };
  }

  /**
   * One bound handler per field, built once. A factory called from `render`
   * hands Lit a new function on every pass, so the listener is torn down and
   * re-attached each time for no gain.
   */
  private readonly companyInput: Record<keyof Company, (event: Event) => void> = {
    name: this.onCompanyInput('name'),
    address: this.onCompanyInput('address'),
    phone: this.onCompanyInput('phone'),
    logo: this.onCompanyInput('logo'),
    paymentInstructions: this.onCompanyInput('paymentInstructions'),
  };

  private onCompanyInput(field: keyof Company): (event: Event) => void {
    return (event: Event) =>
      this.editCompany(field, (event.target as HTMLInputElement).value);
  }

  /**
   * The logo travels as a `data:` URI, so the file is read here.
   *
   * The size is checked **before** anything is read. Base64 inflates the
   * payload by a third, so an oversized image would arrive as a body axum
   * refuses outright — a generic 413 in place of the server's own sentence
   * about the 128 KiB cap. `wc-dropzone` checks extension and size client-side
   * for exactly this reason. Everything else about the file — the magic bytes,
   * the type actually inside it, whether it decodes — is still the server's to
   * answer, because only it can.
   */
  private handleLogoFile = async (event: Event) => {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    if (file.size > MAX_LOGO_BYTES) {
      this.companyError = `${file.name} is ${Math.ceil(file.size / 1024)} KiB; the limit is ${MAX_LOGO_BYTES / 1024} KiB.`;
      input.value = '';
      return;
    }
    try {
      const dataUri = await new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => resolve(String(reader.result));
        reader.onerror = () => reject(reader.error ?? new Error('unreadable'));
        reader.readAsDataURL(file);
      });
      this.editCompany('logo', dataUri);
      this.companyError = '';
    } catch {
      this.companyError = `Could not read ${file.name}.`;
    } finally {
      // So choosing the same file twice still fires a change event.
      input.value = '';
    }
  };

  private clearLogo = () => {
    this.editCompany('logo', '');
  };

  private saveCompany = async () => {
    if (!this.company) return;
    this.companyError = '';
    this.busy = 'company';
    try {
      // The response is the stored values, trimmed — so the form shows what
      // was actually saved rather than what was typed.
      this.company = await this.client.setCompany(this.company);
      // The sidebar and the document title read the name from status.
      await this.store.refreshStatus();
      this.toastOk('Letterhead saved.');
    } catch (error) {
      // Kept beside the fields rather than only in a toast: a refused logo is
      // about one control, and the form must not be cleared under it.
      this.companyError =
        error instanceof ApiError ? error.message : 'Could not save the letterhead.';
      this.toastError(error, 'Could not save the letterhead.');
    } finally {
      this.busy = null;
    }
  };

  // -- update check ---------------------------------------------------------

  private toggleUpdateCheck = async (event: Event) => {
    const control = event.target as HTMLElement & { checked: boolean };
    const wanted = control.checked;
    const previous = this.appSettings;
    this.busy = 'update';
    try {
      this.appSettings = await this.client.updateAppSettings({ updateCheck: wanted });
    } catch (error) {
      // Put the switch back where the server still has it: a control whose
      // write failed must not keep claiming it succeeded.
      //
      // The control is reset directly rather than by re-rendering the old
      // value. The user moved the DOM property out from under lit, so lit's
      // dirty check sees the value it already committed and skips the update,
      // leaving the switch showing a state nobody saved.
      this.appSettings = previous;
      control.checked = previous?.updateCheck ?? true;
      this.toastError(error, 'Could not save the update setting.');
    } finally {
      this.busy = null;
    }
  };

  // -- data directory -------------------------------------------------------

  private handleDataDirInput = (event: Event) => {
    this.dataDirDraft = (event.target as HTMLInputElement).value;
  };

  private switchDataDir = async () => {
    const path = this.dataDirDraft.trim();
    if (path.length === 0) return;

    this.dataDirError = '';
    const confirmed = await confirmDialog({
      heading: 'Switch data directory?',
      message: `Nigel will reload and open the books in ${path}.`,
      confirmLabel: 'Switch',
    });
    if (!confirmed) return;

    this.busy = 'data-dir';
    const outcome = await this.store.switchDataDir(path);
    this.busy = null;
    if (!outcome.ok) this.dataDirError = outcome.message;
  };

  // -- password -------------------------------------------------------------

  private handlePasswordSubmit = async (event: CustomEvent<NcPasswordSubmitDetail>) => {
    const detail = event.detail;
    if (detail.mode === 'remove') {
      const confirmed = await confirmDialog({
        heading: 'Remove the database password?',
        message:
          'The database will be decrypted and readable by anyone with access to the file.',
        confirmLabel: 'Remove password',
        variant: 'danger',
      });
      if (!confirmed) return;
    }

    this.passwordFailure = null;
    this.busy = 'password';
    try {
      if (detail.mode === 'set') {
        await this.client.setPassword({ newPassword: detail.newPassword ?? '' });
        this.toastOk('Database encrypted.');
      } else if (detail.mode === 'change') {
        await this.client.changePassword({
          currentPassword: detail.currentPassword ?? '',
          newPassword: detail.newPassword ?? '',
        });
        this.toastOk('Password changed.');
      } else {
        await this.client.removePassword({
          currentPassword: detail.currentPassword ?? '',
        });
        this.toastOk('Password removed.');
      }
      // Re-read rather than assume: the encryption state the next render draws
      // comes from the server, never from an optimistic local flag.
      await this.store.refreshStatus();
    } catch (error) {
      this.passwordFailure = {
        mode: detail.mode,
        message:
          error instanceof ApiError ? error.message : PASSWORD_FAILURE[detail.mode],
      };
    } finally {
      this.busy = null;
    }
  };

  /**
   * Which of the forms on screen carries the failure. Normally the operation
   * that produced it; if that operation is no longer rendered — another
   * session encrypted or decrypted these books while the message was up, and
   * `refreshStatus` swapped the forms underneath it — it falls back to the
   * first one, because a message that is stale is still one somebody can read
   * and a message filed against a form that does not exist is one nobody can.
   */
  private failureSlot(rendered: readonly WcPasswordMode[]): WcPasswordMode | null {
    const failure = this.passwordFailure;
    if (!failure) return null;
    return rendered.includes(failure.mode) ? failure.mode : (rendered[0] ?? null);
  }

  private errorAt(mode: WcPasswordMode, slot: WcPasswordMode | null): string {
    return slot === mode ? (this.passwordFailure?.message ?? '') : '';
  }

  /**
   * The three states this panel has: loaded, loading, and could-not-load. The
   * last one is not a form — an empty letterhead over a load failure would save
   * five blank fields on top of whatever is really stored.
   */
  private renderCompanyBody(nameLabel: string): TemplateResult {
    if (this.companyLoadError) {
      return html`<p class="error">${this.companyLoadError}</p>`;
    }
    if (!this.company) {
      return html`<wc-spinner label="Loading the letterhead"></wc-spinner>`;
    }
    return this.renderCompanyFields(this.company, nameLabel);
  }

  private renderCompanyFields(company: Company, nameLabel: string): TemplateResult {
    const disabled = this.busy === 'company';
    return html`
      <div class="row">
        <wa-input
          label=${nameLabel}
          .value=${company.name}
          ?disabled=${disabled}
          @input=${this.companyInput.name}
        ></wa-input>
        <wa-input
          label="Phone"
          .value=${company.phone}
          ?disabled=${disabled}
          @input=${this.companyInput.phone}
        ></wa-input>
      </div>
      <wa-textarea
        label="Address"
        rows="3"
        .value=${company.address}
        ?disabled=${disabled}
        @input=${this.companyInput.address}
      ></wa-textarea>
      <wa-textarea
        label="Payment instructions"
        rows="3"
        .value=${company.paymentInstructions}
        ?disabled=${disabled}
        @input=${this.companyInput.paymentInstructions}
      ></wa-textarea>
      <p class="note">
        Printed on both the invoice page and the PDF, or on neither. Leave it
        empty if you do not take bank transfers.
      </p>
      <div class="row">
        <label class="logo-field">
          Logo
          <input
            type="file"
            accept="image/png,image/jpeg"
            ?disabled=${disabled}
            @change=${this.handleLogoFile}
          />
        </label>
        ${company.logo
          ? html`<img class="logo-preview" src=${company.logo} alt="The configured logo" />
              <wa-button ?disabled=${disabled} @click=${this.clearLogo}>Remove</wa-button>`
          : nothing}
      </div>
      <p class="note">
        PNG or JPEG, up to 128 KiB. It is embedded in the PDF and inlined in the
        page; Gmail does not render inlined images, so a Gmail reader sees your
        name in the email body and the logo on the attachment.
      </p>
    `;
  }

  render() {
    const status = this.store.status.get();
    const encrypted = status?.encrypted ?? false;
    // Same metadata field either way; only the label follows the books profile.
    const nameLabel = status?.profile === 'personal' ? 'Household name' : 'Business name';
    const operations: readonly [WcPasswordMode, ...WcPasswordMode[]] = encrypted
      ? ['change', 'remove']
      : ['set'];
    const failureSlot = this.failureSlot(operations);

    return html`
      <wc-panel
        heading="Letterhead"
        description="What the invoice page and the PDF say about you. The name is also shown in the sidebar, on reports, and in the browser tab."
      >
        ${this.renderCompanyBody(nameLabel)}
        ${this.companyError ? html`<p class="error">${this.companyError}</p>` : nothing}
        ${this.companyLoadError
          ? html`<wa-button slot="actions" variant="brand" @click=${this.retryCompany}
              >Retry</wa-button
            >`
          : html`<wa-button
              slot="actions"
              variant="brand"
              ?disabled=${this.busy === 'company' || this.company === null}
              @click=${this.saveCompany}
              >Save</wa-button
            >`}
      </wc-panel>

      <wc-panel
        heading="Appearance"
        description="Light, dark, or whatever this device is set to. Saved in this browser only — a laptop and a desktop on the same books can differ."
      >
        <wc-mode-switcher
          .mode=${this.colorMode}
          .resolved=${this.resolvedMode}
          @nc-color-mode-change=${this.handleColorMode}
        ></wc-mode-switcher>
      </wc-panel>

      <wc-panel
        heading="Updates"
        description="Check GitHub for a newer version once a day when nigel starts."
      >
        ${this.appSettings
          ? html`<wa-switch
              ?checked=${this.appSettings.updateCheck}
              ?disabled=${this.busy === 'update'}
              @change=${this.toggleUpdateCheck}
              >Check for updates automatically</wa-switch
            >`
          : html`<wc-spinner label="Loading settings"></wc-spinner>`}
      </wc-panel>

      <wc-panel
        heading="Data directory"
        description="Where this set of books lives. Switching reloads nigel onto the other database."
      >
        <p class="path">${status?.dataDir ?? ''}</p>
        <div class="row">
          <wa-input
            label="Switch to"
            placeholder="~/Documents/other-books"
            .value=${this.dataDirDraft}
            ?disabled=${this.busy === 'data-dir'}
            @input=${this.handleDataDirInput}
          ></wa-input>
        </div>
        ${this.dataDirError
          ? html`<p class="error">${this.dataDirError}</p>`
          : nothing}
        <wa-button
          slot="actions"
          ?disabled=${this.busy === 'data-dir'}
          @click=${this.switchDataDir}
          >Switch</wa-button
        >
      </wc-panel>

      <wc-panel
        heading="Database password"
        description=${encrypted
          ? 'This database is encrypted. You can change the password, or remove it altogether.'
          : 'Warning, this database is not encrypted, anyone with access to the file can read it. It is strongly recommended that you set a password and store it in a safe place.'}
      >
        <div class="operations">
          <wc-password-form
            mode=${operations[0]}
            ?busy=${this.busy === 'password'}
            error=${this.errorAt(operations[0], failureSlot)}
            @nc-password-submit=${this.handlePasswordSubmit}
          ></wc-password-form>
          ${encrypted
            ? html`<wc-password-form
                mode="remove"
                ?busy=${this.busy === 'password'}
                error=${this.errorAt('remove', failureSlot)}
                @nc-password-submit=${this.handlePasswordSubmit}
              ></wc-password-form>`
            : nothing}
        </div>
      </wc-panel>
    `;
  }
}

export function renderSettings(ctx: ScreenContext): TemplateResult {
  return html`<nigel-settings-screen .client=${ctx.client}></nigel-settings-screen>`;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-settings-screen': NigelSettingsScreen;
  }
}
