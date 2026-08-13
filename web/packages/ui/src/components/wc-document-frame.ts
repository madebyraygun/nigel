import { LitElement, html, css, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import './wc-spinner.js';

/**
 * The sandbox a framed document is shown in.
 *
 * `allow-same-origin` is deliberately absent. The preview is served from the
 * SPA's own origin, so granting it would put a page rendered from invoice data
 * back inside the app's origin with access to its cookies and storage. Without
 * it the iframe is an opaque origin and the containment is real, which is what
 * the route's `Content-Security-Policy: sandbox` header says independently.
 * The route answers `X-Frame-Options: SAMEORIGIN` (overriding the blanket
 * `DENY`) purely so the frame is allowed to exist at all.
 *
 * It matters more for `srcdoc` than for `src`: the header does not apply to a
 * document the parent supplies, so the attribute has always been the real
 * control.
 */
export const PREVIEW_SANDBOX = 'allow-popups allow-popups-to-escape-sandbox';

/**
 * One document in a sandboxed frame, with a spinner until it loads.
 *
 * It owns the iframe and nothing else, so there is one sandbox constant and one
 * loading state in the app: `wc-invoice-preview` frames a `src` behind its
 * disclosure, and the send dialog frames HTML it already holds as `srcdoc` —
 * because an iframe cannot report a failure, and a broken custom template has
 * to arrive as a sentence rather than as a JSON envelope rendered in a box.
 */
@customElement('wc-document-frame')
export class WcDocumentFrame extends LitElement {
  static styles = css`
    :host {
      display: block;
    }

    .frame {
      position: relative;
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-sm, 6px);
      overflow: hidden;
      background: var(--wa-color-surface, #fff);
    }

    iframe {
      display: block;
      width: 100%;
      height: var(--nc-document-frame-height, 32rem);
      border: 0;
    }

    .loading {
      position: absolute;
      inset: 0;
      display: flex;
      align-items: center;
      justify-content: center;
      background: var(--wa-color-surface, #fff);
    }
  `;

  /** The document itself, when the caller already holds it. Wins over `src`. */
  @property({ type: String })
  srcdoc = '';

  /** An address to fetch instead, for a document nobody has read yet. */
  @property({ type: String })
  src = '';

  /** What a screen reader calls this frame. */
  @property({ type: String })
  label = 'Document preview';

  /** Overrides `--nc-document-frame-height` for this instance. */
  @property({ type: String })
  height = '';

  @state() private loaded = false;

  protected willUpdate(changed: Map<string, unknown>): void {
    // A new document is a new load: the spinner comes back rather than sitting
    // under whatever the last one left on screen.
    if (changed.has('srcdoc') || changed.has('src')) this.loaded = false;
  }

  private handleLoad = (): void => {
    this.loaded = true;
  };

  render() {
    const style = this.height ? `--nc-document-frame-height: ${this.height}` : nothing;
    return html`
      <div class="frame" style=${style}>
        ${this.srcdoc
          ? html`<iframe
              data-frame
              title=${this.label}
              srcdoc=${this.srcdoc}
              sandbox=${PREVIEW_SANDBOX}
              @load=${this.handleLoad}
            ></iframe>`
          : html`<iframe
              data-frame
              title=${this.label}
              src=${this.src}
              sandbox=${PREVIEW_SANDBOX}
              @load=${this.handleLoad}
            ></iframe>`}
        ${this.loaded
          ? nothing
          : html`<div class="loading" data-loading>
              <wc-spinner show-label label="Rendering the document"></wc-spinner>
            </div>`}
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-document-frame': WcDocumentFrame;
  }
}
