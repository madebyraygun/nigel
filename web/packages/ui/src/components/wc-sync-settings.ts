import { LitElement, html, css, nothing, type PropertyValues } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import { controlsCss } from '@nigel/theme';
import '../icons/icons.js';
import './wc-money.js';
import { shortDate } from './short-date.js';

/** A bank account the bridge has reported, and the Nigel account it feeds. */
export interface SyncLinkedAccount {
  key: string;
  orgName: string;
  name: string;
  currency: string;
  /** The Nigel account it syncs into, or `null` when a sync skips it. */
  account: string | null;
  lastSynced: string | null;
}

export interface SyncAccountOutcome {
  key: string;
  /** "Bank — account", as the bridge names it. */
  label: string;
  /** The Nigel account it synced into. */
  account: string;
  imported: number;
  skipped: number;
  malformed: number;
  balance: { balance: number; asOf: string } | null;
}

/** What one sync, or one preview of a sync, came to. */
export interface SyncOutcome {
  dryRun: boolean;
  accounts: SyncAccountOutcome[];
  /** Bank accounts that were skipped because nothing is linked to them. */
  unmapped: string[];
  warnings: string[];
  categorized: number | null;
  stillFlagged: number | null;
}

/** Which operation is in flight; the matching button shows it. */
export type SyncBusy = 'setup' | 'refresh' | 'map' | 'preview' | 'sync' | null;

export interface NcSyncSetupDetail {
  token: string;
}

export interface NcSyncMapDetail {
  key: string;
  /** `null` stops syncing this bank account. */
  account: string | null;
}

export interface NcSyncRunDetail {
  dryRun: boolean;
}

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/**
 * Bank sync through a SimpleFIN Bridge: connect with a setup token, link each
 * bank account to a Nigel account, and pull new transactions in.
 *
 * Controlled. The screen owns every value and every call; this reports what
 * the person asked for with `nc-sync-setup`, `nc-sync-refresh`, `nc-sync-map`
 * and `nc-sync-run`.
 *
 * The setup token is a credential. Its field is a password field that is never
 * bound to a property, so nothing the screen holds can echo it back, and it is
 * emptied once the connection is made.
 */
@customElement('wc-sync-settings')
export class WcSyncSettings extends LitElement {
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

      :host(:not([configured])) .tile {
        background: var(--wa-color-surface-alt);
        color: var(--wa-color-muted);
      }

      .status .text {
        flex: 1;
        min-width: 0;
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

      .block {
        padding: var(--wa-space-m) var(--wa-space-l);
        border-top: 1px solid var(--wa-color-border-soft);
      }

      form.token {
        display: flex;
        gap: var(--wa-space-s);
        align-items: flex-end;
        flex-wrap: wrap;
      }

      form.token wa-input {
        flex: 1 1 18rem;
      }

      .accounts {
        list-style: none;
        margin: 0;
        padding: 0;
      }

      .accounts li {
        display: flex;
        align-items: flex-end;
        gap: var(--wa-space-m);
        flex-wrap: wrap;
        padding: var(--wa-space-m) var(--wa-space-l);
        border-top: 1px solid var(--wa-color-border-soft);
      }

      .accounts .text {
        flex: 1 1 14rem;
        min-width: 0;
        align-self: center;
      }

      .account-name {
        margin: 0;
        font-size: var(--nc-font-size-body);
        font-weight: var(--wa-font-weight-medium);
      }

      .accounts wa-select {
        flex: 0 1 16rem;
        min-width: 12rem;
      }

      .actions {
        display: flex;
        flex-wrap: wrap;
        gap: var(--wa-space-s);
        padding: var(--wa-space-m) var(--wa-space-l);
        border-top: 1px solid var(--wa-color-border-soft);
      }

      .actions .spacer {
        flex: 1;
      }

      .note {
        margin: 0;
        font-size: var(--nc-font-size-meta);
        color: var(--wa-color-muted);
      }

      .error {
        margin: 0;
        font-size: var(--wa-font-size-s);
        color: var(--wa-color-danger);
      }

      .results {
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-lg);
        background: var(--wa-color-surface-alt);
        padding: var(--wa-space-m) var(--wa-space-l);
        display: grid;
        gap: var(--wa-space-s);
      }

      .results h3 {
        margin: 0;
        font-size: var(--nc-font-size-card-title);
        font-weight: var(--wa-font-weight-semibold);
      }

      .results ul {
        margin: 0;
        padding: 0;
        list-style: none;
        display: grid;
        gap: var(--wa-space-xs);
      }

      .results li {
        font-size: var(--nc-font-size-body);
      }

