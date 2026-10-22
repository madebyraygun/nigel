import { LitElement, html, css, nothing, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@nigel/ui';
import { dispatchNcToast } from '@nigel/ui';

import { SignalWatcher } from '../mixins/signal-watcher.js';
import { ApiError, type ApiClient } from '../api/index.js';
import { getAppStore, type AppStore } from '../state/app-store.js';
import {
  initializeDashboardStore,
  type DashboardStore,
} from '../state/dashboard-store.js';
import { cashflowBuckets, droppedRowsNotice } from './dashboard-data.js';
import type { ScreenContext } from './context.js';

/**
 * The web counterpart of the TUI's home screen: year-to-date profit and loss,
 * account balances, twelve months of cash flow, and what needs review.
 *
 * The title row and its review pill belong to the app shell. The three reports
 * are fetched in parallel and rendered independently, so a failure shows up on
 * the card it belongs to rather than emptying the screen.
 */
@customElement('nigel-dashboard-screen')
export class NigelDashboardScreen extends SignalWatcher(LitElement) {
  static styles = [
    css`
      :host {
        display: flex;
        flex-direction: column;
        gap: 14px;
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
      }

      .panels {
        display: grid;
        grid-template-columns: minmax(0, 2fr) minmax(0, 1fr);
        gap: 14px;
        align-items: start;
      }

      /* minmax(0, …) rather than a bare 1fr: a track's automatic minimum is
         its content, so a card holding something that cannot shrink — a
         thirteen-month chart — would widen the page itself and leave the phone
         scrolling sideways. Capped at the track, that child scrolls inside its
         own card instead. */
      @media (max-width: 60rem) {
        .panels {
          grid-template-columns: minmax(0, 1fr);
        }
      }
    `,
  ];

  /** Supplied by the registry from the screen context. */
  @property({ attribute: false })
  client!: ApiClient;

  /** Dismissal lasts for the session — the server keeps no record of it. */
  @state() private updateDismissed = false;

  private appStore: AppStore = getAppStore();
  private store: DashboardStore | null = null;
  private reportedErrors = new Set<string>();

  connectedCallback(): void {
    super.connectedCallback();
    this.store = initializeDashboardStore(this.client);
    void this.store.load();
  }

  /**
   * Toast a failure once.
   *
   * Render runs on every signal change, and a persistent error would otherwise
   * raise a fresh toast each time — the inline message on the card is the
   * durable report, the toast is only the announcement.
   */
  private announce(error: ApiError | null): void {
    if (!error || this.reportedErrors.has(error.message)) return;
    this.reportedErrors.add(error.message);
    dispatchNcToast(this, { message: error.message, variant: 'danger' });
  }

  private renderUpdateNotice() {
    const version = this.appStore.status.get()?.updateAvailable;
    if (!version || this.updateDismissed) return nothing;

    return html`
      <wc-notice-bar
        variant="warning"
        icon="wc-icon-download"
        message="Nigel v${version} is available. Run nigel update to install it."
        dismissible
        @nc-notice-dismiss=${() => (this.updateDismissed = true)}
      ></wc-notice-bar>
    `;
  }

  private renderDroppedNotice(store: DashboardStore) {
    const notice = droppedRowsNotice(store.imports.data.get() ?? []);
    if (!notice) return nothing;
    return html`
      <wc-notice-bar
        variant="warning"
        message=${notice}
        action-label="Review imports"
        @nc-notice-action=${() => {
          window.location.hash = '#/undo';
        }}
      ></wc-notice-bar>
    `;
  }

  private renderStats(store: DashboardStore) {
    const pnl = store.pnl.data.get();
    const loading = store.pnl.loading.get();
    const error = store.pnl.error.get()?.message ?? '';
    const year = String(new Date().getFullYear());
    const cell = (id: string, label: string, amount: number, hint: string) => ({
      id,
      label,
      amount,
      hint,
      loading,
      error,
    });

    return html`
      <wc-stat-strip
        .cells=${[
          cell('income', 'Income, year to date', pnl?.totalIncome ?? 0, year),
          cell('expenses', 'Expenses, year to date', pnl?.totalExpenses ?? 0, year),
          cell('net', 'Net', pnl?.net ?? 0, 'Cash basis'),
        ]}
        @nc-retry=${() => void store.reloadPnl()}
      ></wc-stat-strip>
    `;
  }

  private renderPanels(store: DashboardStore) {
    const balance = store.balance.data.get();
    const cashflow = store.cashflow.data.get();
    const chart = cashflow
      ? cashflowBuckets(cashflow)
      : { buckets: [], caption: '' };

    return html`
      <div class="panels">
        <wc-cashflow-card
          .buckets=${chart.buckets}
          range=${chart.caption}
          ?loading=${store.cashflow.loading.get()}
          error=${store.cashflow.error.get()?.message ?? ''}
          @nc-retry=${() => void store.reloadCashflow()}
        >
          <wc-empty-state
            slot="empty"
            icon="wc-icon-import"
            heading="No cash flow yet"
            message="Import a statement and the monthly chart fills in."
          ></wc-empty-state>
        </wc-cashflow-card>

        <wc-balances-card
          .items=${balance?.accounts ?? []}
          .total=${balance?.total}
          ?loading=${store.balance.loading.get()}
          error=${store.balance.error.get()?.message ?? ''}
          @nc-retry=${() => void store.reloadBalance()}
        ></wc-balances-card>
      </div>
    `;
  }

  render() {
    const store = this.store;
    if (!store) return nothing;

    this.announce(store.pnl.error.get());
    this.announce(store.balance.error.get());
    this.announce(store.cashflow.error.get());

    if (store.isEmpty.get()) {
      return html`
        ${this.renderUpdateNotice()}
        <wc-empty-state
          icon="wc-icon-import"
          heading="No books yet"
          message="Import a bank statement to see year-to-date figures, balances and the monthly chart."
        >
          <a slot="actions" href="#/import">Import a statement</a>
        </wc-empty-state>
      `;
    }

    return html`
      ${this.renderUpdateNotice()} ${this.renderDroppedNotice(store)}
      ${this.renderStats(store)} ${this.renderPanels(store)}
    `;
  }
}

export function renderDashboard(ctx: ScreenContext): TemplateResult {
  return html`<nigel-dashboard-screen
    .client=${ctx.client}
  ></nigel-dashboard-screen>`;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-dashboard-screen': NigelDashboardScreen;
  }
}
