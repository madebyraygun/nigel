import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import { controlsCss } from '@nigel/theme';
import './wc-monogram.js';
import '../icons/icons.js';

/** One level of the trail after the book. */
export interface BreadcrumbItem {
  label: string;
  /**
   * The screen this level returns to. A level with an id is a button; the
   * last level is where the user is and never is one.
   */
  id?: string;
}

/** Which part of the palette a control asks for. */
export type PaletteScope = 'books' | 'all';

export interface NcPaletteOpenDetail {
  scope: PaletteScope;
}

/** Ask for the palette. Bubbles and composes, so it reaches the document. */
export function dispatchPaletteOpen(from: EventTarget, scope: PaletteScope): void {
  from.dispatchEvent(
    new CustomEvent<NcPaletteOpenDetail>('nc-palette-open', {
      detail: { scope },
      bubbles: true,
      composed: true,
    }),
  );
}

/**
 * Where the user is: the open book, then the screen, then anything below it.
 *
 * `[monogram] Book name ▾ / Screen`, or `… / Invoices / #1254` a level down.
 * The book is a button that asks for the palette scoped to books
 * (`nc-palette-open`); an earlier level is a button that navigates back to
 * its screen (`nc-navigate`), the same event the sidebar sends.
 */
@customElement('wc-breadcrumb')
export class WcBreadcrumb extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: block;
        min-width: 0;
        font-family: var(--wa-font-family-sans);
        font-size: var(--nc-font-size-control);
      }

      ol {
        display: flex;
        align-items: center;
        min-width: 0;
        margin: 0;
        padding: 0;
        list-style: none;
      }

      li {
        display: flex;
        align-items: center;
        min-width: 0;
      }

      /* Short of room, the book's name gives way before the screen's: the
         monogram still says which book it is. */
      li + li {
        flex-shrink: 0;
      }

      button {
        display: inline-flex;
        align-items: center;
        min-width: 0;
        height: 30px;
        border: 0;
        border-radius: var(--wa-radius-md);
        background: transparent;
        font: inherit;
        cursor: default;
        transition: background var(--nc-transition-fast);
      }

      button:hover {
        background: var(--nc-color-hover);
      }

      .book {
        gap: var(--wa-space-s);
        padding: 0 6px 0 4px;
        color: var(--wa-color-text);
      }

      .book-name,
      .level {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }

      .book wc-icon-chevron-down {
        --nc-icon-size: 14px;
        color: var(--wa-color-muted);
      }

      .sep {
        padding: 0 6px;
        color: var(--wa-color-muted);
      }

      .level {
        color: var(--wa-color-muted);
      }

      button.level {
        padding: 0 4px;
      }

      /* The last level of a deeper trail is the thing on screen, and reads
         as text; a lone screen name stays muted beside the book. */
      .level.current.deep {
        color: var(--wa-color-text);
        font-weight: var(--wa-font-weight-medium);
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

  /** The levels after the book, outermost first. */
  @property({ attribute: false })
  trail: BreadcrumbItem[] = [];

  private navigate(id: string): void {
    this.dispatchEvent(
      new CustomEvent<{ id: string }>('nc-navigate', {
        detail: { id },
        bubbles: true,
        composed: true,
      }),
    );
  }

  render() {
    const last = this.trail.length - 1;
    const deep = this.trail.length > 1;
    return html`
      <nav aria-label="Breadcrumb">
        <ol>
          <li>
            <button
              type="button"
              class="book"
              aria-haspopup="dialog"
              title="Switch books"
              @click=${() => dispatchPaletteOpen(this, 'books')}
            >
              <wc-monogram name=${this.bookName} seed=${this.bookKey}></wc-monogram>
              <span class="book-name">${this.bookName}</span>
              <wc-icon-chevron-down></wc-icon-chevron-down>
            </button>
          </li>
          ${this.trail.map((item, index) => {
            const current = index === last;
            const classes = `level${current ? ' current' : ''}${deep ? ' deep' : ''}`;
            return html`
              <li>
                <span class="sep" aria-hidden="true">/</span>
                ${!current && item.id
                  ? html`<button
                      type="button"
                      class=${classes}
                      data-level=${item.id}
                      @click=${() => this.navigate(item.id!)}
                    >
                      ${item.label}
                    </button>`
                  : html`<span
                      class=${classes}
                      aria-current=${current ? 'page' : nothing}
                      >${item.label}</span
                    >`}
              </li>
            `;
          })}
        </ol>
      </nav>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-breadcrumb': WcBreadcrumb;
  }
  interface HTMLElementEventMap {
    'nc-palette-open': CustomEvent<NcPaletteOpenDetail>;
  }
}
