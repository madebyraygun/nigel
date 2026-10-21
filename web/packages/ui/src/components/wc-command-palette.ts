import { LitElement, html, css, nothing, type PropertyValues } from 'lit';
import { customElement, property, query, state } from 'lit/decorators.js';
import { classMap } from 'lit/directives/class-map.js';
import type { PaletteScope } from './wc-breadcrumb.js';
import './wc-monogram.js';
import '../icons/icons.js';

/** A set of books as the palette lists it. */
export interface PaletteBook {
  /** Stable identity; also what picks the monogram's colour. */
  id: string;
  name: string;
  /** `Business` or `Personal`, as printed. */
  kind: string;
  /** The folder the books live in. */
  path: string;
  encrypted: boolean;
  /** The book this window has open; marked OPEN and listed as `Open now`. */
  open?: boolean;
  /** When it was last opened, already formatted (`Yesterday`, `Sep 28`). */
  lastOpened?: string;
}

/** A screen the palette can go to. */
export interface PaletteScreen {
  id: string;
  label: string;
  /** Tag name of a `wc-icon-*` element. */
  icon: string;
}

export interface NcPaletteSelectDetail {
  kind: 'book' | 'screen';
  id: string;
}

type Row =
  | { kind: 'book'; id: string; book: PaletteBook }
  | { kind: 'screen'; id: string; screen: PaletteScreen };

function matches(query: string, ...fields: string[]): boolean {
  return fields.some((field) => field.toLowerCase().includes(query));
}

/** The element that really has focus, through any number of shadow roots. */
function deepActiveElement(): Element | null {
  let active = document.activeElement;
  while (active?.shadowRoot?.activeElement) active = active.shadowRoot.activeElement;
  return active;
}

/**
 * The ⌘K palette: one search over the books and the screens.
 *
 * A modal dialog holding a combobox and its listbox. Focus stays in the
 * input the whole time it is open — the options are reached with
 * `aria-activedescendant`, not focus, and a press anywhere else in the panel
 * is kept from taking it — so Tab has nowhere to go and is held there. On
 * close, focus goes back to `opener`, or to whatever had it when the palette
 * opened if no opener was given.
 * The host fixes itself over the viewport, backdrop included; a caller that
 * wants the page behind it out of the tab order as well makes it `inert`.
 *
 * It knows nothing about routing or which books exist: it is handed both and
 * answers with `nc-palette-select` (`{ kind, id }`) and `nc-palette-close`.
 * Opening and closing are the caller's, through `open`.
 */
@customElement('wc-command-palette')
export class WcCommandPalette extends LitElement {
  static styles = css`
    :host {
      display: none;
      position: fixed;
      inset: 0;
      z-index: 800;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    :host([open]) {
      display: block;
    }

    .backdrop {
      position: absolute;
      inset: 0;
      background: var(--wa-color-overlay-modal);
    }

    .panel {
      position: absolute;
      top: 76px;
      left: 50%;
      transform: translateX(-50%);
      width: 620px;
      max-width: calc(100% - 48px);
      display: flex;
      flex-direction: column;
      overflow: hidden;
      background: var(--wa-color-surface);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-lg);
      box-shadow: var(--wa-shadow-lg);
    }

    .search {
      display: flex;
      align-items: center;
      gap: var(--wa-space-s);
      height: 54px;
      padding: 0 14px 0 16px;
      flex: none;
    }

    .search wc-icon-search {
      --nc-icon-size: 18px;
      color: var(--wa-color-muted);
    }

    input {
      flex: 1;
      min-width: 0;
      border: 0;
      outline: none;
      background: transparent;
      color: var(--wa-color-text);
      font: inherit;
      font-size: 15px;
      user-select: text;
      -webkit-user-select: text;
    }

    input::placeholder {
      color: var(--wa-color-muted);
    }

    kbd {
      padding: 1px 6px;
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-xs);
      background: var(--wa-color-surface-alt);
      color: var(--wa-color-muted);
      font-family: inherit;
      font-size: var(--nc-font-size-label);
      line-height: 16px;
    }

    .results {
      max-height: 430px;
      overflow-y: auto;
      overscroll-behavior: contain;
      padding: 6px;
      border-top: 1px solid var(--wa-color-border);
    }

    .section + .section {
      margin-top: 6px;
    }

    .heading {
      padding: 8px 10px 6px;
      color: var(--wa-color-muted);
      font-size: 11px;
      font-weight: var(--wa-font-weight-medium);
      letter-spacing: var(--nc-letter-spacing-label);
      text-transform: uppercase;
    }

    .option {
      display: grid;
      align-items: center;
      gap: 12px;
      padding: 8px 10px;
      border-radius: var(--wa-radius-md);
      cursor: default;
    }

    .option.active {
      background: var(--nc-color-highlight);
    }

    .book {
      grid-template-columns: 32px minmax(0, 1fr) auto;
    }

    .screen {
      grid-template-columns: 16px minmax(0, 1fr) auto;
      font-size: var(--nc-font-size-body);
    }

    .screen > :first-child {
      --nc-icon-size: 16px;
      color: var(--wa-color-muted);
    }

    .name {
      display: flex;
      align-items: center;
      gap: var(--wa-space-xs);
      font-size: 14px;
      font-weight: var(--wa-font-weight-medium);
    }

    .badge {
      padding: 0 4px;
      border: 1px solid var(--wa-color-brand);
      border-radius: var(--wa-radius-xs);
      color: var(--wa-color-brand);
      font-size: var(--nc-font-size-label);
      font-weight: var(--wa-font-weight-medium);
      letter-spacing: 0.04em;
      line-height: 15px;
      text-transform: uppercase;
    }

    .sub {
      overflow: hidden;
      color: var(--wa-color-muted);
      font-size: var(--nc-font-size-meta);
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .when {
      display: flex;
      align-items: center;
      justify-content: flex-end;
      gap: var(--wa-space-xs);
      min-width: 72px;
      color: var(--wa-color-muted);
      font-size: var(--nc-font-size-meta);
    }

    .when wc-icon-lock {
      --nc-icon-size: 14px;
    }

    .tag {
      color: var(--wa-color-muted);
      font-size: var(--nc-font-size-caption);
    }

    .empty {
      padding: 28px 16px;
      color: var(--wa-color-muted);
      font-size: var(--nc-font-size-body);
      text-align: center;
    }

    footer {
      display: flex;
      align-items: center;
      gap: 14px;
      height: 34px;
      padding: 0 14px;
      flex: none;
      border-top: 1px solid var(--wa-color-border);
      background: var(--wa-color-surface-alt);
      color: var(--wa-color-muted);
      font-size: var(--nc-font-size-caption);
    }

    footer kbd {
      padding: 0;
      border: 0;
      background: none;
      font-size: inherit;
    }

    .wordmark {
      margin-left: auto;
      font-weight: var(--wa-font-weight-bold);
      background: var(--nc-grad-brand-text);
      -webkit-background-clip: text;
      background-clip: text;
      color: transparent;
    }

    .sr-only {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    @media print {
      :host {
        display: none !important;
      }
    }
  `;

