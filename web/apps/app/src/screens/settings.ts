import { LitElement, html, css, nothing, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import { live } from 'lit/directives/live.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/textarea/textarea.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '@nigel/ui';
import {
  confirmDialog,
  dispatchNcToast,
  type NcLogoFileDetail,
  type NcPasswordSubmitDetail,
  type NcSecurityPanelDetail,
  type NcSettingsSectionDetail,
  type NcSwitchChangeDetail,
  type SecurityPanel,
  type SettingsIndexGroup,
  type WcPasswordMode,
} from '@nigel/ui';

import { SignalWatcher } from '../mixins/signal-watcher.js';
import { ApiError, type ApiClient } from '../api/index.js';
import { getAppStore, type AppStore } from '../state/app-store.js';
import type { AppSettings, Company } from '../api/types.js';
import type { ScreenContext } from './context.js';
import { activeSection, scrolledToBottom, scrollingAncestor } from './settings-spy.js';
import { setUnsavedChanges, type UnsavedChanges } from '../state/unsaved.js';
import { applyMode, controlsCss, readMode, writeMode, type ColorMode } from '@nigel/theme';

/**
 * The cap `document::parse_logo` enforces, in the one place the browser needs
 * it: `MAX_LOGO_BYTES` in `crates/nigel-core/src/invoicing/document.rs`.
 */
const MAX_LOGO_BYTES = 128 * 1024;

const LOGO_HELP =
  'Up to 128 KiB. It is embedded in the PDF and inlined in the page; Gmail readers see your name in the email body and the logo on the attachment.';

/** How long "✓ Saved" stays beside the Letterhead title. */
const SAVED_FLASH_MS = 2200;

/** How long a section picked from the index holds the highlight while the page scrolls to it. */
const SCROLL_SETTLE_MS = 900;

const COMPANY_FIELDS: readonly (keyof Company)[] = [
  'name',
  'phone',
  'address',
  'paymentInstructions',
  'logo',
];

const LETTERHEAD_UNSAVED: UnsavedChanges = {
  heading: 'Discard unsaved letterhead changes?',
  message: 'The letterhead has edits that are not saved yet. Leaving now throws them away.',
};

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
 * Settings, in two groups under a sticky index that follows the scroll
 * position. **This book** is what is stored with the books: the letterhead,
 * the database password, and the folder they live in. **This device** is
 * what this browser or window keeps for itself: the colour mode, and the
 * update check.
 *
 * The letterhead is a draft against the stored values. While the two differ a
 * save bar holds Discard and Save; saving refreshes `/api/status`, which is
 * where the breadcrumb, the status line and the window title read the name.
 */
@customElement('nigel-settings-screen')
export class NigelSettingsScreen extends SignalWatcher(LitElement) {
  static styles = [
    controlsCss,
    css`
      :host {
        display: flex;
        flex-direction: column;
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
      }

      .layout {
        display: grid;
        grid-template-columns: 184px minmax(0, 1fr);
        gap: 36px;
        align-items: start;
      }

      .sections {
        max-width: 720px;
        min-width: 0;
      }

      .device {
        margin-top: 36px;
      }

      .pair {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: var(--wa-space-m);
      }

      .fields {
        display: grid;
        gap: var(--wa-space-m);
      }

      .row {
        display: flex;
        gap: var(--wa-space-s);
        align-items: flex-end;
        flex-wrap: wrap;
      }

      .row wa-input {
        flex: 1 1 18rem;
      }

      .switch-to {
        margin-top: var(--wa-space-l);
      }

      .note {
        color: var(--wa-color-muted);
        font-size: var(--nc-font-size-meta);
        margin: var(--wa-space-xs) 0 0;
      }

      .error {
        color: var(--wa-color-danger);
        font-size: var(--wa-font-size-s);
        margin: var(--wa-space-xs) 0 0;
      }

      wc-save-bar {
        margin-top: var(--wa-space-s);
      }

      @media (max-width: 48rem) {
        .layout {
          grid-template-columns: minmax(0, 1fr);
          gap: var(--wa-space-l);
        }

        .pair {
          grid-template-columns: minmax(0, 1fr);
        }
      }
    `,
  ];

  /** Supplied by the registry from the screen context. */
  @property({ attribute: false })
  client!: ApiClient;

  @state() private appSettings: AppSettings | null = null;
  /** The letterhead as edited. */
  @state() private company: Company | null = null;
  /** The letterhead as stored, which Discard goes back to. */
  @state() private savedCompany: Company | null = null;
  @state() private companyError = '';
  /** A failed *load*, which is a different state from a failed save. */
  @state() private companyLoadError = '';
  @state() private savedFlash = false;
  @state() private dataDirDraft = '';
  @state() private busy: string | null = null;
  /**
   * A failed password operation and which operation it was. An encrypted
   * database has two password forms, both collecting a field called "Current
   * password", so a message without its operation lands under whichever form
   * happens to be first. The two travel as one object because a message filed
   * against nothing is not a state worth being able to write.
   */
  @state() private passwordFailure: PasswordFailure | null = null;
  @state() private securityPanel: SecurityPanel = null;
  @state() private dataDirError = '';
  @state() private colorMode: ColorMode = 'system';
  @state() private active = 'letterhead';

  private store: AppStore = getAppStore();
  private savedTimer: ReturnType<typeof setTimeout> | undefined;
  private scroller: HTMLElement | null = null;
  private spyFrame = 0;
  private spyHeldUntil = 0;
  /** Whether the app has been told about the letterhead draft. */
  private reportedDirty = false;

  /**
   * Closing the window or reloading the page is the one way out no dialog of
   * ours can ask about, so the browser's own prompt does it.
   */
  private onBeforeUnload = (event: BeforeUnloadEvent) => {
    if (!this.companyDirty) return;
    event.preventDefault();
    event.returnValue = '';
  };

  connectedCallback(): void {
    super.connectedCallback();
    window.addEventListener('beforeunload', this.onBeforeUnload);
    void this.loadAppSettings();
    void this.loadCompany();
    this.colorMode = readMode();
  }

  firstUpdated(): void {
    this.scroller = scrollingAncestor(this);
    (this.scroller ?? window).addEventListener('scroll', this.onScroll, { passive: true });
  }

  updated(): void {
    const dirty = this.companyDirty;
    if (dirty === this.reportedDirty) return;
    this.reportedDirty = dirty;
    setUnsavedChanges(dirty ? LETTERHEAD_UNSAVED : null);
  }

  disconnectedCallback(): void {
    window.removeEventListener('beforeunload', this.onBeforeUnload);
    if (this.reportedDirty) setUnsavedChanges(null);
    this.reportedDirty = false;
    (this.scroller ?? window).removeEventListener('scroll', this.onScroll);
    cancelAnimationFrame(this.spyFrame);
    clearTimeout(this.savedTimer);
    super.disconnectedCallback();
  }

  // -- the index ------------------------------------------------------------

  private sections(): HTMLElement[] {
    return [...(this.shadowRoot?.querySelectorAll<HTMLElement>('wc-settings-section') ?? [])];
  }

  private onScroll = () => {
    cancelAnimationFrame(this.spyFrame);
    this.spyFrame = requestAnimationFrame(() => this.spy());
  };

  private spy(): void {
    if (performance.now() < this.spyHeldUntil) return;
    const area = this.scroller ?? document.scrollingElement;
    if (!area) return;
    const areaTop = this.scroller ? this.scroller.getBoundingClientRect().top : 0;
    const tops = this.sections().map((section) => ({
      id: section.id,
      top: section.getBoundingClientRect().top - areaTop,
    }));
    const next = activeSection(tops, scrolledToBottom(area));
    if (next) this.active = next;
  }

  private goToSection = (event: CustomEvent<NcSettingsSectionDetail>) => {
    const section = this.shadowRoot?.getElementById(event.detail.id);
    if (!section) return;
    this.active = event.detail.id;
    // Held while the smooth scroll runs, or the spy would walk the highlight
    // through every section in between — and land on the last one when the
    // section asked for is too close to the bottom to reach the top.
    this.spyHeldUntil = performance.now() + SCROLL_SETTLE_MS;
    const reduce =
      globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;
    const behavior: ScrollBehavior = reduce ? 'auto' : 'smooth';
    // The content area is scrolled directly: `scrollIntoView` also scrolls
    // every ancestor that can be scrolled from script, the document included,
    // and would slide the whole shell up out of the window.
    if (this.scroller) {
      const offset =
        section.getBoundingClientRect().top - this.scroller.getBoundingClientRect().top;
      this.scroller.scrollTo({ top: this.scroller.scrollTop + offset, behavior });
    } else {
      section.scrollIntoView?.({ behavior, block: 'start' });
    }
  };

  // -- shared ---------------------------------------------------------------

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
      const company = await this.client.getCompany();
      this.company = company;
      this.savedCompany = company;
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

  private get companyDirty(): boolean {
    const draft = this.company;
    const saved = this.savedCompany;
    if (!draft || !saved) return false;
    return COMPANY_FIELDS.some((field) => draft[field] !== saved[field]);
  }

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
   * about the 128 KiB cap. Everything else about the file — the magic bytes,
   * the type actually inside it, whether it decodes — is still the server's to
   * answer, because only it can.
   */
  private handleLogoFile = async (event: CustomEvent<NcLogoFileDetail>) => {
    const file = event.detail.file;
    if (file.size > MAX_LOGO_BYTES) {
      this.companyError = `${file.name} is ${Math.ceil(file.size / 1024)} KiB; the limit is ${MAX_LOGO_BYTES / 1024} KiB.`;
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
    }
  };

  private clearLogo = () => {
    this.editCompany('logo', '');
  };

  private discardCompany = () => {
    if (!this.savedCompany) return;
    this.company = this.savedCompany;
    this.companyError = '';
    void this.focusLetterhead();
  };

  /**
   * The save bar goes once nothing is unsaved, and with it the button that
   * had focus; the first letterhead field is where the work was.
   */
  private async focusLetterhead(): Promise<void> {
    await this.updateComplete;
    this.shadowRoot?.querySelector<HTMLElement>('#letterhead wa-input')?.focus();
  }

  private saveCompany = async () => {
    if (!this.company) return;
    this.companyError = '';
    this.busy = 'company';
    try {
      // The response is the stored values, trimmed — so the form shows what
      // was actually saved rather than what was typed.
      const stored = await this.client.setCompany(this.company);
      this.company = stored;
      this.savedCompany = stored;
      // The breadcrumb, the status line and the window title read the name
      // from status.
      await this.store.refreshStatus();
      this.flashSaved();
      void this.focusLetterhead();
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

  private flashSaved(): void {
    clearTimeout(this.savedTimer);
    this.savedFlash = true;
    this.savedTimer = setTimeout(() => {
      this.savedFlash = false;
    }, SAVED_FLASH_MS);
  }

  // -- update check ---------------------------------------------------------

  /**
   * The switch is controlled, so it shows what `appSettings` says. On a
   * failed write `appSettings` is left as it was and the switch with it: a
   * control whose write failed must not keep claiming it succeeded.
   */
  private toggleUpdateCheck = async (event: CustomEvent<NcSwitchChangeDetail>) => {
    this.busy = 'update';
    try {
      this.appSettings = await this.client.updateAppSettings({
        updateCheck: event.detail.checked,
      });
    } catch (error) {
      this.toastError(error, 'Could not save the update setting.');
    } finally {
      this.busy = null;
    }
  };

  // -- location -------------------------------------------------------------

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

  private revealDataDir = async () => {
    try {
      await this.client.dataDirReveal()?.run();
    } catch (error) {
      dispatchNcToast(this, {
        message: typeof error === 'string' ? error : 'Could not open the folder.',
        variant: 'danger',
      });
    }
  };

  // -- appearance -----------------------------------------------------------

  private handleColorMode(event: CustomEvent<{ mode: ColorMode }>): void {
    const mode = event.detail.mode;
    this.colorMode = mode;
    // The container owns persistence; the picker stays controlled. Nothing
    // is sent to the server: every /api/settings/* route is behind the locked
    // guard, so a server-stored mode could not be honoured on the unlock
    // screen, and the preference is per-device by nature anyway.
    writeMode(mode);
    applyMode(mode);
  }

  // -- password -------------------------------------------------------------

  private handleSecurityPanel = (event: CustomEvent<NcSecurityPanelDetail>) => {
    this.securityPanel = event.detail.panel;
    // A failure belongs to the attempt in the panel it was shown in; the
    // panel closing ends that attempt.
    this.passwordFailure = null;
  };

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
      this.securityPanel = null;
      // Re-read rather than assume: the encryption state the next render draws
      // comes from the server, never from an optimistic local flag.
      await this.store.refreshStatus();
      // Encrypting or decrypting swaps the forms, taking the one that had
      // focus; a change only closes its panel, which hands focus back itself.
      if (detail.mode !== 'change') void this.focusSecurity();
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

  private async focusSecurity(): Promise<void> {
    await this.updateComplete;
    const card = this.shadowRoot?.querySelector('wc-security-card');
    await card?.updateComplete;
    card?.focusFirstAction();
  }

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

  // -- rendering ------------------------------------------------------------

  /**
   * The three states the letterhead has: loaded, loading, and could-not-load.
   * The last one is not a form — an empty letterhead over a load failure would
   * save five blank fields on top of whatever is really stored.
   */
  private renderCompanyBody(nameLabel: string): TemplateResult {
    if (this.companyLoadError) {
      return html`<p class="error">${this.companyLoadError}</p>
        <div class="row">
          <wa-button variant="brand" @click=${this.retryCompany}>Retry</wa-button>
        </div>`;
    }
    if (!this.company) {
      return html`<wc-spinner label="Loading the letterhead"></wc-spinner>`;
    }
    return this.renderCompanyFields(this.company, nameLabel);
  }

  private renderCompanyFields(company: Company, nameLabel: string): TemplateResult {
    const disabled = this.busy === 'company';
    const native = this.client.importSource().kind === 'native';
    return html`
      <div class="fields">
        <div class="pair">
          <wa-input
            label=${nameLabel}
            .value=${live(company.name)}
            ?disabled=${disabled}
            @input=${this.companyInput.name}
          ></wa-input>
          <wa-input
            label="Phone"
            .value=${live(company.phone)}
            ?disabled=${disabled}
            @input=${this.companyInput.phone}
          ></wa-input>
        </div>
        <wa-textarea
          label="Address"
          rows="3"
          .value=${live(company.address)}
          ?disabled=${disabled}
          @input=${this.companyInput.address}
        ></wa-textarea>
        <div>
          <wa-textarea
            label="Payment instructions"
            rows="3"
            .value=${live(company.paymentInstructions)}
            ?disabled=${disabled}
            @input=${this.companyInput.paymentInstructions}
          ></wa-textarea>
          <p class="note">
            Printed on both the invoice page and the PDF, or on neither. Leave it
            empty if you do not take bank transfers.
          </p>
        </div>
        <wc-logo-field
          name=${company.name}
          logo=${company.logo}
          help=${LOGO_HELP}
          prompt=${native ? 'Choose a PNG or JPEG' : 'Drop a PNG or JPEG here'}
          ?disabled=${disabled}
          @nc-logo-file=${this.handleLogoFile}
          @nc-logo-clear=${this.clearLogo}
        ></wc-logo-field>
        ${this.companyError ? html`<p class="error">${this.companyError}</p>` : nothing}
      </div>
    `;
  }

  private renderSecurity(encrypted: boolean): TemplateResult {
    const operations: readonly WcPasswordMode[] = encrypted ? ['change', 'remove'] : ['set'];
    const slot = this.failureSlot(operations);
    const busy = this.busy === 'password';
    return html`
      <wc-security-card
        ?encrypted=${encrypted}
        ?busy=${busy}
        .panel=${encrypted ? this.securityPanel : null}
        @nc-security-panel=${this.handleSecurityPanel}
        @nc-password-submit=${this.handlePasswordSubmit}
      >
        ${operations.map(
          (mode) => html`<wc-password-form
            slot=${mode}
            mode=${mode}
            layout="inline"
            ?busy=${busy}
            error=${this.errorAt(mode, slot)}
          ></wc-password-form>`,
        )}
      </wc-security-card>
    `;
  }

  private renderLocation(dataDir: string): TemplateResult {
    const reveal = this.client.dataDirReveal();
    return html`
      <wc-folder-path path=${dataDir}>
        ${reveal
          ? html`<wa-button appearance="outlined" @click=${this.revealDataDir}
              >${reveal.label}</wa-button
            >`
          : nothing}
      </wc-folder-path>
      <div class="row switch-to">
        <wa-input
          label="Switch to"
          placeholder="~/Documents/other-books"
          spellcheck="false"
          .value=${this.dataDirDraft}
          ?disabled=${this.busy === 'data-dir'}
          @input=${this.handleDataDirInput}
        ></wa-input>
        <wa-button
          data-switch
          ?disabled=${this.busy === 'data-dir'}
          @click=${this.switchDataDir}
          >Switch</wa-button
        >
      </div>
      ${this.dataDirError ? html`<p class="error">${this.dataDirError}</p>` : nothing}
    `;
  }

  private renderUpdates(version: string, updateAvailable: string | null): TemplateResult {
    const current = `v${version.replace(/^v/, '')}`;
    return html`
      <wc-settings-section id="updates" heading="Updates">
        ${this.appSettings
          ? html`<wc-switch-row
              label="Check for updates automatically"
              description="Checks GitHub for a newer version once a day when Nigel starts."
              ?checked=${this.appSettings.updateCheck}
              ?disabled=${this.busy === 'update'}
              @nc-switch-change=${this.toggleUpdateCheck}
            ></wc-switch-row>`
          : html`<wc-spinner label="Loading settings"></wc-spinner>`}
        ${version
          ? html`<p class="note">
              ${updateAvailable
                ? `Version ${updateAvailable} is available; you're on ${current}.`
                : `You're on ${current}.`}
            </p>`
          : nothing}
      </wc-settings-section>
    `;
  }

  render() {
    const status = this.store.status.get();
    const encrypted = status?.encrypted ?? false;
    // Same metadata field either way; only the label follows the books profile.
    const nameLabel = status?.profile === 'personal' ? 'Household name' : 'Business name';
    // An app-store build updates through the store. `/api/status` reports
    // `distribution` once TASK-150.4 lands; until then this is always shown.
    const showUpdates = status?.distribution !== 'store';
    const groups: SettingsIndexGroup[] = [
      {
        label: 'This book',
        items: [
          { id: 'letterhead', label: 'Letterhead' },
          { id: 'security', label: 'Security' },
          { id: 'location', label: 'Location' },
        ],
      },
      {
        label: 'This device',
        items: [
          { id: 'appearance', label: 'Appearance' },
          ...(showUpdates ? [{ id: 'updates', label: 'Updates' }] : []),
        ],
      },
    ];

    return html`
      <div class="layout">
        <wc-settings-index
          .groups=${groups}
          active=${this.active}
          @nc-settings-section=${this.goToSection}
        ></wc-settings-index>

        <div class="sections">
          <wc-settings-scope
            kind="book"
            label="This book"
            heading=${this.store.companyName.get()}
            seed=${status?.dataDir ?? ''}
          ></wc-settings-scope>

          <wc-settings-section
            id="letterhead"
            heading="Letterhead"
            description="What the invoice page and the PDF say about you. The name also appears in the breadcrumb, on reports and in the window title."
            ?saved=${this.savedFlash}
          >
            ${this.renderCompanyBody(nameLabel)}
          </wc-settings-section>

          <wc-settings-section id="security" heading="Security">
            ${this.renderSecurity(encrypted)}
          </wc-settings-section>

          <wc-settings-section
            id="location"
            heading="Location"
            description="Where this set of books lives. Each set is a folder with a nigel.db inside; switching reloads Nigel onto the other database."
          >
            ${this.renderLocation(status?.dataDirDisplay || status?.dataDir || '')}
          </wc-settings-section>

          <wc-settings-scope
            class="device"
            kind="device"
            label="This device"
            heading="Applies to every set of books"
          ></wc-settings-scope>

          <wc-settings-section
            id="appearance"
            heading="Appearance"
            description="Light, dark, or whatever this device is set to. Kept on this device only, so a laptop and a desktop on the same books can differ."
          >
            <wc-appearance-picker
              .mode=${this.colorMode}
              @nc-color-mode-change=${this.handleColorMode}
            ></wc-appearance-picker>
          </wc-settings-section>

          ${showUpdates
            ? this.renderUpdates(status?.version ?? '', status?.updateAvailable ?? null)
            : nothing}

          ${this.companyDirty
            ? html`<wc-save-bar
                message="Unsaved changes to the letterhead"
                ?busy=${this.busy === 'company'}
                @nc-save-bar-discard=${this.discardCompany}
                @nc-save-bar-save=${this.saveCompany}
              ></wc-save-bar>`
            : nothing}
        </div>
      </div>
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
