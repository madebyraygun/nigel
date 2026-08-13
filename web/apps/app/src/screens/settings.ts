import { LitElement, html, css, nothing, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/textarea/textarea.js';
import '@awesome.me/webawesome/dist/components/switch/switch.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '@nigel/ui';
import { confirmDialog, dispatchNcToast, type NcPasswordSubmitDetail } from '@nigel/ui';

import { SignalWatcher } from '../mixins/signal-watcher.js';
import { ApiError, type ApiClient } from '../api/index.js';
import { getAppStore, type AppStore } from '../state/app-store.js';
import type { AppSettings, Company } from '../api/types.js';
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
        display: grid;
        gap: var(--wa-space-l, 16px);
        max-width: 48rem;
        padding: var(--wa-space-l, 16px);
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
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

      .logo-field {
        display: grid;
        gap: var(--wa-space-2xs, 4px);
        font-size: var(--wa-font-size-s, 13px);
      }

      .logo-preview {
        max-height: 3rem;
        max-width: 12rem;
        /* The stored image may be transparent, and both documents put it on
           white. Showing it on the card would misrepresent it in dark mode. */
        background: #fff;
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
  @state() private dataDirDraft = '';
  @state() private busy: string | null = null;
  @state() private passwordError = '';
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
    try {
      this.company = await this.client.getCompany();
    } catch (error) {
      this.toastError(error, 'Could not load the letterhead.');
    }
  }

  private editCompany(field: keyof Company, value: string): void {
    if (!this.company) return;
    this.company = { ...this.company, [field]: value };
  }

  private handleCompanyInput = (field: keyof Company) => (event: Event) => {
    this.editCompany(field, (event.target as HTMLInputElement).value);
  };

  /**
   * The logo travels as a `data:` URI, so the file is read here. The server
   * checks the type, the magic bytes and the size cap and answers a message;
   * this only has to hand it the bytes.
   */
  private handleLogoFile = async (event: Event) => {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
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

    this.passwordError = '';
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
      this.passwordError =
        error instanceof ApiError ? error.message : 'Could not change the password.';
    } finally {
      this.busy = null;
    }
  };

  private renderCompanyFields(company: Company, nameLabel: string): TemplateResult {
    const disabled = this.busy === 'company';
    return html`
      <div class="row">
        <wa-input
          label=${nameLabel}
          .value=${company.name}
          ?disabled=${disabled}
          @input=${this.handleCompanyInput('name')}
        ></wa-input>
        <wa-input
          label="Phone"
          .value=${company.phone}
          ?disabled=${disabled}
          @input=${this.handleCompanyInput('phone')}
        ></wa-input>
      </div>
      <wa-textarea
        label="Address"
        rows="3"
        .value=${company.address}
        ?disabled=${disabled}
        @input=${this.handleCompanyInput('address')}
      ></wa-textarea>
      <wa-textarea
        label="Payment instructions"
        rows="3"
        .value=${company.paymentInstructions}
        ?disabled=${disabled}
        @input=${this.handleCompanyInput('paymentInstructions')}
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

    return html`
      <wc-panel
        heading="Letterhead"
        description="What the invoice page and the PDF say about you. The name is also shown in the sidebar, on reports, and in the browser tab."
      >
        ${this.company ? this.renderCompanyFields(this.company, nameLabel) : html`<wc-spinner
              label="Loading the letterhead"
            ></wc-spinner>`}
        ${this.companyError ? html`<p class="error">${this.companyError}</p>` : nothing}
        <wa-button
          slot="actions"
          variant="brand"
          ?disabled=${this.busy === 'company' || this.company === null}
          @click=${this.saveCompany}
          >Save</wa-button
        >
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
          ? 'This database is encrypted.'
          : 'This database is not encrypted. Anyone with the file can read it.'}
      >
        <wc-password-form
          mode=${encrypted ? 'change' : 'set'}
          ?busy=${this.busy === 'password'}
          error=${this.passwordError}
          @nc-password-submit=${this.handlePasswordSubmit}
        ></wc-password-form>
        ${encrypted
          ? html`<wc-password-form
              mode="remove"
              ?busy=${this.busy === 'password'}
              @nc-password-submit=${this.handlePasswordSubmit}
            ></wc-password-form>`
          : nothing}
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