  @property({ type: Boolean, reflect: true })
  open = false;

  /** `books` lists only the books; `all` adds the screens. */
  @property({ type: String })
  scope: PaletteScope = 'all';

  @property({ attribute: false })
  books: PaletteBook[] = [];

  @property({ attribute: false })
  screens: PaletteScreen[] = [];

  /** What is typed. Cleared on close, so the palette opens empty. */
  @property({ type: String })
  query = '';

  /**
   * Where focus returns on close. A caller that knows which control asked
   * passes it: WebKit does not focus a button on click, so what had focus
   * then is not always the control that was pressed.
   */
  @property({ attribute: false })
  opener: HTMLElement | null = null;

  @state()
  private active = 0;

  @query('input')
  private input!: HTMLInputElement | null;

  private returnTo: HTMLElement | null = null;

  protected willUpdate(changed: PropertyValues<this>): void {
    if (changed.has('open')) {
      if (this.open) {
        const active = deepActiveElement();
        this.returnTo = this.opener ?? (active instanceof HTMLElement ? active : null);
      } else if (changed.get('open') === true) {
        this.query = '';
      }
    }
    if (changed.has('query') || changed.has('scope') || changed.has('open')) this.active = 0;
  }

  protected updated(changed: PropertyValues<this>): void {
    if (!changed.has('open')) return;
    if (this.open) {
      this.input?.focus();
    } else if (changed.get('open') === true) {
      const target = this.returnTo;
      this.returnTo = null;
      if (target?.isConnected) target.focus();
    }
  }

  private get rows(): { books: Row[]; screens: Row[] } {
    const q = this.query.trim().toLowerCase();
    const books: Row[] = this.books
      .filter((book) => !q || matches(q, book.name, book.kind, book.path))
      .map((book) => ({ kind: 'book', id: book.id, book }));
    const screens: Row[] =
      this.scope === 'books'
        ? []
        : this.screens
            .filter((screen) => !q || matches(q, screen.label))
            .map((screen) => ({ kind: 'screen', id: screen.id, screen }));
    return { books, screens };
  }

  private optionId(index: number): string {
    return `option-${index}`;
  }

  private close(): void {
    this.dispatchEvent(new CustomEvent('nc-palette-close', { bubbles: true, composed: true }));
  }