      .results .meta {
        display: block;
        font-size: var(--nc-font-size-meta);
        color: var(--wa-color-muted);
      }
    `,
  ];

  /** Whether a bridge is connected. */
  @property({ type: Boolean, reflect: true })
  configured = false;

  @property({ attribute: false })
  accounts: SyncLinkedAccount[] = [];

  /** The Nigel accounts a bank account can sync into, by name. */
  @property({ attribute: false })
  targets: string[] = [];

  @property({ attribute: false })
  busy: SyncBusy = null;

  /** The last sync or preview, shown under the card. */
  @property({ attribute: false })
  outcome: SyncOutcome | null = null;

  /** What the bridge said alongside a connect or a refresh. */
  @property({ attribute: false })
  notices: string[] = [];

  /** The last operation's failure, in the server's words. */
  @property({ type: String })
  error = '';

  /** A connected bridge is being swapped for another token. */
  @state()
  private replacing = false;

  protected willUpdate(changed: PropertyValues<this>): void {
    if (changed.has('configured') && this.configured) {
      this.replacing = false;
      const field = this.tokenField();
      if (field) field.value = '';
    }
  }

  private tokenField(): (HTMLElement & { value: string }) | null {
    return this.shadowRoot?.querySelector<HTMLElement & { value: string }>('[data-token]') ?? null;
  }

  private emit<T>(name: string, detail: T): void {
    this.dispatchEvent(new CustomEvent<T>(name, { detail, bubbles: true, composed: true }));
  }

  private handleSetup = (event: Event): void => {
    event.preventDefault();
    if (this.busy) return;
    const token = (this.tokenField()?.value ?? '').trim();
    if (!token) return;
    this.emit<NcSyncSetupDetail>('nc-sync-setup', { token });
  };

  private handleRefresh = (): void => {
    if (!this.busy) this.emit('nc-sync-refresh', {});
  };

  private handlePreview = (): void => {
    if (!this.busy) this.emit<NcSyncRunDetail>('nc-sync-run', { dryRun: true });
  };

  private handleSync = (): void => {
    if (!this.busy) this.emit<NcSyncRunDetail>('nc-sync-run', { dryRun: false });
  };

  private handleMap(key: string, event: Event): void {
    const value = (event.target as HTMLElement & { value: string }).value ?? '';
    this.emit<NcSyncMapDetail>('nc-sync-map', { key, account: value === '' ? null : value });
  }

  private toggleReplace = (): void => {
    this.replacing = !this.replacing;
  };

  private get linkedCount(): number {
    return this.accounts.filter((a) => a.account !== null).length;
  }

  render() {
    return html`
      <div class="card">
        ${this.renderStatus()}
        ${!this.configured || this.replacing ? this.renderToken() : nothing}
        ${this.configured ? this.renderAccounts() : nothing}
        ${this.configured ? this.renderActions() : nothing}
      </div>
      ${this.error ? html`<p class="error" role="alert">${this.error}</p>` : nothing}
      ${this.notices.length > 0
        ? html`<ul class="results" aria-label="Notes from your bridge">
            ${this.notices.map((n) => html`<li class="note">${n}</li>`)}
          </ul>`
        : nothing}
      ${this.outcome ? this.renderOutcome(this.outcome) : nothing}
    `;
  }

  private renderStatus() {
    const sub = this.configured
      ? this.accounts.length === 0
        ? 'Connected. Your bridge has not shared any bank accounts yet.'
        : `${plural(this.accounts.length, 'bank account')}, ${this.linkedCount} syncing.`
      : 'Connect a SimpleFIN Bridge and Nigel can bring your bank transactions in for you.';
    return html`
      <div class="status">
        <span class="tile" aria-hidden="true"><wc-icon-refresh></wc-icon-refresh></span>
        <div class="text">
          <p class="title">${this.configured ? 'Connected to SimpleFIN' : 'Not connected'}</p>
          <p class="sub">${sub}</p>
        </div>
        ${this.configured
          ? html`<wa-button
              appearance="plain"
              size="s"
              data-replace
              aria-expanded=${this.replacing ? 'true' : 'false'}
              ?disabled=${this.busy !== null}
              @click=${this.toggleReplace}
              >${this.replacing ? 'Keep this connection' : 'Use a new token'}</wa-button
            >`
          : nothing}
      </div>
    `;
  }

  private renderToken() {
    return html`
      <div class="block">
        <form class="token" @submit=${this.handleSetup}>
          <wa-input
            data-token
            type="password"
            label="Setup token"
            autocomplete="off"
            spellcheck="false"
            ?disabled=${this.busy !== null}
          ></wa-input>
          <wa-button
            type="submit"
            variant="brand"
            data-connect
            ?disabled=${this.busy !== null}
            ?loading=${this.busy === 'setup'}
            >Connect</wa-button
          >
        </form>
        <p class="note">
          ${this.replacing
            ? 'A new token replaces the current connection. Your linked accounts stay as they are.'
            : 'Make a setup token at your bridge and paste it here. It only works once.'}
        </p>
      </div>
    `;
  }

  private renderAccounts() {
    if (this.accounts.length === 0) {
      return html`<div class="block">
        <p class="note">Nothing here yet. Refresh to ask your bridge again.</p>
      </div>`;
    }
    const disabled = this.busy !== null;
    return html`
      <ul class="accounts" aria-label="Bank accounts">
        ${this.accounts.map(
          (a) => html`
            <li>
              <div class="text">
                <p class="account-name">${a.orgName} — ${a.name}</p>
                <p class="sub">
                  ${a.currency}${a.lastSynced
                    ? ` · last synced ${shortDate(a.lastSynced)}`
                    : a.account
                      ? ' · not synced yet'
                      : ''}
                </p>
              </div>
              <wa-select
                label="Syncs into"
                size="s"
                data-key=${a.key}
                value=${a.account ?? ''}
                ?disabled=${disabled}
                @change=${(event: Event) => this.handleMap(a.key, event)}
              >
                <wa-option value="">Not synced</wa-option>
                ${this.targets.map((t) => html`<wa-option value=${t}>${t}</wa-option>`)}
              </wa-select>
            </li>
          `,
        )}
      </ul>
    `;
  }

  private renderActions() {
    const disabled = this.busy !== null;
    const nothingLinked = this.linkedCount === 0;
    return html`
      <div class="actions">
        <wa-button
          appearance="outlined"
          size="s"
          data-refresh
          ?disabled=${disabled}
          ?loading=${this.busy === 'refresh'}
          @click=${this.handleRefresh}
          >Refresh accounts</wa-button
        >
        <span class="spacer"></span>
        <wa-button
          appearance="outlined"
          size="s"
          data-preview
          ?disabled=${disabled || nothingLinked}
          ?loading=${this.busy === 'preview'}
          @click=${this.handlePreview}
          >Preview sync</wa-button
        >
        <wa-button
          variant="brand"
          size="s"
          data-sync
          ?disabled=${disabled || nothingLinked}
          ?loading=${this.busy === 'sync'}
          @click=${this.handleSync}
          >Sync now</wa-button
        >
      </div>
    `;
  }

  private renderOutcome(outcome: SyncOutcome) {
    const total = outcome.accounts.reduce((sum, a) => sum + a.imported, 0);
    const heading = outcome.dryRun
      ? total === 0
        ? 'Nothing new to bring in'
        : `A sync would bring in ${plural(total, 'transaction')}`
      : total === 0
        ? 'All caught up'
        : `Brought in ${plural(total, 'transaction')}`;
    return html`
      <section class="results" aria-label="Sync results" data-outcome>
        <h3>${heading}</h3>
        ${outcome.dryRun ? html`<p class="note">Just a preview. Nothing has changed yet.</p>` : nothing}
        ${outcome.accounts.length > 0
          ? html`<ul>
              ${outcome.accounts.map((a) => this.renderAccountOutcome(a, outcome.dryRun))}
            </ul>`
          : nothing}
        ${outcome.categorized !== null
          ? html`<p class="note">
              Sorted ${plural(outcome.categorized, 'transaction')} by your rules${outcome.stillFlagged
                ? `; ${plural(outcome.stillFlagged, 'transaction')} still ${outcome.stillFlagged === 1 ? 'needs' : 'need'} a look in Review`
                : ''}.
            </p>`
          : nothing}
        ${outcome.unmapped.length > 0
          ? html`<p class="note">Skipped, not linked yet: ${outcome.unmapped.join(', ')}.</p>`
          : nothing}
        ${outcome.warnings.map((w) => html`<p class="note">Your bridge says: ${w}</p>`)}
      </section>
    `;
  }

  private renderAccountOutcome(a: SyncAccountOutcome, dryRun: boolean) {
    const parts = [
      `${a.imported} new`,
      `${a.skipped} already here`,
      ...(a.malformed > 0 ? [`${a.malformed} unreadable`] : []),
    ];
    return html`
      <li>
        ${a.label} → ${a.account}: ${dryRun ? 'would bring in ' : ''}${parts.join(', ')}
        ${a.balance
          ? html`<span class="meta"
              >Bank balance
              <wc-money .amount=${a.balance.balance} variant="plain"></wc-money> as of
              ${shortDate(a.balance.asOf)}</span
            >`
          : nothing}
      </li>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-sync-settings': WcSyncSettings;
  }

  interface HTMLElementEventMap {
    'nc-sync-setup': CustomEvent<NcSyncSetupDetail>;
    'nc-sync-refresh': CustomEvent<Record<string, never>>;
    'nc-sync-map': CustomEvent<NcSyncMapDetail>;
    'nc-sync-run': CustomEvent<NcSyncRunDetail>;
  }
}
