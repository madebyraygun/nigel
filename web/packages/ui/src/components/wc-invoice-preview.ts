import { LitElement, html, css, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import './wc-notice-bar.js';
import './wc-document-frame.js';
import type { ExportTarget } from './wc-export-links.js';
import { dispatchNcToast } from './wc-toast.js';

/**
 * The sandbox a framed document is shown in.
 *
 * It lives in `wc-document-frame`, which owns every iframe in the app; this
 * re-export is the name the rest of the codebase already imports it by.
 */
export { PREVIEW_SANDBOX } from './wc-document-frame.js';

/**
 * A word on why a save failed, short enough to read as a reason rather than
 * a stack trace. Empty when the error has no message or the message is long.
 */
const MAX_DETAIL_LENGTH = 120;

function failureDetail(error: unknown): string {
  const detail = error instanceof Error ? error.message : String(error);
  return detail.length > 0 && detail.length <= MAX_DETAIL_LENGTH ? detail : '';
}

/**
 * The invoice page as the client will see it, behind a disclosure.
 *
 * Collapsed by default and opened with one click: it is a second render of the
 * whole document per detail view, and expanded it makes the screen tall enough
 * to push the actions and the payment history off the first screenful. The
 * iframe is not created until it is opened, so a closed preview costs no
 * request at all.
 */
@customElement('wc-invoice-preview')
export class WcInvoicePreview extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    details {
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-m, 8px);
    }

    summary {
      padding: var(--wa-space-s, 8px) var(--wa-space-m, 12px);
      cursor: default;
      font-weight: var(--wa-font-weight-medium, 500);
    }

    summary:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
    }

    .body {
      display: grid;
      gap: var(--wa-space-s, 8px);
      padding: 0 var(--wa-space-m, 12px) var(--wa-space-m, 12px);
    }

    wc-document-frame {
      --nc-document-frame-height: var(--nc-invoice-preview-height, 32rem);
    }

    .links {
      display: flex;
      flex-wrap: wrap;
      gap: var(--wa-space-m, 12px);
      font-size: var(--wa-font-size-s, 13px);
    }

    a,
    button[data-pdf-link] {
      color: var(--wa-color-brand);
    }

    button[data-pdf-link] {
      font: inherit;
      background: none;
      border: none;
      padding: 0;
      cursor: default;
    }

    button[data-pdf-link]:disabled {
      opacity: 0.5;
      cursor: default;
    }

    .unavailable {
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
    }
  `;

  /** The HTML preview address, from `ApiClient.invoicePreviewUrl`. */
  @property({ type: String })
  src = '';

  /** The PDF address. Offered only when `pdfAvailable`. */
  @property({ type: String, attribute: 'pdf-src' })
  pdfSrc = '';

  /** Wins over `pdfSrc` when set. */
  @property({ attribute: false })
  pdfTarget: ExportTarget | null = null;

  /**
   * Whether this build of the server can render a PDF.
   *
   * A download link cannot inspect what comes back, so without this the PDF
   * link on a build without the `pdf` feature would save a 501 envelope.
   */
  @property({ type: Boolean, attribute: 'pdf-available' })
  pdfAvailable = true;

  /** Unset invoicing keys, by name, from `/api/status`. Never their values. */
  @property({ attribute: false })
  missing: string[] = [];

  /** Where the missing-configuration notice points. */
  @property({ type: String, attribute: 'settings-href' })
  settingsHref = '#/settings';

  @property({ type: Boolean, reflect: true })
  open = false;

  private handleToggle = (event: Event): void => {
    const details = event.currentTarget as HTMLDetailsElement;
    this.open = details.open;
  };

  /** Guards a second click while the first save is still running. */
  @state()
  private downloadingPdf = false;

  /** The same treatment `wc-export-links` gives its links, for the one here. */
  private renderPdfLink() {
    const target = this.pdfTarget;
    if (target?.kind === 'action') {
      return html`<button
        type="button"
        data-pdf-link
        ?disabled=${this.downloadingPdf}
        @click=${(event: Event) => this.runPdfDownload(event, target)}
      >
        Download the PDF
      </button>`;
    }
    const href = target?.kind === 'href' ? target.href : this.pdfSrc;
    return html`<a href=${href} data-pdf-link download>Download the PDF</a>`;
  }

  /**
   * `run` throws on failure rather than reporting it itself, so a full disk
   * or a read-only destination has to be surfaced here — otherwise the
   * button un-busies and the operator believes the PDF saved.
   */
  private runPdfDownload = async (
    event: Event,
    target: Extract<ExportTarget, { kind: 'action' }>,
  ): Promise<void> => {
    event.preventDefault();
    if (this.downloadingPdf) return;
    this.downloadingPdf = true;
    try {
      await target.run();
    } catch (error) {
      const detail = failureDetail(error);
      dispatchNcToast(this, {
        message: detail ? `Couldn't save the PDF. ${detail}` : "Couldn't save the PDF.",
        variant: 'danger',
      });
    } finally {
      this.downloadingPdf = false;
    }
  };

  render() {
    return html`
      <details ?open=${this.open} @toggle=${this.handleToggle}>
        <summary data-toggle>Preview</summary>
        <div class="body">
          ${this.missing.length > 0
            ? html`
                <wc-notice-bar
                  variant="warning"
                  data-missing
                  message=${`${this.missing.join(', ')} ${
                    this.missing.length === 1 ? 'is' : 'are'
                  } not set, so this invoice cannot be sent. The preview below still renders.`}
                ></wc-notice-bar>
              `
            : nothing}
          ${this.open ? this.renderFrame() : nothing}
          <div class="links">
            ${this.src
              ? html`<a href=${this.src} target="_blank" rel="noreferrer" data-html-link
                  >Open the HTML page</a
                >`
              : nothing}
            ${this.pdfAvailable
              ? this.renderPdfLink()
              : html`<span class="unavailable" data-pdf-unavailable
                  >PDF export is not available in this build.</span
                >`}
          </div>
        </div>
      </details>
    `;
  }

  private renderFrame() {
    // Created only once the disclosure is open, so a closed preview costs no
    // request at all — the lazy rule stays here, the iframe does not.
    return html`<wc-document-frame
      data-frame
      .src=${this.src}
      label="Invoice preview"
    ></wc-document-frame>`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-invoice-preview': WcInvoicePreview;
  }
}