  private activate(row: Row | undefined): void {
    if (!row) return;
    this.dispatchEvent(
      new CustomEvent<NcPaletteSelectDetail>('nc-palette-select', {
        detail: { kind: row.kind, id: row.id },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handleKeydown = (event: KeyboardEvent): void => {
    // Mid-composition, Enter and the arrows belong to the IME.
    if (event.isComposing || event.keyCode === 229) return;
    const { books, screens } = this.rows;
    const flat = [...books, ...screens];
    switch (event.key) {
      case 'Escape':
        event.preventDefault();
        event.stopPropagation();
        this.close();
        return;
      case 'Tab':
        // The input is the dialog's only stop; Tab would otherwise walk out.
        event.preventDefault();
        return;
      case 'ArrowDown':
      case 'ArrowUp': {
        event.preventDefault();
        if (flat.length === 0) return;
        const step = event.key === 'ArrowDown' ? 1 : -1;
        this.active = (this.active + step + flat.length) % flat.length;
        void this.updateComplete.then(() =>
          this.renderRoot.querySelector(`#${this.optionId(this.active)}`)?.scrollIntoView?.({
            block: 'nearest',
          }),
        );
        return;
      }
      case 'Enter':
        event.preventDefault();
        this.activate(flat[this.active]);
        return;
    }
  };

  /** A press on a heading, a row or the footer would otherwise take focus to the body. */
  private keepFocus = (event: MouseEvent): void => {
    if (event.target !== this.input) event.preventDefault();
  };

  private handleInput = (event: Event): void => {
    this.query = (event.target as HTMLInputElement).value;
  };

  render() {
    if (!this.open) return nothing;
    const { books, screens } = this.rows;
    const count = books.length + screens.length;
    const placeholder = this.scope === 'books' ? 'Search books…' : 'Search books and screens…';
    const label = this.scope === 'books' ? 'Books' : 'Search books and screens';
    const status =
      count === 0 ? `Nothing matches "${this.query}".` : `${count} result${count === 1 ? '' : 's'}`;

    return html`
      <div class="backdrop" @click=${() => this.close()}></div>
      <div
        class="panel"
        role="dialog"
        aria-modal="true"
        aria-label=${label}
        @mousedown=${this.keepFocus}
        @keydown=${this.handleKeydown}
      >
        <div class="search">
          <wc-icon-search></wc-icon-search>
          <input
            type="text"
            role="combobox"
            aria-label=${label}
            aria-autocomplete="list"
            aria-expanded=${count > 0 ? 'true' : 'false'}
            aria-controls=${count > 0 ? 'results' : nothing}
            aria-activedescendant=${count > 0 ? this.optionId(this.active) : nothing}
            placeholder=${placeholder}
            autocomplete="off"
            spellcheck="false"
            .value=${this.query}
            @input=${this.handleInput}
          />
          <kbd aria-hidden="true">esc</kbd>
        </div>
        <div class="results">
          ${count === 0
            ? html`<div class="empty" aria-hidden="true">Nothing matches "${this.query}".</div>`
            : html`<div id="results" role="listbox" aria-label="Results">
                ${this.renderSection('books', 'Books', books, 0)}
                ${this.renderSection('screens', 'Go to', screens, books.length)}
              </div>`}
        </div>
        <footer>
          <span><kbd>↑↓</kbd> move</span>
          <span><kbd>↵</kbd> open</span>
          <span><kbd>esc</kbd> close</span>
          <span class="wordmark" aria-hidden="true">nigel</span>
        </footer>
        <div class="sr-only" role="status">${status}</div>
      </div>
    `;
  }

  private renderSection(key: string, heading: string, rows: Row[], offset: number) {
    if (rows.length === 0) return nothing;
    return html`
      <div class="section" role="group" aria-labelledby="heading-${key}">
        <div class="heading" id="heading-${key}" role="presentation">${heading}</div>
        ${rows.map((row, i) => this.renderOption(row, offset + i))}
      </div>
    `;
  }

  private renderOption(row: Row, index: number) {
    const active = index === this.active;
    const classes = { option: true, active, [row.kind]: true };
    const content = row.kind === 'book' ? this.renderBook(row.book) : this.renderScreen(row.screen);
    return html`
      <div
        id=${this.optionId(index)}
        class=${classMap(classes)}
        role="option"
        aria-selected=${active ? 'true' : 'false'}
        @mousemove=${() => {
          if (this.active !== index) this.active = index;
        }}
        @click=${() => this.activate(row)}
      >
        ${content}
      </div>
    `;
  }

  private renderBook(book: PaletteBook) {
    const when = book.open ? 'Open now' : (book.lastOpened ?? '');
    return html`
      <wc-monogram size="l" name=${book.name} seed=${book.id}></wc-monogram>
      <div>
        <div class="name">
          <span>${book.name}</span>
          ${book.open ? html`<span class="badge">Open</span>` : nothing}
        </div>
        <div class="sub">${book.kind} · ${book.path}</div>
      </div>
      <div class="when">
        ${book.encrypted
          ? html`<wc-icon-lock></wc-icon-lock><span class="sr-only">Encrypted,</span>`
          : nothing}
        ${when}
      </div>
    `;
  }

  private renderScreen(screen: PaletteScreen) {
    const icon = document.createElement(screen.icon);
    icon.setAttribute('aria-hidden', 'true');
    return html`${icon}<span>${screen.label}</span><span class="tag">screen</span>`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-command-palette': WcCommandPalette;
  }
}
