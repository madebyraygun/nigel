import { LitElement, html, css, nothing, type PropertyValues } from 'lit';
import { customElement, property, state, query } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '../icons/icons.js';
import './wc-money.js';
import './wc-empty-state.js';
import { categoryLabel, type CategoryOption } from './category-option.js';
import type { ShortcutHint } from './wc-shortcut-help.js';
import { controlsCss } from '@nigel/theme';

export type { CategoryOption };

/**
 * The register's keyboard legend, beside the switch that implements it.
 *
 * A screen renders this through `wc-shortcut-help`; `keys` carries the real
 * `KeyboardEvent.key` values so a test can walk the legend and prove every
 * line of it does something, rather than the legend being prose that drifts.
 */
export const REGISTER_SHORTCUTS: readonly ShortcutHint[] = [
  { keys: ['ArrowUp', 'ArrowDown'], display: '↑ ↓', description: 'Move between rows' },
  { keys: ['PageUp', 'PageDown'], display: 'PgUp PgDn', description: 'Move a screenful' },
  { keys: ['Home', 'End'], display: 'Home End', description: 'First or last row' },
  { keys: ['Enter'], display: 'Enter', description: 'Edit the category and vendor' },
  { keys: ['Escape'], display: 'Esc', description: 'Cancel the edit' },
  { keys: ['f'], display: 'f', description: 'Flag or unflag the row' },
  { keys: ['/'], display: '/', description: 'Jump to the search box' },
];

/** One transaction, in the shape the register table draws. */
export interface RegisterTableRow {
  id: number;
  /** `YYYY-MM-DD`. */
  date: string;
  description: string;
  amount: number;
  category: string | null;
  categoryId: number | null;
  vendor: string | null;
  accountName: string;
  isFlagged: boolean;
}

export interface NcRowEventDetail {
  id: number;
}

export interface NcEditCommitDetail {
  id: number;
  /** The chosen category, or null when the row is left uncategorized. */
  categoryId: number | null;
  /** The typed vendor, or null when the field was left empty (a clear). */
  vendor: string | null;
}

export interface NcFlagToggleDetail {
  id: number;
  /** The desired state, not a toggle — a retry must be safe. */
  flag: boolean;
}

/**
 * Rows to move by on PgUp/PgDn when the viewport cannot be measured — jsdom
 * reports every height as zero. Matches the TUI's `PAGE_SIZE`.
 */
const DEFAULT_PAGE_ROWS = 20;

/**
 * Rows kept in the DOM on each side of the viewport, so a scroll of a line or
 * two costs nothing and a focus move never lands on a row that is not there.
 */
const OVERSCAN = 8;

/**
 * The row height assumed until one has been measured — the first paint, and
 * every jsdom test, where every box measures zero.
 */
const ESTIMATED_ROW_HEIGHT = 33;

/**
 * Below this many rows the whole register goes into the DOM. Windowing costs a
 * scroll listener, two spacer rows and a re-render per frame; a register that
 * fits in a few screenfuls is cheaper without it, and the report screen's
 * read-only view is usually one month.
 */
const VIRTUALIZE_ABOVE = 120;

/**
 * The transaction register — the web counterpart of `browser.rs`.
 *
 * Two constraints shape it. Rows stay cheap: a row that is not being edited
 * renders text, one `wc-money` and one icon button, and no `wa-*` component at
 * all, because an unfiltered register is thousands of rows and every custom
 * element in a row is paid for thousands of times. And selection follows DOM
 * focus through a roving tabindex, so the keyboard model the TUI has is the
 * same one a screen reader announces.
 *
 * The category editor is a hand-built combobox rather than `wa-select`: this
 * Web Awesome build has no searchable select, and the combobox ARIA wiring
 * (`aria-activedescendant` onto the option list) needs an input this component
 * owns rather than one inside another component's shadow root.
 */
