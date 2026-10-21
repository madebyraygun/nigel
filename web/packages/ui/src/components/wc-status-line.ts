import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import { controlsCss } from '@nigel/theme';
import { dispatchPaletteOpen } from './wc-breadcrumb.js';
import './wc-monogram.js';
import '../icons/icons.js';

/** What the status line says about the open book beyond its name. */
export interface StatusLineInfo {
  /** The folder as shown, home written as `~`. */
  path: string;
  /** The full folder, for the tooltip. */
  fullPath?: string;
  encrypted: boolean;
  version: string;
  /** `store` omits the version. Absent on a server that does not report it. */
  distribution?: string;
}

/**
 * The floating status line under the content: the open book, where it lives
 * on disk, whether it is encrypted, what is waiting in Review, the palette
 * hint and the version.
 *
 * Presentational, like the rest of the shell: the book opens the palette
 * scoped to books and the palette hint opens it unscoped (`nc-palette-open`);
 * the review count asks to go to Review with `nc-navigate`. A store build
 * (`distribution="store"`) has no version segment, because the store owns
 * versioning. The line is a desktop furnishing and stands down below the
 * mobile breakpoint, where the drawer's book card takes its place.
 */
@customElement('wc-status-line')
export class WcStatusLine extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: flex;
        align-items: stretch;
        flex: none;
        height: 34px;
        margin: 0 var(--nc-window-inset) var(--nc-window-inset);
        box-sizing: border-box;
        background: var(--nc-color-sidebar-bg);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-lg);
        box-shadow: var(--nc-shadow-floating);
        overflow: hidden;
        color: var(--wa-color-muted);
        font-family: var(--wa-font-family-sans);
        font-size: var(--nc-font-size-caption);
        white-space: nowrap;
        container-type: inline-size;
      }

      .segment {
        display: inline-flex;
        align-items: center;
        gap: 6px;
        flex: none;
        padding: 0 12px;
        border: 0;
        border-right: 1px solid var(--wa-color-border-soft);
        background: transparent;
        color: inherit;
        font: inherit;
        cursor: default;
      }

      button.segment:focus-visible {
        outline-offset: -2px;
        border-radius: var(--wa-radius-sm);
      }

      .spacer {
        flex: 1;
        min-width: 0;
      }

      .spacer ~ .segment {
        border-right: 0;
        border-left: 1px solid var(--wa-color-border-soft);
      }

      .brand {
        display: inline-flex;
        align-items: center;
        flex: none;
        margin: 4px 2px 4px 4px;
        padding: 0 10px;
        border-radius: var(--wa-radius-sm);
        background: var(--nc-grad-pastel);
        color: var(--nc-color-on-gradient);
        font-weight: var(--wa-font-weight-semibold);
      }

      button.segment {
        transition: background var(--nc-transition-fast);
      }

      button.segment:hover {
        background: var(--nc-color-hover);
      }

      /* Short of room the book's name gives way first, then the path; the
         monogram still says which book it is. */
      .book {
        flex: 0 8 auto;
        min-width: 0;
        color: var(--wa-color-text);
      }

      .book wc-monogram {
        --nc-monogram-size: 14px;
      }

      .book wc-monogram::part(tile) {
        border-radius: var(--wa-radius-xs);
        font-size: 7px;
        font-weight: var(--wa-font-weight-semibold);
      }

      .book-name {
        overflow: hidden;
        text-overflow: ellipsis;
        max-width: 32ch;
      }

      .path {
        flex: 0 1 auto;
        min-width: 0;
        display: block;
        line-height: 32px;
        overflow: hidden;
        text-overflow: ellipsis;
        user-select: text;
        cursor: text;
      }

      /* The lock's label gives way after the book's name and before the path. */
      .lock {
        position: relative;
        flex: 0 4 auto;
        min-width: 0;
      }

      .lock .label {
        overflow: hidden;
        text-overflow: ellipsis;
      }

      .lock wc-icon-lock,
      .lock wc-icon-lock-open {
        --nc-icon-size: 12px;
      }

      .review wc-icon-status-overdue {
        --nc-icon-size: 9px;
      }

      .review {
        color: var(--nc-color-flagged);
      }

      kbd {
        font-family: inherit;
      }

      /* The version goes first when the line runs out of room, then the palette
         hint, so nothing else is clipped. */
      @container (max-width: 760px) {
        .version {
          display: none;
        }
      }

      @container (max-width: 620px) {
        .hint {
          display: none;
        }
      }

      /* Narrower still, the lock is its icon alone; the label stays in the
         accessibility tree and the tooltip. */
      @container (max-width: 560px) {
        .lock .label {
          position: absolute;
          width: 1px;
          height: 1px;
          overflow: hidden;
          clip-path: inset(50%);
        }
      }

      @media (max-width: 48rem) {
        :host {
          display: none;
        }
      }

      @media print {
        :host {
          display: none;
        }
      }
    `,
  ];

  /** The open book's name. */
  @property({ type: String, attribute: 'book-name' })
  bookName = '';

  /** What picks the monogram's colour: something that names the book for good. */
  @property({ type: String, attribute: 'book-key' })
  bookKey = '';

  /** The folder the book lives in, as shown: the home directory written as `~`. */
  @property({ type: String })
  path = '';

  /** The unabbreviated folder, for the path's tooltip. Falls back to `path`. */
  @property({ type: String, attribute: 'full-path' })
  fullPath = '';

  @property({ type: Boolean })
  encrypted = false;

  /** Transactions waiting in Review. Zero or unknown hides the segment. */
  @property({ type: Number, attribute: 'review-count' })
  reviewCount: number | undefined = undefined;

  /** The palette hint's chord, as the keycap spells it on this platform. */
  @property({ type: String })
  shortcut = '⌘K';

  @property({ type: String, attribute: 'key-shortcuts' })
  keyShortcuts = 'Meta+K';

  @property({ type: String })
  version = '';

  /** How this build was distributed. `store` omits the version. */
  @property({ type: String })
  distribution = '';

  private goToReview(): void {
    this.dispatchEvent(
      new CustomEvent<{ id: string }>('nc-navigate', {
        detail: { id: 'review' },
        bubbles: true,
        composed: true,
      }),
    );
  }

  render() {
    const review = this.reviewCount && this.reviewCount > 0 ? this.reviewCount : 0;
    const showVersion = this.version !== '' && this.distribution !== 'store';
    const lock = this.encrypted
      ? html`<wc-icon-lock></wc-icon-lock><span class="label">encrypted</span>`
      : html`<wc-icon-lock-open></wc-icon-lock-open><span class="label">not encrypted</span>`;
    return html`
      <span class="brand" aria-hidden="true">nigel</span>
      ${
        this.bookName
          ? html`<button
              type="button"
              class="segment book"
              aria-haspopup="dialog"
              title="Switch books"
              @click=${() => dispatchPaletteOpen(this, 'books')}
            >
              <wc-monogram name=${this.bookName} seed=${this.bookKey}></wc-monogram>
              <span class="book-name">${this.bookName}</span>
            </button>`
          : nothing
      }
      ${
        this.path
          ? html`<span class="segment path" title=${this.fullPath || this.path}>${this.path}</span>`
          : nothing
      }
      <span class="segment lock" title=${this.encrypted ? 'Encrypted' : 'Not encrypted'}
        >${lock}</span
      >
      <span class="spacer"></span>
      ${
        review
          ? html`<button type="button" class="segment review" @click=${this.goToReview}>
              <wc-icon-status-overdue></wc-icon-status-overdue>${review} to review
            </button>`
          : nothing
      }
      <button
        type="button"
        class="segment hint"
        aria-haspopup="dialog"
        aria-keyshortcuts=${this.keyShortcuts}
        @click=${() => dispatchPaletteOpen(this, 'all')}
      >
        <kbd>${this.shortcut}</kbd> palette
      </button>
      ${
        showVersion
          ? html`<span class="segment version">v${this.version.replace(/^v/, '')}</span>`
          : nothing
      }
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-status-line': WcStatusLine;
  }
}
