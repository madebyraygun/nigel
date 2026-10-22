import { LitElement, html, css, nothing } from 'lit';
import { customElement, property, query } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '../icons/icons.js';
import './wc-period-nav.js';
import type { NcDateGranularity, NcPeriod } from './wc-period-nav.js';
import { controlsCss } from '@nigel/theme';

/** An account the register can be filtered to. */
export interface AccountOption {
  id: number;
  name: string;
}

export interface NcAccountChangeDetail {
  /** Null is "all accounts" — the register's unfiltered default. */
  account: string | null;
}

export interface NcSearchChangeDetail {
  query: string;
}

/** The value `wa-select` carries for "all accounts"; empty is not selectable. */
const ALL_ACCOUNTS = '__all__';

/**
 * Filters above the register: account, period, and incremental search, with
 * an `end` slot for the screen's own trailing controls (the register puts its
 * keyboard legend there).
 *
 * Search is client-side — `/api/reports/register` has no search parameter and
 * the TUI filters in memory too — so there is no debounce. A keystroke is a
 * pass over an array that is already loaded, and waiting would only add
 * latency to something the TUI does instantly.
 */
@customElement('wc-register-toolbar')
export class WcRegisterToolbar extends LitElement {
  static styles = [
    controlsCss,
    css`
      /* One wrapping row of 32px controls. On a phone it wraps into three
         short lines — account and granularity, the pager and the slotted
         end controls, then search and the count — rather than one control a
         line. */
      :host {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--wa-space-s);
        font-family: var(--wa-font-family-sans);
        font-size: var(--nc-font-size-control);
        color: var(--wa-color-text);
        --wa-form-control-height: 32px;
        --wa-form-control-value-font-size: var(--nc-font-size-control);
        --wa-form-control-padding-inline: 10px;
      }

      wa-select {
        flex: 0 1 12rem;
        min-width: 9rem;
      }

      /* Each field is named for assistive technology; its placeholder and
         its options show a sighted user what it is, so the label takes no
         room. */
      wa-select::part(form-control-label),
      .search::part(form-control-label) {
        position: absolute;
        width: 1px;
        height: 1px;
        overflow: hidden;
        clip-path: inset(50%);
        white-space: nowrap;
      }

      wc-period-nav {
        gap: var(--wa-space-s);
      }

      wc-period-nav::part(pager) {
        height: 32px;
        box-sizing: border-box;
        gap: 0;
        background: var(--nc-color-field);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-md);
      }

      wc-period-nav::part(step) {
        height: 30px;
        padding: 0 var(--wa-space-s);
        border: none;
        border-radius: var(--wa-radius-md);
        color: var(--wa-color-muted);
      }

      wc-period-nav::part(label) {
        min-width: 132px;
        font-weight: var(--wa-font-weight-medium);
        white-space: nowrap;
      }

      wc-period-nav::part(kinds) {
        height: 32px;
        box-sizing: border-box;
        padding: 3px;
        gap: 2px;
        background: var(--wa-color-surface-alt);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-md);
      }

      wc-period-nav::part(kind) {
        height: 100%;
        padding: 0 10px;
        border-radius: var(--wa-radius-sm);
        color: var(--wa-color-muted);
        font-size: var(--nc-font-size-control);
      }

      wc-period-nav::part(kind-checked) {
        background: var(--wa-color-surface);
        color: var(--wa-color-text);
        box-shadow: var(--wa-shadow-sm);
      }

      .search {
        flex: 1 1 220px;
        min-width: 12rem;
      }

      .search wc-icon-search {
        --nc-icon-size: 16px;
        color: var(--wa-color-muted);
      }

      .status {
        margin: 0;
        font-size: var(--nc-font-size-meta);
        color: var(--wa-color-muted);
        white-space: nowrap;
        font-variant-numeric: tabular-nums;
      }

      /* A phone lays the same controls out in three full lines: account
         and granularity, then the pager beside the slotted end controls, then
         search and the count. The pager's two halves become items of this
         row so each can sit on its own line. */
      @media (max-width: 48rem) {
        wa-select {
          order: -2;
          flex: 1 1 8rem;
        }

        wc-period-nav {
          display: contents;
        }

        wc-period-nav::part(kinds) {
          order: -1;
        }

        wc-period-nav::part(pager) {
          flex: 1 1 auto;
          justify-content: space-between;
        }

        .search,
        .status {
          order: 1;
        }

        .search {
          min-width: 0;
        }
      }

      /* Screen chrome, not part of the report. This lives here rather than in
         @nigel/theme's print sheet because a rule that hides an element has to
         be in the tree that element is in, and every wc-* here sits inside
         nigel-app's shadow root where a document rule cannot reach it. */
      @media print {
        :host {
          display: none;
        }
      }
    `,
  ];