@customElement('wc-register-table')
export class WcRegisterTable extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: flex;
        flex-direction: column;
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
        min-height: 0;
      }

      .scroller {
        overflow: auto;
        max-height: var(--nc-register-height, 60vh);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-md, 8px);
      }

      /* The fill attribute is the register screen's mode: the table is a flex
         item that takes what is left between the toolbar and the bottom of
         the window, and the scroller is the only thing that scrolls. Without
         it the table is content-sized under a cap, which is what the
         read-only view inside a longer report page wants. */
      :host([fill]) {
        flex: 1 1 auto;
      }

      :host([fill]) .scroller {
        flex: 1 1 auto;
        min-height: 0;
        max-height: none;
      }

      /* Fixed layout is what makes every row exactly one line tall, which is
         what lets the window place rows by arithmetic instead of measuring
         each one. It also stops the columns from resizing as rows scroll in
         and out, which an auto-layout table would do on every frame. */
      table {
        width: 100%;
        border-collapse: collapse;
        table-layout: fixed;
      }

      .col-flag {
        width: 2.5rem;
      }

      .col-date {
        width: 6.5rem;
      }

      .col-category {
        width: 11rem;
      }

      .col-vendor {
        width: 9rem;
      }

      .col-amount {
        width: 7rem;
      }

      .col-account {
        width: 9rem;
      }

      /* Description is the only auto column, so it takes what is left — the
         Fill(1) the TUI gives the same column. */

      caption {
        position: absolute;
        width: 1px;
        height: 1px;
        overflow: hidden;
        clip-path: inset(50%);
        white-space: nowrap;
      }

      th {
        position: sticky;
        top: 0;
        z-index: 1;
        text-align: left;
        font-size: var(--wa-font-size-s, 13px);
        font-weight: var(--wa-font-weight-medium, 500);
        color: var(--wa-color-muted);
        background: var(--wa-color-surface);
        padding: var(--wa-space-xs, 6px) var(--wa-space-s, 8px);
        border-bottom: 1px solid var(--wa-color-border);
        white-space: nowrap;
      }

      td {
        padding: var(--wa-space-xs, 6px) var(--wa-space-s, 8px);
        border-bottom: 1px solid var(--wa-color-border-soft, var(--wa-color-border));
        vertical-align: top;
      }

      /* One line per transaction, clipped rather than wrapped. The full text
         stays in the DOM for a screen reader and rides a title for a pointer;
         what a wrapped row would cost is a table whose rows are all different
         heights, which a windowed list cannot place. */
      td.text {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }

      /* The rows above and below the window, standing in for their height so
         the scrollbar measures the whole register. */
      tr.spacer td {
        padding: 0;
        border-bottom: none;
      }

      :host([dense]) td {
        padding: var(--wa-space-2xs, 4px) var(--wa-space-s, 8px);
      }

      th.amount,
      td.amount {
        text-align: right;
      }

      td.date {
        white-space: nowrap;
        font-variant-numeric: tabular-nums;
      }

      th.flag,
      td.flag {
        text-align: center;
      }

      tbody tr[aria-selected='true'] {
        background: var(--wa-color-surface-alt, rgba(120, 120, 160, 0.18));
      }

      tbody tr[data-flagged='true'] td.date {
        box-shadow: inset 3px 0 0 var(--nc-color-flagged, #e0a13a);
      }

      tbody tr:focus-visible {
        outline: 2px solid var(--wa-color-focus);
        outline-offset: -2px;
      }

      tbody tr[aria-busy='true'] {
        opacity: 0.6;
      }

      .muted {
        color: var(--wa-color-muted);
      }

      .icon-button {
        font: inherit;
        color: var(--wa-color-muted);
        background: none;
        border: none;
        border-radius: var(--wa-radius-sm, 6px);
        padding: 2px;
        cursor: pointer;
        line-height: 0;
      }

      .icon-button[aria-pressed='true'] {
        color: var(--nc-color-flagged, #e0a13a);
      }

      .icon-button:focus-visible {
        outline: 2px solid var(--wa-color-focus);
        outline-offset: 1px;
      }

      tfoot td {
        border-bottom: none;
        border-top: 2px solid var(--wa-color-border);
        font-weight: var(--wa-font-weight-bold, 700);
        position: sticky;
        bottom: 0;
        background: var(--wa-color-surface);
      }

      .note {
        font-weight: var(--wa-font-weight-normal, 400);
        color: var(--wa-color-muted);
        font-size: var(--wa-font-size-s, 13px);
      }

      /* -- the inline editors -- */

      .combobox {
        position: relative;
      }

      .combobox input {
        font: inherit;
        width: 100%;
        min-width: 12ch;
        color: var(--wa-color-text);
        background: var(--wa-color-bg);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-sm, 6px);
        padding: var(--wa-space-2xs, 4px) var(--wa-space-xs, 6px);
      }

      .combobox input:focus-visible {
        outline: 2px solid var(--wa-color-focus);
        outline-offset: 1px;
      }

      .options {
        position: absolute;
        z-index: 2;
        margin: 2px 0 0;
        padding: 0;
        list-style: none;
        max-height: 12rem;
        overflow: auto;
        min-width: 100%;
        background: var(--wa-color-surface);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-sm, 6px);
        box-shadow: var(--wa-shadow-m, 0 4px 12px rgba(0, 0, 0, 0.25));
      }

      .options li {
        padding: var(--wa-space-2xs, 4px) var(--wa-space-xs, 6px);
        cursor: pointer;
        white-space: nowrap;
      }

      .options li[aria-selected='true'] {
        background: var(--wa-color-brand, #4a6cf7);
        color: var(--wa-color-on-brand, #fff);
      }

      /* The label names the field for a screen reader; the column header shows
         a sighted user the same thing, so it does not need the space. */
      .vendor-input::part(form-control-label) {
        position: absolute;
        width: 1px;
        height: 1px;
        overflow: hidden;
        clip-path: inset(50%);
        white-space: nowrap;
      }

      .sr-only {
        position: absolute;
        width: 1px;
        height: 1px;
        overflow: hidden;
        clip-path: inset(50%);
        white-space: nowrap;
      }

      .edit-actions {
        display: flex;
        gap: var(--wa-space-2xs, 4px);
        justify-content: flex-end;
      }

      .edit-actions button {
        font: inherit;
        font-size: var(--wa-font-size-s, 13px);
        color: inherit;
        background: none;
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-sm, 6px);
        padding: 2px var(--wa-space-xs, 6px);
        cursor: pointer;
      }

      .edit-actions button:focus-visible {
        outline: 2px solid var(--wa-color-focus);
        outline-offset: 1px;
      }
    `,
  ];

  @property({ attribute: false })
  rows: RegisterTableRow[] = [];

  @property({ attribute: false })
  categories: CategoryOption[] = [];

  @property({ type: Number, attribute: 'selected-id' })
  selectedId: number | null = null;

  /** The row in edit mode. The host owns this: activation only asks for it. */
  @property({ type: Number, attribute: 'editing-id' })
  editingId: number | null = null;

  /** A row with a write in flight. */
  @property({ type: Number, attribute: 'busy-id' })
  busyId: number | null = null;

  @property({ type: Boolean, reflect: true })
  dense = false;

  /**
   * Take the height a flex-column parent has left over, and scroll inside it,
   * rather than sizing to the rows under a cap.
   */
  @property({ type: Boolean, reflect: true })
  fill = false;

  /**
   * Drop every affordance that writes: no flag button, no activation, no edit
   * mode. Selection and scrolling stay, because reading a long register still
   * wants a cursor.
   *
   * This is what the reports screen's register view uses. Editing lives at
   * `#/register`; offering it from two screens would mean two places to keep
   * honest about the same row.
   */
  @property({ type: Boolean, reflect: true })
  readonly = false;

  @property({ type: Boolean, attribute: 'show-account' })
  showAccount = true;

  /** The table's accessible name. */
  @property({ type: String })
  caption = 'Transaction register';

  /** Rendered as the footer's running total. Omit and no total is shown. */
  @property({ type: Number })
  total?: number;

  /** Free text beside the total, e.g. a match count. */
  @property({ type: String, attribute: 'footer-note' })
  footerNote = '';

  @property({ type: String, attribute: 'empty-message' })
  emptyMessage = 'No transactions match these filters.';

  /**
   * Row count above which only the visible slice is put in the DOM. Set it to
   * `Infinity` to render everything — what a test that wants the whole table
   * does, and nothing else should need.
   */
  @property({ type: Number, attribute: 'virtualize-above' })
  virtualizeAbove = VIRTUALIZE_ABOVE;

  /**
   * Selection, held internally so the component still works uncontrolled (in
   * the preview harness, say). A host that sets `selectedId` overrides it.
   */
  @state() private activeId: number | null = null;

  @state() private editCategoryId: number | null = null;
  @state() private editCategoryQuery = '';
  @state() private editOptionIndex = 0;
  @state() private editListOpen = false;

  /** Index of the first row in the DOM. Meaningless while not windowing. */
  @state() private windowStart = 0;

  /** Measured once rows exist; the estimate only has to be close. */
  private rowHeight = ESTIMATED_ROW_HEIGHT;

  /**
   * Identifies the row *set*, not the array. The host rebuilds its filtered
   * array on every render, so array identity says nothing; length and the two
   * end ids change exactly when a search, a period or an account filter has
   * actually produced different rows, which is when the window goes back to
   * the top.
   */
  private rowsKey = '';

  @query('.scroller') private scroller?: HTMLElement;
  @query('.vendor-input') private vendorInput?: HTMLElement & { value: string };

  private pendingFocusId: number | null = null;

  /** A scroll position to apply once the rows it refers to have rendered. */
  private pendingScrollTop: number | null = null;

  private viewportObserver?: ResizeObserver;

  connectedCallback(): void {
    super.connectedCallback();
    this.watchViewport();
  }

  disconnectedCallback(): void {
    this.viewportObserver?.disconnect();
    this.viewportObserver = undefined;
    super.disconnectedCallback();
  }

  constructor() {
    super();
    // On the host, not on the scroller: the shortcuts have to fire from
    // wherever focus is inside the table — a row, the flag button on a row,
    // or the scroller itself — and a listener on one of those hears only its
    // own subtree.
    this.addEventListener('keydown', this.handleKeydown);
  }

  willUpdate(changed: PropertyValues<this>): void {
    if (changed.has('selectedId')) this.activeId = this.selectedId;

    if (changed.has('rows')) {
      if (this.activeId !== null && !this.rows.some((row) => row.id === this.activeId)) {
        // A row that filtering removed cannot stay selected.
        this.activeId = this.rows[0]?.id ?? null;
      }

      const key = `${this.rows.length}:${this.rows[0]?.id ?? ''}:${
        this.rows[this.rows.length - 1]?.id ?? ''
      }`;
      if (key !== this.rowsKey) {
        this.rowsKey = key;
        // A different set of rows is a different list: a search jump shows
        // its results from the top, not from wherever the last one was.
        this.windowStart = 0;
        this.pendingScrollTop = 0;
      }
    }

    if (changed.has('editingId')) {
      this.resetEditState();
      // The editors have to be in the DOM to be typed into.
      if (this.editingId !== null) {
        this.coverIndex(this.rows.findIndex((row) => row.id === this.editingId));
      }
    }
  }

  updated(): void {
    this.applyPendingScroll();

    if (this.remeasureRowHeight() && this.windowing) {
      // Spacer heights are computed from the row height, so the first real
      // measurement redraws them once and then settles.
      this.requestUpdate();
    }

    if (this.pendingFocusId === null) return;
    const id = this.pendingFocusId;
    this.pendingFocusId = null;
    this.rowElement(id)?.focus();
  }

  // -- the window -----------------------------------------------------------

  /**
   * How many rows the last render drew, so a resize that does not change that
   * asks for nothing — the observer fires on the layout each render causes.
   */
  private lastWindowLength = 0;

  /**
   * A taller window shows more rows, and nothing else says the viewport
   * changed: no scroll event follows a browser being maximized.
   */
  private watchViewport(): void {
    if (this.viewportObserver || typeof ResizeObserver === 'undefined') return;
    this.viewportObserver = new ResizeObserver(() => {
      if (this.windowing && this.windowLength !== this.lastWindowLength) {
        this.requestUpdate();
      }
    });
    this.viewportObserver.observe(this);
  }

  /** Whether this register is big enough to be worth windowing. */
  private get windowing(): boolean {
    return this.rows.length > this.virtualizeAbove;
  }

  /** Rows the viewport shows, plus the overscan on both sides. */
  private get windowLength(): number {
    const viewport = this.scroller?.clientHeight ?? 0;
    const visible = viewport > 0 ? Math.ceil(viewport / this.rowHeight) : DEFAULT_PAGE_ROWS;
    return visible + OVERSCAN * 2;
  }

  private get maxWindowStart(): number {
    return Math.max(0, this.rows.length - this.windowLength);
  }

  /** The half-open slice of `rows` that is in the DOM. */
  private get windowRange(): { start: number; end: number } {
    if (!this.windowing) return { start: 0, end: this.rows.length };
    const start = Math.min(Math.max(this.windowStart, 0), this.maxWindowStart);
    return { start, end: Math.min(this.rows.length, start + this.windowLength) };
  }

  /** Bring an index into the DOM without moving the scroll box. */
  private coverIndex(index: number): void {
    if (!this.windowing || index < 0) return;
    const { start, end } = this.windowRange;
    if (index >= start && index < end) return;
    this.windowStart = Math.min(Math.max(index - OVERSCAN, 0), this.maxWindowStart);
  }

  /**
   * Scroll so an index is on screen.
   *
   * The arithmetic is the point: a row outside the window has no box to ask,
   * so `scrollIntoView` cannot reach it. Every row is one line tall, so where
   * row *n* sits is `n * rowHeight` and nothing has to be measured.
   */
  private scrollToRowIndex(index: number, block: 'center' | 'nearest'): void {
    const scroller = this.scroller;
    if (!scroller) return;

    const viewport = scroller.clientHeight;
    const top = index * this.rowHeight;

    if (viewport <= 0) return;

    if (block === 'center') {
      this.pendingScrollTop = Math.max(0, top - viewport / 2 + this.rowHeight / 2);
      return;
    }

    const current = scroller.scrollTop;
    if (top < current) this.pendingScrollTop = top;
    else if (top + this.rowHeight > current + viewport) {
      this.pendingScrollTop = top + this.rowHeight - viewport;
    }
  }

  private applyPendingScroll(): void {
    if (this.pendingScrollTop === null) return;
    const top = this.pendingScrollTop;
    this.pendingScrollTop = null;
    if (this.scroller) this.scroller.scrollTop = top;
  }

  private handleScroll = (): void => {
    if (!this.windowing) return;
    const scroller = this.scroller;
    if (!scroller) return;
    // Browsers already deliver at most one scroll event per frame, and lit
    // batches the render into a microtask, so this needs no throttle of its
    // own.
    const first = Math.floor(scroller.scrollTop / this.rowHeight);
    const start = Math.min(Math.max(first - OVERSCAN, 0), this.maxWindowStart);
    if (start !== this.windowStart) this.windowStart = start;
  };

  /**
   * Read a real row's height back. Answers whether it changed.
   *
   * The editing row carries two inputs and is taller than the rest, so it is
   * never the one measured.
   */
  private remeasureRowHeight(): boolean {
    const rows = this.shadowRoot?.querySelectorAll<HTMLElement>('tbody tr[data-id]') ?? [];
    for (const row of rows) {
      if (Number(row.dataset.id) === this.editingId) continue;
      const height = row.getBoundingClientRect().height;
      if (height <= 0) return false;
      if (height === this.rowHeight) return false;
      this.rowHeight = height;
      return true;
    }
    return false;
  }

  // -- public API -----------------------------------------------------------

  /** Select a row by index and bring it into view. Used for scroll-to-today. */
  scrollToIndex(index: number): boolean {
    const row = this.rows[index];
    if (!row) return false;

    this.setActive(row.id);
    this.coverIndex(index);
    this.scrollToRowIndex(index, 'center');
    void this.updateComplete.then(() => this.bringIntoView(row.id));
    return true;
  }

  /** Select a row by transaction id and bring it into view. */
  scrollToRow(id: number): boolean {
    const index = this.rows.findIndex((row) => row.id === id);
    return index === -1 ? false : this.scrollToIndex(index);
  }

  /**
   * Put DOM focus on the row the cursor is on, so the shortcuts have somewhere
   * to fire from. Answers whether there was a row to focus.
   */
  focusSelectedRow(): boolean {
    const id = this.tabStopId;
    if (id === null) return false;
    const row = this.rowElement(id);
    if (!row) return false;
    row.focus();
    return true;
  }

  // -- selection ------------------------------------------------------------

  /**
   * The one row in the tab order.
   *
   * A roving tabindex needs a home even before anything is selected, or the
   * table is not reachable by Tab at all and none of its keys can ever fire —
   * which is exactly what happened on a register opened with a date filter,
   * where nothing lands a cursor on load.
   */
  private get tabStopId(): number | null {
    const { start, end } = this.windowRange;
    if (this.activeId !== null) {
      const index = this.rows.findIndex((row) => row.id === this.activeId);
      // A selection the user has scrolled away from is not in the DOM, so it
      // cannot hold the tab stop; the first row on screen takes it instead.
      if (index >= start && index < end) return this.activeId;
    }
    return this.rows[start]?.id ?? null;
  }

  private rowElement(id: number): HTMLElement | null {
    return this.shadowRoot?.querySelector<HTMLElement>(`tr[data-id="${id}"]`) ?? null;
  }

  /**
   * The unwindowed path: every row has a box, so the browser can be asked.
   * A windowed table has already been scrolled by arithmetic.
   */
  private bringIntoView(id: number): void {
    if (this.windowing) return;
    const element = this.rowElement(id);
    // jsdom has no layout and no scrollIntoView.
    if (element && typeof element.scrollIntoView === 'function') {
      element.scrollIntoView({ block: 'center' });
    }
  }

  private setActive(id: number): void {
    if (this.activeId === id) return;
    this.activeId = id;
    this.dispatchEvent(
      new CustomEvent<NcRowEventDetail>('nc-row-select', {
        detail: { id },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private get activeIndex(): number {
    return this.rows.findIndex((row) => row.id === this.activeId);
  }

  private focusIndex(index: number): void {
    const clamped = Math.min(Math.max(index, 0), this.rows.length - 1);
    const row = this.rows[clamped];
    if (!row) return;
    this.setActive(row.id);
    this.pendingFocusId = row.id;
    // Both are needed and in this order: the row has to exist before the
    // scroll box is moved to where it will be.
    this.coverIndex(clamped);
    this.scrollToRowIndex(clamped, 'nearest');
    this.requestUpdate();
  }

  private pageRows(): number {
    this.remeasureRowHeight();
    const viewport = this.scroller?.clientHeight ?? 0;
    const rows = this.rowHeight > 0 ? Math.floor(viewport / this.rowHeight) : 0;
    return rows > 1 ? rows : DEFAULT_PAGE_ROWS;
  }

  // -- keyboard -------------------------------------------------------------

  private handleKeydown = (event: KeyboardEvent): void => {
    if (this.editingId !== null) return;
    if (this.rows.length === 0) return;

    const current = this.activeIndex;
    const from = current === -1 ? 0 : current;

    switch (event.key) {
      case 'ArrowDown':
        this.focusIndex(current === -1 ? 0 : from + 1);
        break;
      case 'ArrowUp':
        this.focusIndex(current === -1 ? 0 : from - 1);
        break;
      case 'PageDown':
        this.focusIndex(from + this.pageRows());
        break;
      case 'PageUp':
        this.focusIndex(from - this.pageRows());
        break;
      case 'Home':
        this.focusIndex(0);
        break;
      case 'End':
        this.focusIndex(this.rows.length - 1);
        break;
      case 'Enter':
        this.activateRow(this.rows[from]);
        break;
      case 'f':
      case 'F':
        this.requestFlag(this.rows[from]);
        break;
      case '/':
        this.dispatchEvent(
          new CustomEvent('nc-search-focus', { bubbles: true, composed: true }),
        );
        break;
      default:
        // Escape is the register's cancel-the-edit key and belongs to the two
        // editors, which handle it themselves. Clearing the selection here
        // instead used to take the table's only tab stop away with it.
        return;
    }

    event.preventDefault();
  };

  private activateRow(row: RegisterTableRow | undefined): void {
    if (!row) return;
    this.setActive(row.id);
    // A read-only table still moves its cursor; it just never asks to edit.
    if (this.readonly) return;
    this.dispatchEvent(
      new CustomEvent<NcRowEventDetail>('nc-row-activate', {
        detail: { id: row.id },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private requestFlag(row: RegisterTableRow | undefined): void {
    if (!row || this.readonly) return;
    this.dispatchEvent(
      new CustomEvent<NcFlagToggleDetail>('nc-flag-toggle', {
        detail: { id: row.id, flag: !row.isFlagged },
        bubbles: true,
        composed: true,
      }),
    );
  }

  // -- inline editing -------------------------------------------------------

  private get editingRow(): RegisterTableRow | undefined {
    return this.rows.find((row) => row.id === this.editingId);
  }

  private resetEditState(): void {
    const row = this.editingRow;
    this.editCategoryId = row?.categoryId ?? null;
    this.editCategoryQuery = row?.category ?? '';
    this.editOptionIndex = 0;
    this.editListOpen = false;
  }

  /** Case-insensitive substring over the "Name (inc)" label, as the TUI does. */
  private get filteredCategories(): CategoryOption[] {
    const query = this.editCategoryQuery.trim().toLowerCase();
    if (query === '') return this.categories;
    return this.categories.filter((category) =>
      categoryLabel(category).toLowerCase().includes(query),
    );
  }

  private chooseCategory(option: CategoryOption | undefined): void {
    if (!option) return;
    this.editCategoryId = option.id;
    this.editCategoryQuery = option.name;
    this.editListOpen = false;
    this.editOptionIndex = 0;
  }

  private handleCategoryInput(event: Event): void {
    this.editCategoryQuery = (event.target as HTMLInputElement).value;
    this.editListOpen = true;
    this.editOptionIndex = 0;
  }

  private handleCategoryKeydown(event: KeyboardEvent): void {
    const options = this.filteredCategories;

    switch (event.key) {
      case 'ArrowDown':
        this.editListOpen = true;
        this.editOptionIndex = Math.min(this.editOptionIndex + 1, options.length - 1);
        break;
      case 'ArrowUp':
        this.editListOpen = true;
        this.editOptionIndex = Math.max(this.editOptionIndex - 1, 0);
        break;
      case 'Enter':
        // Category then vendor, the order the TUI's two-stage editor uses.
        this.chooseCategory(options[this.editOptionIndex]);
        void this.updateComplete.then(() => this.vendorInput?.focus());
        break;
      case 'Escape':
        this.cancelEdit();
        break;
      default:
        return;
    }

    event.preventDefault();
    event.stopPropagation();
  }

  private handleVendorKeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter') {
      this.commitEdit();
    } else if (event.key === 'Escape') {
      this.cancelEdit();
    } else {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
  }

  private commitEdit(): void {
    const row = this.editingRow;
    if (!row) return;

    const vendor = (this.vendorInput?.value ?? '').trim();
    this.dispatchEvent(
      new CustomEvent<NcEditCommitDetail>('nc-edit-commit', {
        detail: {
          id: row.id,
          categoryId: this.editCategoryId,
          vendor: vendor === '' ? null : vendor,
        },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private cancelEdit(): void {
    const row = this.editingRow;
    if (!row) return;
    this.dispatchEvent(
      new CustomEvent<NcRowEventDetail>('nc-edit-cancel', {
        detail: { id: row.id },
        bubbles: true,
        composed: true,
      }),
    );
  }

  // -- render ---------------------------------------------------------------

  render() {
    if (this.rows.length === 0) {
      return html`
        <wc-empty-state
          icon="wc-icon-register"
          heading="Nothing to show"
          message=${this.emptyMessage}
        ></wc-empty-state>
      `;
    }

    const columns = this.showAccount ? 7 : 6;
    const { start, end } = this.windowRange;
    this.lastWindowLength = this.windowLength;
    const above = start * this.rowHeight;
    const below = (this.rows.length - end) * this.rowHeight;
    // Resolved once: the getter walks the rows, and a row must not pay for
    // that on a register of two thousand.
    const tabStop = this.tabStopId;

    return html`
      <div class="scroller" @scroll=${this.handleScroll}>
        <table role="grid" aria-rowcount=${this.rows.length + 1}>
          <caption>
            ${this.caption}
          </caption>
          <colgroup>
            <col class="col-flag" />
            <col class="col-date" />
            <col class="col-description" />
            <col class="col-category" />
            <col class="col-vendor" />
            <col class="col-amount" />
            ${this.showAccount ? html`<col class="col-account" />` : nothing}
          </colgroup>
          <thead>
            <tr role="row" aria-rowindex="1">
              <th scope="col" class="flag"><span class="sr-only">Flag</span></th>
              <th scope="col">Date</th>
              <th scope="col">Description</th>
              <th scope="col">Category</th>
              <th scope="col">Vendor</th>
              <th scope="col" class="amount">Amount</th>
              ${this.showAccount ? html`<th scope="col">Account</th>` : nothing}
            </tr>
          </thead>
          <tbody>
            ${this.renderSpacer(above, columns)}
            ${this.rows
              .slice(start, end)
              .map((row, offset) => this.renderRow(row, start + offset, tabStop))}
            ${this.renderSpacer(below, columns)}
          </tbody>
          ${this.total === undefined && this.footerNote === ''
            ? nothing
            : html`
                <tfoot>
                  <tr role="row">
                    <td role="gridcell" colspan=${columns - 1}>
                      Net
                      ${this.footerNote
                        ? html`<span class="note">· ${this.footerNote}</span>`
                        : nothing}
                    </td>
                    <td role="gridcell" class="amount">
                      ${this.total === undefined
                        ? nothing
                        : html`<wc-money .amount=${this.total} align="end"></wc-money>`}
                    </td>
                    ${this.showAccount ? html`<td role="gridcell"></td>` : nothing}
                  </tr>
                </tfoot>
              `}
        </table>
      </div>
    `;
  }

  /** Height standing in for the rows outside the window, so the scrollbar is
      the length of the whole register rather than of what is drawn. */
  private renderSpacer(height: number, columns: number) {
    if (height <= 0) return nothing;
    return html`
      <tr class="spacer" aria-hidden="true">
        <td colspan=${columns} style=${`height: ${height}px`}></td>
      </tr>
    `;
  }

  private renderFlagButton(row: RegisterTableRow) {
    return html`
      <button
        class="icon-button"
        type="button"
        aria-pressed=${row.isFlagged ? 'true' : 'false'}
        aria-label=${`Flag transaction ${row.id}`}
        ?disabled=${row.id === this.busyId}
        @click=${() => this.requestFlag(row)}
      >
        <wc-icon-flag></wc-icon-flag>
      </button>
    `;
  }

  /**
   * Read-only rows still have to show which transactions are flagged, and the
   * row's colour cannot be the only channel that says so.
   */
  private renderFlagMark(row: RegisterTableRow) {
    if (!row.isFlagged) return nothing;
    return html`<wc-icon-flag role="img" aria-label="Flagged"></wc-icon-flag>`;
  }

  private renderRow(row: RegisterTableRow, index: number, tabStopId: number | null) {
    const selected = row.id === this.activeId;
    const editing = row.id === this.editingId;

    return html`
      <tr
        role="row"
        data-id=${row.id}
        data-flagged=${row.isFlagged ? 'true' : 'false'}
        aria-selected=${selected ? 'true' : 'false'}
        aria-busy=${row.id === this.busyId ? 'true' : 'false'}
        aria-rowindex=${index + 2}
        tabindex=${row.id === tabStopId ? '0' : '-1'}
        @focusin=${() => this.setActive(row.id)}
        @dblclick=${() => this.activateRow(row)}
      >
        <td role="gridcell" class="flag">
          ${this.readonly ? this.renderFlagMark(row) : this.renderFlagButton(row)}
        </td>
        <td role="gridcell" class="date">${row.date}</td>
        <td role="gridcell" class="text" title=${row.description}>${row.description}</td>
        ${editing
          ? this.renderEditCells(row)
          : html`
              <td role="gridcell" class="text" title=${row.category ?? ''}>
                ${row.category ?? html`<span class="muted">—</span>`}
              </td>
              <td role="gridcell" class="text" title=${row.vendor ?? ''}>
                ${row.vendor ?? ''}
              </td>
            `}
        <td role="gridcell" class="amount">
          <wc-money .amount=${row.amount} align="end"></wc-money>
        </td>
        ${this.showAccount
          ? html`<td role="gridcell" class=${editing ? '' : 'text'} title=${row.accountName}>
              ${editing ? this.renderEditActions() : row.accountName}
            </td>`
          : nothing}
      </tr>
    `;
  }

  private renderEditCells(row: RegisterTableRow) {
    const options = this.filteredCategories;
    const active = options[this.editOptionIndex];

    return html`
      <td role="gridcell">
        <div class="combobox">
          <input
            class="category-input"
            type="text"
            role="combobox"
            aria-label="Category"
            aria-expanded=${this.editListOpen ? 'true' : 'false'}
            aria-controls=${this.editListOpen ? 'category-options' : nothing}
            aria-autocomplete="list"
            aria-activedescendant=${this.editListOpen && active
              ? `category-option-${active.id}`
              : nothing}
            .value=${this.editCategoryQuery}
            @input=${this.handleCategoryInput}
            @keydown=${this.handleCategoryKeydown}
          />
          ${this.editListOpen
            ? html`
                <ul class="options" id="category-options" role="listbox" aria-label="Categories">
                  ${options.map(
                    (option, index) => html`
                      <li
                        id=${`category-option-${option.id}`}
                        role="option"
                        aria-selected=${index === this.editOptionIndex ? 'true' : 'false'}
                        @mousedown=${() => this.chooseCategory(option)}
                      >
                        ${categoryLabel(option)}
                      </li>
                    `,
                  )}
                </ul>
              `
            : nothing}
        </div>
      </td>
      <td role="gridcell">
        <wa-input
          class="vendor-input"
          label="Vendor"
          size="s"
          value=${row.vendor ?? ''}
          @keydown=${this.handleVendorKeydown}
        ></wa-input>
      </td>
    `;
  }

  private renderEditActions() {
    return html`
      <div class="edit-actions">
        <button type="button" @click=${() => this.commitEdit()}>Save</button>
        <button type="button" @click=${() => this.cancelEdit()}>Cancel</button>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-register-table': WcRegisterTable;
  }

  interface HTMLElementEventMap {
    'nc-row-select': CustomEvent<NcRowEventDetail>;
    'nc-row-activate': CustomEvent<NcRowEventDetail>;
    'nc-edit-commit': CustomEvent<NcEditCommitDetail>;
    'nc-edit-cancel': CustomEvent<NcRowEventDetail>;
    'nc-flag-toggle': CustomEvent<NcFlagToggleDetail>;
    'nc-search-focus': CustomEvent<void>;
  }
}
