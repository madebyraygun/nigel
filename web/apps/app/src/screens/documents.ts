import { LitElement, html, css, nothing, type PropertyValues, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@nigel/ui';
import type { NcFileErrorDetail, NcFileSelectDetail } from '@nigel/ui';
import { controlsCss } from '@nigel/theme';

import { ApiError, type ApiClient } from '../api/index.js';
import { SignalWatcher } from '../mixins/signal-watcher.js';
import type {
  Client,
  DocumentDetail,
  DocumentKind,
  DocumentListRow,
  DocumentSyncResult,
} from '../api/types.js';
import {
  DOCUMENT_STATUS_FILTERS,
  documentListParams,
  documentTableRows,
} from './documents-data.js';
import { documentsGuardrailMessage } from './documents-errors.js';
import type { ScreenContext } from './context.js';

type View = 'list' | 'detail';

/** The document a route names, or null when it names none it could fetch. */
function idOf(params: URLSearchParams): number | null {
  const raw = params.get('id');
  if (raw === null || raw.trim() === '') return null;
  const parsed = Number(raw);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : null;
}

function viewOf(params: URLSearchParams): View {
  return idOf(params) !== null ? 'detail' : 'list';
}

function hasUnusableId(params: URLSearchParams): boolean {
  const raw = params.get('id');
  return raw !== null && raw.trim() !== '' && idOf(params) === null;
}

function activeStatusFilter(params: URLSearchParams): string {
  return documentListParams(params).status ?? 'all';
}

/** One line per recorded, refused or warned-about response, as the CLI prints them. */
function syncReportLines(result: DocumentSyncResult): string[] {
  const lines: string[] = [];
  for (const line of result.lines) {
    if (line.recorded.length === 0) lines.push(`#${line.documentId} ${line.title}`);
    for (const recorded of line.recorded) {
      lines.push(`#${line.documentId} ${line.title}: ${recorded} → ${line.status}`);
    }
    for (const refused of line.refused) lines.push(`Refused: ${refused}`);
    for (const warning of line.warnings) lines.push(`Warning: ${warning}`);
  }
  for (const failure of result.failures) {
    lines.push(`Could not check #${failure.documentId}: ${failure.message}`);
  }
  return lines;
}

/**
 * Documents: the list with its filing panel, and one document.
 *
 * The invoices screen's arrangement: views keyed off `ctx.params`, filters
 * that navigate so a filtered list is a URL, and every write followed by a
 * refetch rather than a splice.
 */
@customElement('nigel-documents-screen')
export class NigelDocumentsScreen extends SignalWatcher(LitElement) {
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

      header {
        display: flex;
        flex-wrap: wrap;
        align-items: flex-start;
        justify-content: space-between;
        gap: var(--wa-space-m, 12px);
      }

      h2 {
        margin: 0;
        font-size: var(--wa-font-size-l, 18px);
      }

      .actions {
        display: flex;
        flex-wrap: wrap;
        gap: var(--wa-space-s, 8px);
      }

      .filters {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--wa-space-xs, 6px);
        font-size: var(--wa-font-size-s, 13px);
      }

      .filters .label {
        color: var(--wa-color-muted);
      }

      .chip {
        padding: var(--wa-space-2xs, 4px) var(--wa-space-s, 8px);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-pill, 999px);
        color: inherit;
        text-decoration: none;
      }

      .chip[aria-current='true'] {
        border-color: var(--wa-color-brand);
        color: var(--wa-color-brand);
        font-weight: var(--wa-font-weight-medium, 500);
      }

      .chip:focus-visible {
        outline: 2px solid var(--wa-color-focus);
        outline-offset: 2px;
      }

      .back {
        color: var(--wa-color-brand);
        font-size: var(--wa-font-size-s, 13px);
        text-decoration: none;
      }

      .filing {
        display: grid;
        gap: var(--wa-space-m, 12px);
      }

      .fields {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(12rem, 1fr));
        gap: var(--wa-space-m, 12px);
      }

      .report {
        margin: 0;
        padding-left: var(--wa-space-l, 16px);
        user-select: text;
        -webkit-user-select: text;
      }

      .error {
        margin: 0;
        color: var(--wa-color-danger);
      }
    `,
  ];

  @property({ attribute: false })
  client!: ApiClient;

  @property({ attribute: false })
  params = new URLSearchParams();

  @property({ attribute: false })
  navigate!: ScreenContext['navigate'];

  @state() private rows: DocumentListRow[] = [];
  @state() private clients: Client[] = [];
  @state() private kinds: DocumentKind[] = [];
  @state() private detail: DocumentDetail | null = null;

  @state() private loading = false;
  /** A load that failed: there is nothing to show, so the whole view is it. */
  @state() private error: string | null = null;
  /**
   * A refused action on data that loaded fine — a duplicate filing, a sync the
   * settings do not allow. It lands in a notice above the list, never in place
   * of it.
   */
  @state() private actionError: string | null = null;
  @state() private syncReport: string[] | null = null;
  @state() private busy = false;

  @state() private file: File | null = null;
  @state() private fileError = '';
  @state() private fileClientId = '';
  @state() private fileKind = '';
  @state() private fileTitle = '';

  /** The route the shown data answers, so a re-render does not refetch. */
  private loadedKey: string | null = null;

  /** Drops an answer whose request is no longer the current one. */
  private loadSeq = 0;

  willUpdate(changed: PropertyValues<this>): void {
    if (!changed.has('params')) return;
    const key = this.params.toString();
    if (key === this.loadedKey) return;
    void this.load(key);
  }

  private async load(key: string, keepActionError = false): Promise<void> {
    const seq = (this.loadSeq += 1);
    const id = idOf(this.params);
    this.loading = true;
    this.error = null;
    if (!keepActionError) {
      this.actionError = null;
      this.syncReport = null;
    }

    if (hasUnusableId(this.params)) {
      this.detail = null;
      this.error = `“${this.params.get('id')}” is not a document number.`;
      this.loadedKey = null;
      this.loading = false;
      return;
    }

    try {
      if (id === null) {
        // Archived clients included, so the filter can still name one whose
        // documents outlived them; filing offers active clients only.
        const [rows, clients, kinds] = await Promise.all([
          this.client.getDocuments(documentListParams(this.params)),
          this.client.getClients(true),
          this.client.getDocumentKinds(),
        ]);
        if (seq !== this.loadSeq) return;
        this.rows = rows;
        this.clients = clients;
        this.kinds = kinds;
        if (!kinds.some((kind) => kind.name === this.fileKind)) {
          this.fileKind = kinds[0]?.name ?? '';
        }
      } else {
        const detail = await this.client.getDocument(id);
        if (seq !== this.loadSeq) return;
        this.detail = detail;
      }
      this.loadedKey = key;
    } catch (cause) {
      if (seq !== this.loadSeq) return;
      this.loadedKey = null;
      this.error = cause instanceof ApiError ? cause.message : 'Could not load the documents.';
    } finally {
      if (seq === this.loadSeq) this.loading = false;
    }
  }

  private async refresh(keepActionError = false): Promise<void> {
    this.loadedKey = null;
    await this.load(this.params.toString(), keepActionError);
  }

  private next(changes: Record<string, string | null>): URLSearchParams {
    const next = new URLSearchParams(this.params);
    for (const [key, value] of Object.entries(changes)) {
      if (value === null) next.delete(key);
      else next.set(key, value);
    }
    return next;
  }

  private go(changes: Record<string, string | null>): void {
    this.navigate('documents', this.next(changes));
  }

  private href(changes: Record<string, string | null>): string {
    const query = this.next(changes).toString();
    return `#/documents${query ? `?${query}` : ''}`;
  }

  // -- writes ---------------------------------------------------------------

  private handleClientFilter = (event: Event): void => {
    const value = String((event.target as HTMLInputElement).value ?? '');
    this.go({ clientId: value === '' ? null : value });
  };

  private handleFileSelect = (event: Event): void => {
    this.file = (event as CustomEvent<NcFileSelectDetail>).detail.file;
    this.fileError = '';
  };

  private handleFileError = (event: Event): void => {
    this.fileError = (event as CustomEvent<NcFileErrorDetail>).detail.message;
  };

  private handleFileClear = (): void => {
    this.file = null;
    this.fileError = '';
  };

  private handleFileClient = (event: Event): void => {
    this.fileClientId = String((event.target as HTMLInputElement).value ?? '');
  };

  private handleFileKind = (event: Event): void => {
    this.fileKind = String((event.target as HTMLInputElement).value ?? '');
  };

  private handleFileTitle = (event: Event): void => {
    this.fileTitle = (event.target as HTMLInputElement).value;
  };

  private get canFile(): boolean {
    return (
      this.file !== null &&
      this.fileClientId !== '' &&
      this.fileKind !== '' &&
      this.fileTitle.trim() !== ''
    );
  }

  private handleFile = async (): Promise<void> => {
    const file = this.file;
    if (!file || !this.canFile || this.busy) return;

    this.busy = true;
    this.actionError = null;
    try {
      const created = await this.client.createDocument({
        clientId: Number(this.fileClientId),
        kind: this.fileKind,
        title: this.fileTitle.trim(),
        file,
      });
      this.file = null;
      this.fileTitle = '';
      this.go({ id: String(created.id), status: null, clientId: null });
    } catch (error) {
      this.actionError = documentsGuardrailMessage(error);
    } finally {
      this.busy = false;
    }
  };

  private handleSync = async (): Promise<void> => {
    if (this.busy) return;
    this.busy = true;
    try {
      const result = await this.client.syncDocuments();
      this.actionError = null;
      await this.refresh();
      this.syncReport = [
        `Checked ${result.documentsChecked}, recorded ${result.recorded}.`,
        ...syncReportLines(result),
      ];
    } catch (error) {
      this.actionError = documentsGuardrailMessage(error);
    } finally {
      this.busy = false;
    }
  };

  private dismissActionError = (): void => {
    this.actionError = null;
  };

  private dismissSyncReport = (): void => {
    this.syncReport = null;
  };

  // -- rendering ------------------------------------------------------------

  render() {
    if (this.error) {
      // A failed load leaves no filter on screen, so the way back to the
      // unfiltered list has to be here.
      return html`
        <wc-empty-state icon="wc-icon-edit" heading="That did not load">
          <p class="error">${this.error}</p>
          <a class="chip" data-reset href="#/documents">All documents</a>
        </wc-empty-state>
      `;
    }

    if (this.loading) {
      return html`<wc-spinner size="l" show-label label="Loading documents"></wc-spinner>`;
    }

    return viewOf(this.params) === 'detail' ? this.renderDetail() : this.renderList();
  }

  private renderActionError() {
    return this.actionError
      ? html`<wc-notice-bar
          variant="danger"
          data-action-error
          message=${this.actionError}
          action-label="Dismiss"
          @nc-notice-action=${this.dismissActionError}
        ></wc-notice-bar>`
      : nothing;
  }

  private renderSyncReport() {
    const report = this.syncReport;
    if (!report) return nothing;
    const [summary, ...lines] = report;
    return html`<wc-notice-bar
      variant="info"
      data-sync-report
      message=${summary}
      action-label="Dismiss"
      @nc-notice-action=${this.dismissSyncReport}
    >
      ${lines.length > 0
        ? html`<ul class="report">
            ${lines.map((line) => html`<li>${line}</li>`)}
          </ul>`
        : nothing}
    </wc-notice-bar>`;
  }

  private renderList() {
    const clientId = this.params.get('clientId') ?? '';
    const active = this.clients.filter((client) => client.archivedAt === null);

    return html`
      <header>
        <h2>Documents</h2>
        <div class="actions">
          <wa-button
            data-sync
            appearance="outlined"
            ?disabled=${this.busy}
            title="Fetch online responses and record them"
            @click=${this.handleSync}
          >
            Sync responses
          </wa-button>
        </div>
      </header>

      ${this.renderActionError()} ${this.renderSyncReport()}

      <div class="filters">
        <span class="label">Status</span>
        ${DOCUMENT_STATUS_FILTERS.map((filter) => {
          const current = activeStatusFilter(this.params) === filter.value;
          return html`<a
            class="chip"
            data-filter=${filter.value}
            aria-current=${current ? 'true' : nothing}
            href=${this.href({ status: filter.value === 'all' ? null : filter.value })}
            >${filter.label}</a
          >`;
        })}
        <wa-select
          data-client-filter
          size="s"
          label="Client"
          value=${clientId}
          @change=${this.handleClientFilter}
        >
          <wa-option value="">All clients</wa-option>
          ${this.clients.map(
            (client) => html`<wa-option value=${String(client.id)}>${client.name}</wa-option>`,
          )}
        </wa-select>
      </div>

      <wc-document-table
        .rows=${documentTableRows(this.rows)}
        empty-message="No documents yet — file one below."
      ></wc-document-table>

      <wc-panel heading="File a document">
        <div class="filing">
          <wc-dropzone
            accept=".pdf"
            filename=${this.file?.name ?? ''}
            .size=${this.file?.size ?? 0}
            error=${this.fileError}
            ?busy=${this.busy}
            @nc-file-select=${this.handleFileSelect}
            @nc-file-error=${this.handleFileError}
            @nc-file-clear=${this.handleFileClear}
          ></wc-dropzone>
          <div class="fields">
            <wa-select
              data-file-client
              label="Client"
              value=${this.fileClientId}
              ?disabled=${this.busy}
              @change=${this.handleFileClient}
            >
              <wa-option value="">Choose a client…</wa-option>
              ${active.map(
                (client) => html`<wa-option value=${String(client.id)}>${client.name}</wa-option>`,
              )}
            </wa-select>
            <wa-select
              data-file-kind
              label="Kind"
              value=${this.fileKind}
              ?disabled=${this.busy}
              @change=${this.handleFileKind}
            >
              ${this.kinds.map(
                (kind) => html`<wa-option value=${kind.name}>${kind.name}</wa-option>`,
              )}
            </wa-select>
            <wa-input
              data-file-title
              label="Title"
              .value=${this.fileTitle}
              ?disabled=${this.busy}
              @input=${this.handleFileTitle}
            ></wa-input>
          </div>
          <div class="actions">
            <wa-button
              data-file
              variant="brand"
              ?disabled=${!this.canFile || this.busy}
              @click=${this.handleFile}
            >
              ${this.busy ? 'Filing…' : 'File document'}
            </wa-button>
          </div>
        </div>
      </wc-panel>
    `;
  }

  private renderDetail() {
    const detail = this.detail;
    if (!detail) return nothing;

    return html`
      <a class="back" data-back href=${this.href({ id: null })}>← All documents</a>
      ${this.renderActionError()}
      <header>
        <h2>${detail.title}</h2>
        <wc-document-status status=${detail.status}></wc-document-status>
      </header>
    `;
  }
}

export function renderDocuments(ctx: ScreenContext): TemplateResult {
  return html`
    <nigel-documents-screen
      .client=${ctx.client}
      .params=${ctx.params}
      .navigate=${ctx.navigate}
    ></nigel-documents-screen>
  `;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-documents-screen': NigelDocumentsScreen;
  }
}
