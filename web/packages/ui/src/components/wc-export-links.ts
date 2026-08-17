import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '../icons/icons.js';

/**
 * The Text and PDF export controls for one report.
 *
 * Plain download links, not buttons that fetch: the browser streams the file,
 * the session cookie rides along on a same-origin navigation, and the server's
 * `Content-Disposition` names it — the same filename `nigel report … --mode
 * export` would write.
 *
 * The PDF control has to be able to say "not in this build". PDF export is a
 * compile-time cargo feature, so a binary built without it answers `501`, and a
 * download link cannot inspect a response: the browser would cheerfully save an
 * error envelope as `pnl.pdf`. The screen therefore reads `pdfExport` from
 * `/api/status` and turns this off, and off means a disabled control with the
 * reason in text beside it rather than a link that lies.
 */
/**
 * How this component reaches an export.
 *
 * Declared here rather than imported: `@nigel/ui` does not depend on the app.
 * The api client produces the same shape.
 */
export type ExportTarget =
  | { kind: 'href'; href: string }
  | { kind: 'action'; run: () => Promise<void>; filename: string };

@customElement('wc-export-links')
export class WcExportLinks extends LitElement {
  static styles = css`
    :host {
      display: inline-flex;
      align-items: center;
      gap: var(--wa-space-s, 8px);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
      flex-wrap: wrap;
    }

    a,
    button {
      font: inherit;
      font-size: var(--wa-font-size-s, 13px);
      display: inline-flex;
      align-items: center;
      gap: var(--wa-space-2xs, 4px);
      padding: var(--wa-space-2xs, 4px) var(--wa-space-s, 8px);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-sm, 6px);
      background: var(--wa-color-surface);
      color: inherit;
      text-decoration: none;
      cursor: pointer;
    }

    a:hover,
    button:hover:not(:disabled) {
      background: var(--wa-color-surface-alt, rgba(0, 0, 0, 0.03));
    }

    a:focus-visible,
    button:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
    }

    button:disabled,
    a[aria-disabled='true'] {
      opacity: 0.5;
      cursor: default;
    }

    .reason {
      margin: 0;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
      max-width: 28rem;
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
  `;

  /** Built by the api client — a screen never spells an endpoint itself. */
  @property({ type: String, attribute: 'text-href' })
  textHref = '';

  @property({ type: String, attribute: 'pdf-href' })
  pdfHref = '';

  /** Wins over `textHref` when set. */
  @property({ attribute: false })
  textTarget: ExportTarget | null = null;

  /** Wins over `pdfHref` when set. */
  @property({ attribute: false })
  pdfTarget: ExportTarget | null = null;

  /** False when the server reports a build without the pdf feature. */
  @property({ type: Boolean, attribute: 'pdf-available' })
  pdfAvailable = true;

  @property({ type: String, attribute: 'pdf-unavailable-reason' })
  pdfUnavailableReason =
    'This build of nigel was compiled without PDF export. Text export still works.';

  /** Reserved for a report that has not loaded yet: nothing to export. */
  @property({ type: Boolean, reflect: true })
  busy = false;

  private renderPdf() {
    if (!this.pdfAvailable) {
      return html`
        <button type="button" disabled aria-describedby="pdf-reason">
          <wc-icon-download></wc-icon-download>
          PDF
        </button>
        <p class="reason" id="pdf-reason">${this.pdfUnavailableReason}</p>
      `;
    }

    return this.renderTarget('pdf', 'PDF', this.pdfTarget, this.pdfHref);
  }

  /** A busy link stays in the tab order but does not fire a half-built request. */
  private blockWhileBusy = (event: Event): void => {
    if (this.busy) event.preventDefault();
  };

  private runAction = async (event: Event, target: ExportTarget): Promise<void> => {
    event.preventDefault();
    if (this.busy || target.kind !== 'action') return;
    await target.run();
  };

  /**
   * An anchor where the platform can download from a link, a button where it
   * cannot. Both carry the same label, so the accessible name does not depend
   * on which platform is running.
   */
  private renderTarget(
    slot: 'text' | 'pdf',
    label: string,
    target: ExportTarget | null,
    href: string,
  ) {
    if (target?.kind === 'action') {
      return html`
        <button
          type="button"
          data-export=${slot}
          ?disabled=${this.busy}
          @click=${(event: Event) => this.runAction(event, target)}
        >
          <wc-icon-download></wc-icon-download>
          ${label}
        </button>
      `;
    }

    return html`
      <a
        href=${target?.kind === 'href' ? target.href : href}
        download
        aria-disabled=${this.busy ? 'true' : nothing}
        @click=${this.blockWhileBusy}
      >
        <wc-icon-download></wc-icon-download>
        ${label}
      </a>
    `;
  }

  render() {
    return html`
      ${this.renderTarget('text', 'Text', this.textTarget, this.textHref)}
      ${this.renderPdf()}
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-export-links': WcExportLinks;
  }
}
