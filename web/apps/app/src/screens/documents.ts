import { LitElement, html, css, nothing, type PropertyValues, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/textarea/textarea.js';
import '@nigel/ui';
import {
  confirmDialog,
  defaultRecipients,
  validateRecipients,
  type ConfirmOptions,
  type NcFileErrorDetail,
  type NcFileSelectDetail,
  type NcRecipientsChangeDetail,
  type RecipientContactOption,
  type RecipientEditorValue,
  type RecipientErrors,
  type SendFailureView,
  type SendStepView,
} from '@nigel/ui';
import { controlsCss } from '@nigel/theme';

import { ApiError, type ApiClient } from '../api/index.js';
import { SignalWatcher } from '../mixins/signal-watcher.js';
import { getAppStore, type AppStore } from '../state/app-store.js';
import type {
  Client,
  DocumentDetail,
  DocumentKind,
  DocumentListRow,
  DocumentActionResult,
  DocumentPatch,
  DocumentSyncResult,
  RecipientLink,
} from '../api/types.js';
import {
  DOCUMENT_STATUS_FILTERS,
  documentListParams,
  documentSendStepViews,
  documentTableRows,
  latestSentVersion,
  recipientContacts,
  sendRequestFrom,
  timelineVersions,
} from './documents-data.js';
import { documentSendFailureMessage, documentsGuardrailMessage } from './documents-errors.js';
import type { ScreenContext } from './context.js';

type View = 'list' | 'detail';

/** The detail's dialogs that collect something before the confirmation. */
type ActionDialog = 'edit' | 'revise' | 'accept' | 'requestChanges' | 'decline' | 'countersign';

const RECORDED_ASSENT =
  'A simple electronic signature with an audit trail: Nigel records the name, the date and the checksum of the version it binds to. It does not verify the signer’s identity and is not a qualified (eIDAS) signature.';

const NOTE_LIMIT = 4000;

const BLANK_RECIPIENTS: RecipientEditorValue = {
  signer: { name: '', email: '' },
  collaborators: [],
};

function hasRecipientErrors(errors: RecipientErrors): boolean {
  return Object.keys(errors).length > 0;
}

const SYNC_KEYS = ['r2_account_id', 'r2_access_key', 'r2_secret_key', 'r2_private_bucket'];

function missingSentence(subject: string, missing: string[]): string {
  if (missing.length === 0) return `${subject} is not configured yet.`;
  return `${subject} needs ${missing.join(', ')}, which ${
    missing.length === 1 ? 'is' : 'are'
  } not set.`;
}

function pageLinkLabel(link: RecipientLink): string {
  return `${link.name} (${link.role}) — ${link.email}`;
}

/** The document a route names, or null when it is absent or not a positive integer. */
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

/** One line per recorded, refused or warned-about response. */
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

      .meta {
        margin: var(--wa-space-2xs, 4px) 0 0;
        color: var(--wa-color-muted);
        font-size: var(--wa-font-size-s, 13px);
        overflow-wrap: anywhere;
      }

      .dialog-fields {
        display: grid;
        gap: var(--wa-space-m, 12px);
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
   * settings do not allow. It lands in a notice above the list or the
   * document, never in place of it.
   */
  @state() private actionError: string | null = null;
  @state() private syncReport: string[] | null = null;
  /** What an action that went through could not finish — a page left unpublished. */
  @state() private actionWarnings: string[] = [];
  @state() private busy = false;

  @state() private previewOpen = false;
  @state() private previewHtml = '';
  @state() private previewError = '';
  @state() private previewLoading = false;

  @state() private dialog: ActionDialog | null = null;
  @state() private dialogError: string | null = null;
  @state() private fieldName = '';
  @state() private fieldNote = '';
  @state() private fieldDate = '';
  @state() private fieldTitle = '';
  @state() private fieldKind = '';
  @state() private revision: File | null = null;
  @state() private revisionError = '';

  @state() private sendOpen = false;
  @state() private sendPhase: 'confirm' | 'sending' | 'sent' | 'failed' = 'confirm';
  @state() private sendSteps: SendStepView[] = [];
  @state() private sendWarnings: string[] = [];
  @state() private sendFailure: SendFailureView | null = null;
  @state() private sendLinks: RecipientLink[] = [];
  @state() private sendCautions: string[] = [];
  @state() private recipients: RecipientEditorValue = BLANK_RECIPIENTS;
  @state() private contacts: RecipientContactOption[] = [];

  @state() private file: File | null = null;
  @state() private fileError = '';
  @state() private fileClientId = '';
  @state() private fileKind = '';
  @state() private fileTitle = '';

  private appStore: AppStore = getAppStore();

  private get documentsStatus() {
    return this.appStore.status.get()?.documents;
  }

  /** Why Send is unavailable whatever the document's flags say, or empty. */
  private sendBlockReason(): string {
    const documents = this.documentsStatus;
    if (!documents || documents.sendConfigured) return '';
    return missingSentence('Sending documents', documents.missing);
  }

  /** Why Sync is unavailable, or empty. */
  private syncBlockReason(): string {
    const documents = this.documentsStatus;
    if (documents?.syncConfigured) return '';
    const missing = documents ? SYNC_KEYS.filter((key) => documents.missing.includes(key)) : [];
    return missingSentence('Syncing responses', missing);
  }

  /** The route the shown data answers, so a re-render does not refetch. */
  private loadedKey: string | null = null;

  /** Drops an answer whose request is no longer the current one. */
  private loadSeq = 0;

  willUpdate(changed: PropertyValues<this>): void {
    if (!changed.has('params')) return;
    const key = this.params.toString();
    if (key === this.loadedKey) return;
    this.closeDialogs();
    void this.load(key);
  }

  /** Drop every dialog and the preview, which belong to the document being left. */
  private closeDialogs(): void {
    this.dialog = null;
    this.dialogError = null;
    this.revision = null;
    this.revisionError = '';
    this.sendOpen = false;
    this.sendPhase = 'confirm';
    this.sendSteps = [];
    this.sendFailure = null;
    this.sendLinks = [];
    this.previewOpen = false;
    this.previewHtml = '';
    this.previewError = '';
  }

  private async load(key: string, keepActionError = false): Promise<void> {
    const seq = (this.loadSeq += 1);
    const id = idOf(this.params);
    this.loading = true;
    this.error = null;
    if (!keepActionError) {
      this.actionError = null;
      this.actionWarnings = [];
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

  /** Refetch after a write. An open preview is re-rendered, since a revise changes it. */
  private async refresh(keepActionError = false): Promise<void> {
    this.loadedKey = null;
    this.previewHtml = '';
    this.previewError = '';
    await this.load(this.params.toString(), keepActionError);
    if (this.previewOpen) void this.loadPreview();
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

  // -- detail ---------------------------------------------------------------

  private togglePreview = (): void => {
    this.previewOpen = !this.previewOpen;
    if (this.previewOpen) void this.loadPreview();
  };

  /**
   * The rendered page, fetched as text so a render failure arrives as the
   * server's sentence. Shared by the disclosure and the send dialog.
   */
  private async loadPreview(): Promise<void> {
    const detail = this.detail;
    if (!detail || this.previewLoading || this.previewHtml) return;
    this.previewError = '';
    this.previewLoading = true;
    try {
      const page = await this.client.documentPreviewHtml(detail.id);
      if (this.detail?.id === detail.id) this.previewHtml = page;
    } catch (error) {
      if (this.detail?.id === detail.id) {
        this.previewError = error instanceof ApiError ? error.message : String(error);
      }
    } finally {
      this.previewLoading = false;
    }
  }

  /** The desktop shell saves through a dialog rather than following a link. */
  private handlePdf = async (event: Event): Promise<void> => {
    const detail = this.detail;
    if (!detail) return;
    const target = this.client.documentPreviewTarget(detail.id);
    if (target.kind !== 'action') return;
    event.preventDefault();
    try {
      await target.run();
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      this.actionError = `Couldn’t save the PDF. ${reason}`;
    }
  };

  private signerName(detail: DocumentDetail): string {
    const signer = latestSentVersion(detail)?.recipients.find((r) => r.role === 'signer');
    return signer?.name ?? '';
  }

  private openDialog(dialog: ActionDialog): void {
    const detail = this.detail;
    if (!detail || this.busy) return;
    this.dialog = dialog;
    this.dialogError = null;
    this.fieldName =
      dialog === 'accept' || dialog === 'requestChanges' ? this.signerName(detail) : '';
    this.fieldNote = '';
    this.fieldDate = '';
    this.fieldTitle = detail.title;
    this.fieldKind = detail.kind;
    this.revision = null;
    this.revisionError = '';
    if (dialog === 'edit' && this.kinds.length === 0) void this.loadKinds();
  }

  private async loadKinds(): Promise<void> {
    try {
      this.kinds = await this.client.getDocumentKinds();
    } catch (error) {
      this.dialogError = documentsGuardrailMessage(error);
    }
  }

  private closeDialog = (): void => {
    this.dialog = null;
    this.dialogError = null;
    this.revision = null;
    this.revisionError = '';
  };

  private handleFieldName = (event: Event): void => {
    this.fieldName = (event.target as HTMLInputElement).value;
  };

  private handleFieldNote = (event: Event): void => {
    this.fieldNote = (event.target as HTMLTextAreaElement).value;
  };

  private handleFieldDate = (event: Event): void => {
    this.fieldDate = (event.target as HTMLInputElement).value;
  };

  private handleFieldTitle = (event: Event): void => {
    this.fieldTitle = (event.target as HTMLInputElement).value;
  };

  private handleFieldKind = (event: Event): void => {
    this.fieldKind = String((event.target as HTMLInputElement).value ?? '');
  };

  private handleRevisionSelect = (event: Event): void => {
    this.revision = (event as CustomEvent<NcFileSelectDetail>).detail.file;
    this.revisionError = '';
  };

  private handleRevisionError = (event: Event): void => {
    this.revisionError = (event as CustomEvent<NcFileErrorDetail>).detail.message;
  };

  private handleRevisionClear = (): void => {
    this.revision = null;
    this.revisionError = '';
  };

  /** What the dialog refuses before the server sees it, or null. */
  private dialogProblem(dialog: ActionDialog): string | null {
    const name = this.fieldName.trim();
    const note = this.fieldNote.trim();
    switch (dialog) {
      case 'edit':
        return this.fieldTitle.trim() === '' ? 'A document needs a title.' : null;
      case 'revise':
        return this.revision ? null : 'Choose the revised PDF.';
      case 'accept':
        return name === '' ? 'Enter the name of the person who accepted.' : null;
      case 'countersign':
        return name === '' ? 'Enter the name of the person countersigning.' : null;
      case 'requestChanges':
        if (name === '') return 'Enter the name of the person asking for changes.';
        if (note === '') return 'Enter the changes they asked for.';
        return note.length > NOTE_LIMIT
          ? `A change request is at most ${NOTE_LIMIT} characters.`
          : null;
      case 'decline':
        return note.length > NOTE_LIMIT
          ? `A decline note is at most ${NOTE_LIMIT} characters.`
          : null;
    }
  }

  private confirmation(
    dialog: Exclude<ActionDialog, 'edit'>,
    detail: DocumentDetail,
  ): ConfirmOptions {
    const name = this.fieldName.trim();
    const sent = latestSentVersion(detail)?.number;
    const version = sent === undefined ? 'the sent version' : `version ${sent}`;
    const on = this.fieldDate ? ` on ${this.fieldDate}` : '';
    const next = Math.max(0, ...detail.versions.map((v) => v.number)) + 1;
    switch (dialog) {
      case 'revise':
        return {
          heading: `Revise “${detail.title}”?`,
          message: `${this.revision?.name ?? 'The PDF'} becomes version ${next}, a draft to send. Until then the live pages say the document is being revised.`,
          confirmLabel: 'Revise',
        };
      case 'accept':
        return {
          heading: `Record acceptance of “${detail.title}”?`,
          message: `This records that ${name} accepted ${version}${on}. ${RECORDED_ASSENT}`,
          confirmLabel: 'Record acceptance',
        };
      case 'requestChanges':
        return {
          heading: `Record a change request on “${detail.title}”?`,
          message: `This adds ${name}’s note to ${version}${on}, and the document moves to changes requested.`,
          confirmLabel: 'Record change request',
        };
      case 'decline':
        return {
          heading: `Record that “${detail.title}” was declined?`,
          message: `Declining is final: nothing more can be done to the document afterwards.`,
          confirmLabel: 'Record decline',
          variant: 'danger',
        };
      case 'countersign':
        return {
          heading: `Countersign “${detail.title}”?`,
          message: `This records ${name}’s countersignature on ${version}${on}, and the document becomes executed. ${RECORDED_ASSENT}`,
          confirmLabel: 'Countersign',
        };
    }
  }

  private actionCall(
    dialog: Exclude<ActionDialog, 'edit'>,
  ): (id: number) => Promise<DocumentActionResult> {
    const name = this.fieldName.trim();
    const note = this.fieldNote.trim();
    const date = this.fieldDate ? { date: this.fieldDate } : {};
    const revision = this.revision;
    switch (dialog) {
      case 'revise':
        return (id) => this.client.reviseDocument(id, revision as File);
      case 'accept':
        return (id) => this.client.acceptDocument(id, { name, ...date });
      case 'requestChanges':
        return (id) => this.client.requestDocumentChanges(id, { name, note, ...date });
      case 'decline':
        return (id) => this.client.declineDocument(id, { ...(note ? { note } : {}), ...date });
      case 'countersign':
        return (id) => this.client.countersignDocument(id, { name, ...date });
    }
  }

  private handleDialogSave = async (): Promise<void> => {
    const detail = this.detail;
    const dialog = this.dialog;
    if (!detail || !dialog || this.busy) return;

    const problem = this.dialogProblem(dialog);
    if (problem) {
      this.dialogError = problem;
      return;
    }
    this.dialogError = null;

    if (dialog === 'edit') {
      const patch: DocumentPatch = {};
      if (this.fieldTitle.trim() !== detail.title) patch.title = this.fieldTitle.trim();
      if (this.fieldKind !== detail.kind) patch.kind = this.fieldKind;
      if (Object.keys(patch).length === 0) {
        this.closeDialog();
        return;
      }
      await this.runAction((id) => this.client.updateDocument(id, patch));
      return;
    }

    if (!(await confirmDialog(this.confirmation(dialog, detail)))) return;
    await this.runAction(this.actionCall(dialog));
  };

  private handleWithdraw = async (): Promise<void> => {
    const detail = this.detail;
    if (!detail || this.busy) return;
    const confirmed = await confirmDialog({
      heading: `Withdraw “${detail.title}”?`,
      message:
        'Withdrawing is final. Published pages are replaced with a withdrawn notice, and nobody can respond any more.',
      confirmLabel: 'Withdraw',
      variant: 'danger',
    });
    if (!confirmed) return;
    await this.runAction((id) => this.client.withdrawDocument(id));
  };

  /**
   * One write against the shown document, then a refetch.
   *
   * A 400 names a value in the open dialog, so it stays there beside the
   * field. Any other refusal closes the dialog and lands above the document,
   * and the refetch stands because a refusal can mean the view is stale —
   * `document_wrong_state`'s sentence says the view has been refreshed.
   */
  private async runAction(
    call: (id: number) => Promise<DocumentDetail | DocumentActionResult>,
  ): Promise<void> {
    const detail = this.detail;
    if (!detail) return;
    this.busy = true;
    try {
      const result = await call(detail.id);
      this.closeDialog();
      this.actionError = null;
      await this.refresh();
      this.actionWarnings = 'warnings' in result ? result.warnings : [];
    } catch (error) {
      if (this.dialog && error instanceof ApiError && error.status === 400) {
        this.dialogError = documentsGuardrailMessage(error);
        return;
      }
      this.closeDialog();
      this.actionError = documentsGuardrailMessage(error);
      await this.refresh(true);
    } finally {
      this.busy = false;
    }
  }

  private dismissActionWarning(index: number): void {
    this.actionWarnings = this.actionWarnings.filter((_, i) => i !== index);
  }

  // -- send -----------------------------------------------------------------

  private openSend = async (): Promise<void> => {
    const detail = this.detail;
    if (!detail || this.busy || this.sendBlockReason()) return;
    this.sendOpen = true;
    this.sendPhase = 'confirm';
    this.sendSteps = [];
    this.sendFailure = null;
    this.sendWarnings = [];
    this.sendLinks = [];
    this.sendCautions = [];
    this.contacts = [];
    this.recipients = BLANK_RECIPIENTS;
    void this.loadPreview();

    try {
      const client = await this.client.getClient(detail.clientId);
      if (!this.sendOpen || this.detail?.id !== detail.id) return;
      this.contacts = recipientContacts(client);
      if (this.recipients === BLANK_RECIPIENTS) this.recipients = defaultRecipients(this.contacts);
    } catch (error) {
      const reason = error instanceof ApiError ? ` ${error.message}` : '';
      this.sendCautions = [
        `Could not load ${detail.clientName}’s contacts, so no recipient is filled in.${reason}`,
      ];
    }
  };

  private handleRecipients = (event: Event): void => {
    this.recipients = (event as CustomEvent<NcRecipientsChangeDetail>).detail.value;
  };

  private handleSend = async (): Promise<void> => {
    const detail = this.detail;
    if (!detail || this.sendPhase === 'sending') return;
    if (hasRecipientErrors(validateRecipients(this.recipients))) return;

    this.sendPhase = 'sending';
    this.sendWarnings = [];
    this.sendFailure = null;
    this.sendLinks = [];
    this.sendSteps = documentSendStepViews({ running: true });

    try {
      const result = await this.client.sendDocument(detail.id, sendRequestFrom(this.recipients));
      this.sendSteps = documentSendStepViews({ running: false, result });
      this.sendWarnings = [...result.configWarnings, ...result.warnings];
      this.sendLinks = result.links;
      this.sendPhase = 'sent';
      if (result.document && this.detail?.id === detail.id) this.detail = result.document;
    } catch (error) {
      this.sendSteps = documentSendStepViews({ running: false, error });
      this.sendFailure = documentSendFailureMessage(error, detail.title);
      this.sendPhase = 'failed';
    }
  };

  private closeSend = (): void => {
    this.sendOpen = false;
    void this.refresh();
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
    const syncBlocked = this.syncBlockReason();

    return html`
      <header>
        <h2>Documents</h2>
        <div class="actions">
          <wa-button
            data-sync
            appearance="outlined"
            ?disabled=${syncBlocked !== '' || this.busy}
            title=${syncBlocked || 'Fetch online responses and record them'}
            @click=${this.handleSync}
          >
            Sync responses
          </wa-button>
        </div>
      </header>

      ${syncBlocked ? html`<p class="meta" data-sync-note>${syncBlocked}</p>` : nothing}

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
      ${this.actionWarnings.map(
        (warning, index) =>
          html`<wc-notice-bar
            variant="warning"
            data-action-warning
            message=${warning}
            action-label="Dismiss"
            @nc-notice-action=${() => this.dismissActionWarning(index)}
          ></wc-notice-bar>`,
      )}
      <header>
        <div>
          <h2>${detail.title}</h2>
          <p class="meta">${detail.kind} · ${detail.clientName}</p>
        </div>
        <wc-document-status status=${detail.status}></wc-document-status>
      </header>

      ${this.renderActions(detail)} ${this.renderClosure(detail)}

      <wc-panel heading="Preview">
        <div class="actions">
          <wa-button
            data-preview-toggle
            appearance="outlined"
            aria-expanded=${this.previewOpen ? 'true' : 'false'}
            @click=${this.togglePreview}
          >
            ${this.previewOpen ? 'Hide preview' : 'Show preview'}
          </wa-button>
          ${this.renderPdfLink(detail)}
        </div>
        ${this.previewOpen ? this.renderPreview(detail) : nothing}
      </wc-panel>

      <wc-panel
        heading="History"
        description=${`Every version, who it went to and every response. ${RECORDED_ASSENT}`}
      >
        <wc-document-timeline .versions=${timelineVersions(detail)}></wc-document-timeline>
      </wc-panel>

      ${this.renderActionDialog(detail)} ${this.renderSendDialog(detail)}
    `;
  }

  /** Only what the server's flags allow, never what the status word suggests. */
  private renderActions(detail: DocumentDetail) {
    const sendBlocked = this.sendBlockReason();
    const available: {
      action: string;
      label: string;
      run: () => void;
      danger?: boolean;
      blocked?: string;
    }[] = [];
    if (detail.canSend) {
      available.push({
        action: 'send',
        label: 'Send…',
        run: () => void this.openSend(),
        blocked: sendBlocked,
      });
    }
    if (detail.canRevise) {
      available.push({ action: 'revise', label: 'Revise…', run: () => this.openDialog('revise') });
    }
    if (detail.canAccept) {
      available.push({
        action: 'accept',
        label: 'Record acceptance…',
        run: () => this.openDialog('accept'),
      });
    }
    if (detail.canRequestChanges) {
      available.push({
        action: 'requestChanges',
        label: 'Record change request…',
        run: () => this.openDialog('requestChanges'),
      });
    }
    if (detail.canDecline) {
      available.push({
        action: 'decline',
        label: 'Record decline…',
        run: () => this.openDialog('decline'),
        danger: true,
      });
    }
    if (detail.canCountersign) {
      available.push({
        action: 'countersign',
        label: 'Countersign…',
        run: () => this.openDialog('countersign'),
      });
    }
    if (detail.canEdit) {
      available.push({ action: 'edit', label: 'Edit…', run: () => this.openDialog('edit') });
    }
    if (detail.canWithdraw) {
      available.push({
        action: 'withdraw',
        label: 'Withdraw…',
        run: () => void this.handleWithdraw(),
        danger: true,
      });
    }
    if (available.length === 0) return nothing;

    return html`<div class="actions">
        ${available.map(
          (item, index) => html`<wa-button
            data-action=${item.action}
            variant=${item.danger ? 'danger' : index === 0 ? 'brand' : 'neutral'}
            appearance=${index === 0 && !item.danger ? nothing : 'outlined'}
            ?disabled=${this.busy || Boolean(item.blocked)}
            title=${item.blocked || nothing}
            @click=${item.run}
          >
            ${item.label}
          </wa-button>`,
        )}
      </div>
      ${detail.canSend && sendBlocked
        ? html`<p class="meta" data-send-note>${sendBlocked}</p>`
        : nothing}`;
  }

  /** How a declined or withdrawn document ended, which the timeline does not carry. */
  private renderClosure(detail: DocumentDetail) {
    if (detail.withdrawnAt) {
      return html`<p class="meta" data-closure>Withdrawn ${detail.withdrawnAt}.</p>`;
    }
    if (detail.declinedAt) {
      return html`<p class="meta" data-closure>
        Declined ${detail.declinedAt}${detail.declineNote ? `: ${detail.declineNote}` : '.'}
      </p>`;
    }
    return nothing;
  }

  private renderPdfLink(detail: DocumentDetail) {
    const target = this.client.documentPreviewTarget(detail.id);
    return html`<wa-button
      data-pdf-link
      appearance="outlined"
      href=${target.kind === 'href' ? target.href : nothing}
      download=${target.kind === 'href' ? '' : nothing}
      @click=${this.handlePdf}
    >
      Download PDF
    </wa-button>`;
  }

  private renderPreview(detail: DocumentDetail) {
    if (this.previewError) {
      return html`<wc-notice-bar
        variant="danger"
        data-preview-error
        message=${this.previewError}
      ></wc-notice-bar>`;
    }
    if (this.previewLoading || !this.previewHtml) {
      return html`<wc-spinner show-label label="Rendering the document"></wc-spinner>`;
    }
    return html`<wc-document-frame
      label=${`“${detail.title}” as a recipient will see it`}
      .srcdoc=${this.previewHtml}
    ></wc-document-frame>`;
  }

  private renderDialogFields(dialog: ActionDialog, detail: DocumentDetail) {
    const date = html`<wa-input
      data-field-date
      type="date"
      label="Date"
      hint="Leave empty for today."
      .value=${this.fieldDate}
      ?disabled=${this.busy}
      @input=${this.handleFieldDate}
    ></wa-input>`;
    const name = (label: string) => html`<wa-input
      data-field-name
      label=${label}
      autocomplete="off"
      .value=${this.fieldName}
      ?disabled=${this.busy}
      @input=${this.handleFieldName}
    ></wa-input>`;
    const note = (label: string) => html`<wa-textarea
      data-field-note
      label=${label}
      maxlength=${NOTE_LIMIT}
      .value=${this.fieldNote}
      ?disabled=${this.busy}
      @input=${this.handleFieldNote}
    ></wa-textarea>`;
    const assent = html`<p class="meta" data-assent>${RECORDED_ASSENT}</p>`;

    switch (dialog) {
      case 'edit': {
        const kinds = this.kinds.some((kind) => kind.name === detail.kind)
          ? this.kinds.map((kind) => kind.name)
          : [detail.kind, ...this.kinds.map((kind) => kind.name)];
        return html`<wa-input
            data-field-title
            label="Title"
            .value=${this.fieldTitle}
            ?disabled=${this.busy}
            @input=${this.handleFieldTitle}
          ></wa-input>
          <wa-select
            data-field-kind
            label="Kind"
            value=${this.fieldKind}
            ?disabled=${this.busy}
            @change=${this.handleFieldKind}
          >
            ${kinds.map((kind) => html`<wa-option value=${kind}>${kind}</wa-option>`)}
          </wa-select>`;
      }
      case 'revise':
        return html`<wc-dropzone
          accept=".pdf"
          filename=${this.revision?.name ?? ''}
          .size=${this.revision?.size ?? 0}
          error=${this.revisionError}
          ?busy=${this.busy}
          @nc-file-select=${this.handleRevisionSelect}
          @nc-file-error=${this.handleRevisionError}
          @nc-file-clear=${this.handleRevisionClear}
        ></wc-dropzone>`;
      case 'accept':
        return html`${assent} ${name('Signer name')} ${date}`;
      case 'requestChanges':
        return html`${name('Requested by')} ${note('Changes requested')} ${date}`;
      case 'decline':
        return html`${note('Note (optional)')} ${date}`;
      case 'countersign':
        return html`${assent} ${name('Countersigned by')} ${date}`;
    }
  }

  private renderActionDialog(detail: DocumentDetail) {
    const dialog = this.dialog;
    if (!dialog) return nothing;

    const labels: Record<ActionDialog, { heading: string; confirm: string }> = {
      edit: { heading: 'Edit the document', confirm: 'Save' },
      revise: { heading: 'Revise with a new PDF', confirm: 'Revise…' },
      accept: { heading: 'Record an acceptance', confirm: 'Record…' },
      requestChanges: { heading: 'Record a change request', confirm: 'Record…' },
      decline: { heading: 'Record a decline', confirm: 'Record…' },
      countersign: { heading: 'Countersign', confirm: 'Countersign…' },
    };

    return html`
      <wc-manager-dialog
        open
        heading=${`${labels[dialog].heading} — “${detail.title}”`}
        confirm-label=${labels[dialog].confirm}
        ?busy=${this.busy}
        .error=${this.dialogError}
        @nc-manager-save=${this.handleDialogSave}
        @nc-manager-cancel=${this.closeDialog}
      >
        <div class="dialog-fields">${this.renderDialogFields(dialog, detail)}</div>
      </wc-manager-dialog>
    `;
  }

  private renderSendDialog(detail: DocumentDetail) {
    if (!this.sendOpen) return nothing;

    const errors = validateRecipients(this.recipients);
    const blocked = hasRecipientErrors(errors) ? 'Fix the recipients below before sending.' : '';

    return html`
      <wc-send-dialog
        open
        mode="document"
        .documentTitle=${detail.title}
        .responseForm=${this.documentsStatus?.responseForm ?? false}
        .recipientCount=${1 + this.recipients.collaborators.length}
        .previewHtml=${this.previewHtml}
        .previewError=${this.previewError}
        .previewLoading=${this.previewLoading}
        .pdfTarget=${this.client.documentPreviewTarget(detail.id)}
        .configCautions=${this.sendCautions}
        phase=${this.sendPhase}
        .steps=${this.sendSteps}
        .failure=${this.sendFailure}
        .configWarnings=${this.sendWarnings}
        .pageLinks=${this.sendLinks.map((link) => ({
          label: pageLinkLabel(link),
          href: link.url,
        }))}
        .blocked=${blocked}
        @nc-send-confirm=${this.handleSend}
        @nc-send-close=${this.closeSend}
      >
        <wc-recipient-editor
          slot="recipients"
          .value=${this.recipients}
          .contacts=${this.contacts}
          .errors=${errors}
          ?disabled=${this.sendPhase !== 'confirm'}
          @nc-recipients-change=${this.handleRecipients}
        ></wc-recipient-editor>
      </wc-send-dialog>
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