  @property({ attribute: false })
  accounts: AccountOption[] = [];

  /** The account name the register is filtered to, or null for all. */
  @property({ type: String })
  account: string | null = null;

  @property({ attribute: false })
  period: NcPeriod = { kind: 'all' };

  @property({ type: String })
  granularity: NcDateGranularity = 'monthAndYear';

  @property({ type: String })
  search = '';

  /** Rows matching the search. Null when nothing is being searched for. */
  @property({ type: Number, attribute: 'match-count' })
  matchCount: number | null = null;

  @property({ type: Number, attribute: 'total-count' })
  totalCount = 0;

  @property({ type: Boolean, reflect: true })
  busy = false;

  @query('.search') private searchInput?: HTMLElement & { value: string };

  /** Focus the search box — what the table asks for when `/` is pressed. */
  focusSearch(): void {
    this.searchInput?.focus();
  }

  private handleAccountChange(event: Event): void {
    const value = (event.target as HTMLElement & { value: string }).value;
    this.dispatchEvent(
      new CustomEvent<NcAccountChangeDetail>('nc-account-change', {
        detail: { account: value === ALL_ACCOUNTS ? null : value },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handleSearchInput(event: Event): void {
    const value = (event.target as HTMLElement & { value: string }).value ?? '';
    this.dispatchEvent(
      new CustomEvent<NcSearchChangeDetail>('nc-search-change', {
        detail: { query: value },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private get statusText(): string {
    if (this.matchCount === null) {
      return this.totalCount === 1 ? '1 row' : `${this.totalCount} rows`;
    }
    if (this.matchCount === 0) return 'No matches';
    return `${this.matchCount} of ${this.totalCount} rows`;
  }

  render() {
    return html`
      <wa-select
        label="Account"
        size="s"
        value=${this.account ?? ALL_ACCOUNTS}
        ?disabled=${this.busy}
        @change=${this.handleAccountChange}
      >
        <wa-option value=${ALL_ACCOUNTS}>All accounts</wa-option>
        ${this.accounts.map(
          (account) => html`<wa-option value=${account.name}>${account.name}</wa-option>`,
        )}
      </wa-select>

      ${this.granularity === 'none'
        ? nothing
        : html`
            <wc-period-nav
              allow-all
              granularity=${this.granularity}
              .period=${this.period}
              ?disabled=${this.busy}
            ></wc-period-nav>
          `}

      <wa-input
        class="search"
        type="search"
        label="Search"
        size="s"
        placeholder="Description, vendor or category"
        value=${this.search}
        ?disabled=${this.busy}
        @input=${this.handleSearchInput}
      >
        <wc-icon-search slot="start"></wc-icon-search>
      </wa-input>

      <p class="status" role="status" aria-live="polite">${this.statusText}</p>
      <slot name="end"></slot>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-register-toolbar': WcRegisterToolbar;
  }

  interface HTMLElementEventMap {
    'nc-account-change': CustomEvent<NcAccountChangeDetail>;
    'nc-search-change': CustomEvent<NcSearchChangeDetail>;
  }
}
