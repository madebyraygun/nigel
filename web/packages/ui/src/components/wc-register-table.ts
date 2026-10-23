import { LitElement, html, css, nothing, type PropertyValues } from 'lit';
import { customElement, property, state, query } from 'lit/decorators.js';
import { repeat } from 'lit/directives/repeat.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '../icons/icons.js';
import { formatMoney } from './wc-money.js';
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
  {
    keys: ['Escape'],
    display: 'Esc',
    description: 'Cancel the edit, or clear the selection',
  },
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

/** The keys a focused control answers itself, rather than the grid around it. */
const ACTIVATION_KEYS = new Set([' ', 'Enter']);

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
      /* A column filling the screen: with rows that is invisible, and with none
         it is what lets the empty state centre itself where the table was. */
      :host {
        display: flex;
        flex-direction: column;
        flex: 1 1 auto;
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
        min-height: 0;
      }

      /* Content-sized under a cap: the shape a page that scrolls as a whole
         wants, and the only mode --nc-register-height applies to. */
      .scroller {
        overflow: auto;
        max-height: var(--nc-register-height, 60vh);
        background: var(--wa-color-surface);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-lg);
        font-size: var(--nc-font-size-control);
        font-variant-numeric: tabular-nums;
      }

      /* The fill attribute is the register screen's mode: the table takes what
         is left between the toolbar and the bottom of the window.

         The *host* is what grows, and it carries the floor. The scroller only
         ever shrinks into it (flex: 0 1 auto), so three search matches draw a
         three-row box rather than a full-height bordered one with the total row
         pulled up under them — a sticky footer is pulled up by its scroller
         and never pushed down.

         --nc-register-min-height is where shrinking stops. Below it the page
         scrolls instead, which is what a viewport shortened by a docked
         devtools panel needs: the alternative is a table collapsed to a sliver
         under its own sticky total row. It sits on the host rather than on the
         scroller because the scroller has a border and must stay free to hug
         short content.

         --nc-register-height does not apply here and is not meant to: a cap
         and a parent-driven height cannot both decide, and under fill the
         parent decides. */
      :host([fill]) {
        display: flex;
        flex-direction: column;
        flex: 1 1 auto;
        min-height: var(--nc-register-min-height, 12rem);
      }

      :host([fill]) .scroller {
        flex: 0 1 auto;
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

      /* The card's own width picks the layout (the stacked rules below). */
      .scroller {
        container-type: inline-size;
      }

      .col-flag {
        width: 40px;
      }

      .col-date {
        width: 104px;
      }

      .col-category {
        width: 186px;
      }

      .col-vendor {
        width: 118px;
      }

      /* A six-figure amount with its sign and separators; the clipping below
         is the backstop, because a figure that overflowed would land in the
         Account column beside it. */
      .col-amount {
        width: 116px;
      }

      .col-account {
        width: 150px;
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
        height: 34px;
        box-sizing: border-box;
        text-align: left;
        font-size: var(--nc-font-size-meta);
        font-weight: var(--nc-font-weight-table-heading);
        color: var(--wa-color-muted);
        background: var(--wa-color-surface-alt);
        padding: 0 var(--wa-space-s);
        border-bottom: 1px solid var(--wa-color-border);
        white-space: nowrap;
      }

      td {
        height: 36px;
        box-sizing: border-box;
        padding: 0 var(--wa-space-s);
        border-bottom: 1px solid var(--wa-color-border-soft);
        vertical-align: middle;
      }

      /* The transaction data itself — date, description, category, vendor,
         account — is what a person copies out of the register; the flag
         toggle and the inline editors it sits beside are not. */
      td.date,
      td.text,
      td.amount {
        user-select: text;
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
        height: 30px;
      }

      th.amount,
      td.amount {
        text-align: right;
        padding-right: 18px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }

      td.date {
        white-space: nowrap;
        color: var(--wa-color-muted);
      }

      th.flag,
      td.flag {
        text-align: center;
        padding: 0;
      }

      tbody tr[data-id]:hover {
        background: var(--wa-color-surface-alt);
      }

      tbody tr[aria-selected='true'],
      tbody tr[aria-selected='true']:hover {
        background: var(--nc-color-highlight);
      }

      /* The left-edge stripe: brand on the selected row, the flag colour on a
         row that wants attention — flagged, or not yet categorised. The flag
         button and the "Uncategorized" text carry the same news, so colour is
         never the only channel. */
      tbody tr[data-flagged='true'] td.flag,
      tbody tr[data-uncategorized='true'] td.flag {
        box-shadow: inset 2px 0 0 var(--nc-color-flagged);
      }

      tbody tr[aria-selected='true'] td.flag {
        box-shadow: inset 2px 0 0 var(--wa-color-brand);
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
        display: inline-grid;
        place-items: center;
        width: 28px;
        height: 28px;
        font: inherit;
        color: var(--wa-color-muted);
        background: none;
        border: none;
        border-radius: var(--wa-radius-md);
        padding: 0;
        cursor: default;
        line-height: 0;
        --nc-icon-size: 16px;
      }

      .icon-button:hover:not(:disabled) {
        background: var(--nc-color-hover);
      }

      .icon-button[aria-pressed='true'] {
        color: var(--nc-color-flagged);
        --nc-icon-fill: currentColor;
      }

      .icon-button:focus-visible {
        outline: 2px solid var(--wa-color-focus);
        outline-offset: 2px;
      }

      td.flag > wc-icon-flag {
        --nc-icon-size: 16px;
        --nc-icon-fill: currentColor;
        color: var(--nc-color-flagged);
      }

      tfoot td {
        height: 38px;
        border-bottom: none;
        border-top: 1px solid var(--wa-color-border);
        font-size: var(--nc-font-size-meta);
        position: sticky;
        bottom: 0;
        background: var(--wa-color-surface-alt);
      }

      .foot {
        flex: 1;
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--wa-space-m);
        padding: 0 6px 0 var(--wa-space-xs);
        white-space: nowrap;
      }

      .foot-label {
        overflow: hidden;
        text-overflow: ellipsis;
        color: var(--wa-color-text);
      }

      .note {
        color: var(--wa-color-muted);
      }

      .foot-total {
        display: inline-flex;
        align-items: baseline;
        gap: var(--wa-space-s);
        color: var(--wa-color-muted);
      }

      .foot-total wc-money {
        font-weight: var(--wa-font-weight-bold);
      }

      /* -- the stacked layout --

         Below 860px the fixed columns (714px) would leave Description next to
         nothing, so each row becomes two lines in the card's own width: flag,
         date, description and amount, then category · vendor · account. Every
         row is the same 60px, so the window still places rows by
         arithmetic; the resize observer measures the height again when the
         card crosses the line.

         The table keeps its grid roles, so the cells stay announced as cells
         of their columns; the header row is there for assistive technology
         and takes no room. */
      @container (max-width: 860px) {
        table,
        tbody,
        tfoot {
          display: block;
        }

        thead {
          position: absolute;
          width: 1px;
          height: 1px;
          overflow: hidden;
          clip-path: inset(50%);
          white-space: nowrap;
        }

        tbody tr[data-id] {
          display: flex;
          flex-wrap: wrap;
          align-content: flex-start;
          height: 60px;
          box-sizing: border-box;
          border-bottom: 1px solid var(--wa-color-border-soft);
        }

        /* Ends the first line, whatever is left on it. */
        tbody tr[data-id]::after {
          content: '';
          order: 1;
          flex-basis: 100%;
          height: 0;
        }

        tbody tr[data-editing] {
          height: auto;
          min-height: 60px;
          padding-bottom: var(--wa-space-xs);
        }

        tbody td {
          display: block;
          border-bottom: none;
          box-sizing: border-box;
        }

        tr.spacer,
        tr.spacer td {
          display: block;
        }

        td.flag,
        td.date,
        td.description,
        td.amount {
          height: 32px;
          line-height: 32px;
        }

        td.flag {
          flex: none;
          width: 40px;
          display: grid;
          place-items: center;
        }

        td.date {
          flex: none;
          padding: 0 var(--wa-space-xs) 0 0;
        }

        td.description {
          flex: 1 1 0;
          min-width: 0;
        }

        td.amount {
          flex: none;
          padding-right: var(--wa-space-m);
        }

        td.category,
        td.vendor,
        td.account {
          order: 2;
          height: 27px;
          line-height: 27px;
          padding: 0;
          font-size: var(--nc-font-size-meta);
          color: var(--wa-color-muted);
          flex: 0 1 auto;
          min-width: 0;
        }

        /* Capped shares of the line rather than wrapping: a flex line breaks
           on an item's capped size, so all three stay on the second line and
           the one that runs out of room ellipsises. */
        td.category {
          margin-left: 40px;
          max-width: calc((100% - 40px) * 0.4);
        }

        td.vendor {
          max-width: calc((100% - 40px) * 0.3);
        }

        /* Whatever the first two leave: a zero basis never pushes it onto a
           third line. */
        td.account {
          flex: 1 1 0;
        }

        td.vendor::before,
        td.account::before {
          content: '·';
          padding: 0 6px;
        }

        td.blank {
          display: none;
        }

        .uncategorized {
          color: var(--nc-color-flagged);
        }

        /* The editors take the second line between them, Save and Cancel at
           its end. */
        tr[data-editing] td.category,
        tr[data-editing] td.vendor {
          max-width: none;
          flex: 1 1 0;
          height: auto;
          line-height: normal;
        }

        tr[data-editing] td.vendor {
          margin-left: var(--wa-space-xs);
        }

        tr[data-editing] td.account {
          max-width: none;
          flex: none;
          height: auto;
          margin-left: var(--wa-space-xs);
          padding-right: var(--wa-space-s);
        }

        tr[data-editing] td.vendor::before,
        tr[data-editing] td.account::before {
          content: none;
        }

        /* One stripe down the whole row rather than down the flag cell, which
           is only the first line here. */
        tbody tr[data-flagged='true'] td.flag,
        tbody tr[data-uncategorized='true'] td.flag,
        tbody tr[aria-selected='true'] td.flag {
          box-shadow: none;
        }

        tbody tr[data-flagged='true'],
        tbody tr[data-uncategorized='true'] {
          box-shadow: inset 2px 0 0 var(--nc-color-flagged);
        }

        tbody tr[aria-selected='true'] {
          box-shadow: inset 2px 0 0 var(--wa-color-brand);
        }

        /* A block footer is pinned as a whole: a sticky cell would only
           stick inside its own row. */
        tfoot {
          position: sticky;
          bottom: 0;
          z-index: 1;
        }

        tfoot tr {
          display: block;
        }

        tfoot td {
          position: static;
          display: flex;
          align-items: center;
          box-sizing: border-box;
          padding: 0 var(--wa-space-s);
        }
      }

      /* -- the inline editors -- */

      .combobox {
        position: relative;
      }

      .combobox input {
        font: inherit;
        width: 100%;
        min-width: 12ch;
        box-sizing: border-box;
        color: var(--wa-color-text);
        background: var(--nc-color-field);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-md);
        padding: var(--wa-space-2xs) var(--wa-space-xs);
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
        border-radius: var(--wa-radius-sm);
        box-shadow: var(--wa-shadow-m);
      }

      .options li {
        padding: var(--wa-space-2xs) var(--wa-space-xs);
        cursor: default;
        white-space: nowrap;
      }

      .options li[aria-selected='true'] {
        background: var(--wa-color-brand);
        color: var(--wa-color-on-brand);
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
        gap: var(--wa-space-2xs);
        justify-content: flex-end;
      }

      .edit-actions button {
        font: inherit;
        font-size: var(--wa-font-size-s);
        color: inherit;
        background: none;
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-sm);
        padding: 2px var(--wa-space-xs);
        cursor: default;
      }

      .edit-actions button:focus-visible {
        outline: 2px solid var(--wa-color-focus);
        outline-offset: 1px;
      }

      /* Block flow for the sheets, where a register runs to many pages: a flex
         container is not required to fragment, and the leftover height the
         column hands out is a property of a viewport. */
      @media print {
        :host {
          display: block;
        }
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

  /** What the footer's total covers, on its left — the register's account filter. */
  @property({ type: String, attribute: 'footer-label' })
  footerLabel = '';

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

  @query('.scroller') private scroller?: HTMLElement;
  @query('.vendor-input') private vendorInput?: HTMLElement & { value: string };

  private pendingFocusId: number | null = null;

  /**
   * The table was opened at its end with nothing selected: the fallback tab
   * stop is the last row rather than the first one drawn. A new list drops it.
   */
  private stopAtEnd = false;

  /** A scroll position to apply once the rows it refers to have rendered. */
  private pendingScrollTop: number | null = null;

  private viewportObserver?: ResizeObserver;

  /** The pointer behind the last press, for a browser whose clicks do not say. */
  private lastPointerType = '';

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
    this.addEventListener('pointerdown', (event) => {
      this.lastPointerType = event.pointerType;
    });
  }

  willUpdate(changed: PropertyValues<this>): void {
    if (changed.has('selectedId')) this.activeId = this.selectedId;

    if (changed.has('rows')) {
      if (changed.get('rows') !== undefined) this.stopAtEnd = false;
      if (this.activeId !== null && !this.rows.some((row) => row.id === this.activeId)) {
        // A row that filtering removed cannot stay selected.
        this.activeId = this.rows[0]?.id ?? null;
      }

      this.resetWindowIfListChanged(changed.get('rows'));
    }

    if (changed.has('editingId')) this.resetEditState();
  }

  /**
   * Send the window back to the top, but only for a genuinely different list.
   *
   * The test is whether the row the window is anchored on is still among the
   * rows: a search, an account change or a period change leaves nothing it was
   * showing, while an optimistic edit that drops one row elsewhere leaves the
   * anchor where it is. Length alone cannot tell those apart, and resetting on
   * length costs the user their place over an edit they just made.
   */
  private resetWindowIfListChanged(previous: RegisterTableRow[] | undefined): void {
    if (!this.windowing) return;

    const anchorId = previous?.[this.windowStart]?.id;
    if (anchorId !== undefined && this.rows.some((row) => row.id === anchorId)) return;

    this.windowStart = 0;
    this.pendingScrollTop = 0;
  }

  updated(changed: PropertyValues<this>): void {
    this.applyPendingScroll();

    // An open editor is taller than a row (in the stacked layout it adds a
    // line of fields), so its bottom may now sit under the total row.
    if (changed.has('editingId') && this.editingId !== null) {
      this.keepBottomInView(this.editingId);
    }

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
    this.viewportObserver = new ResizeObserver(() => this.handleViewportResize());
    this.viewportObserver.observe(this);
  }

  /**
   * The card was resized. Crossing the stacked-layout width changes how tall
   * a row is, and every row position is `index * rowHeight`, so the height is
   * measured again and the view re-anchored: on the selection when it is in
   * view, otherwise on the first row that was showing.
   */
  private handleViewportResize(): void {
    const scroller = this.scroller;
    const before = this.rowHeight;
    const firstShown = scroller ? Math.floor(scroller.scrollTop / before) : 0;

    if (this.remeasureRowHeight()) {
      const selected = this.activeIndex;
      if (selected !== -1 && this.selectionIsRendered) {
        this.coverIndex(selected);
        this.scrollToRowIndex(selected, 'nearest');
      } else {
        this.pendingScrollTop = firstShown * this.rowHeight;
      }
      this.requestUpdate();
      return;
    }

    if (this.windowing && this.windowLength !== this.lastWindowLength) {
      this.requestUpdate();
    }
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
  private scrollToRowIndex(index: number, block: 'center' | 'end' | 'nearest'): void {
    // Whatever was pending is about to be decided one way or the other. A
    // pending scroll left over from a call that needed none is a view yanked
    // sideways by the next unrelated render.
    this.pendingScrollTop = null;

    const scroller = this.scroller;
    if (!scroller) return;

    const viewport = scroller.clientHeight;
    if (viewport <= 0) return;

    // The header and the total row are painted over the scroller, so the band a
    // row is actually visible in starts below one and ends above the other.
    const head = this.stickyHeadHeight();
    const band = this.visibleRowsHeight();
    const top = index * this.rowHeight;

    if (block === 'center') {
      this.pendingScrollTop = Math.max(0, top + this.rowHeight / 2 - (head + band / 2));
    } else if (block === 'end') {
      // The header is in flow above the rows as well as painted over them, so
      // a row's bottom edge sits `head` below where `top` puts it — the same
      // amount the band starts below the scroller's edge.
      this.pendingScrollTop = Math.max(0, top + this.rowHeight - band);
    } else {
      const current = scroller.scrollTop;
      if (top - head < current) this.pendingScrollTop = Math.max(0, top - head);
      else if (top + this.rowHeight > current + head + band) {
        this.pendingScrollTop = top + this.rowHeight - head - band;
      }
    }

    // A row already in the DOM schedules no re-render, so `updated` may never
    // run: do it now rather than leave the scroll for whatever renders next.
    const { start, end } = this.windowRange;
    if (index >= start && index < end) this.applyPendingScroll();
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

  /**
   * Select a row by index and bring it into view. Used for scroll-to-today.
   *
   * `end` sets the row on the bottom of the visible band, so the rows before
   * it fill the view — how the register opens on the newest transactions.
   */
  scrollToIndex(index: number, block: 'center' | 'end' = 'center'): boolean {
    const row = this.rows[index];
    if (!row) return false;

    this.setActive(row.id);
    this.coverIndex(index);
    this.scrollToRowIndex(index, block);
    void this.updateComplete.then(() =>
      block === 'center' ? this.bringIntoView(row.id) : this.settleAtEnd(row.id),
    );
    return true;
  }

  /**
   * Scroll to the last rows without selecting any. The last row becomes the
   * tab stop, so the keyboard lands where the view is.
   */
  scrollToEnd(): void {
    const last = this.rows[this.rows.length - 1];
    if (!last) return;
    this.stopAtEnd = true;
    this.coverIndex(this.rows.length - 1);
    this.scrollToRowIndex(this.rows.length - 1, 'end');
    void this.updateComplete.then(() => this.settleAtEnd(last.id));
  }

  /** Select a row by transaction id and bring it into view. */
  scrollToRow(id: number): boolean {
    const index = this.rows.findIndex((row) => row.id === id);
    return index === -1 ? false : this.scrollToIndex(index);
  }

  /**
   * Put DOM focus on the tab stop, so the shortcuts have somewhere to fire
   * from. Answers whether there was a row to focus.
   *
   * It selects nothing: on a register that opened with no cursor the stop is
   * the first row as a fallback, and landing the keyboard there is not a
   * decision about which transaction is current. Nor does it scroll: the
   * caller has already put the view where it wants it, and the fallback stop
   * can sit just above the rows on screen.
   *
   * `reveal` scrolls just far enough to show the row, for a caller handing
   * the keyboard back to a row it already chose, such as after an edit.
   */
  focusSelectedRow(options: { reveal?: boolean } = {}): boolean {
    const id = this.tabStopId;
    if (id === null) return false;
    if (options.reveal) {
      this.scrollToRowIndex(
        this.rows.findIndex((candidate) => candidate.id === id),
        'nearest',
      );
    }
    const row = this.rowElement(id);
    if (!row) return false;
    row.focus({ preventScroll: true });
    return true;
  }

  // -- selection ------------------------------------------------------------

  /**
   * The one row in the tab order.
   *
   * A roving tabindex needs a home even before anything is selected: without
   * one the table is not reachable by Tab at all and none of its keys can
   * fire, which is the state a date-filtered register loads in, since nothing
   * lands a cursor there.
   */
  private get tabStopId(): number | null {
    if (this.selectionIsRendered) return this.activeId;
    const { start, end } = this.windowRange;
    const last = this.rows.length - 1;
    if (this.stopAtEnd && last >= start && last < end) return this.rows[last]?.id ?? null;
    return this.rows[start]?.id ?? null;
  }

  /**
   * Whether the selected row is one the table has actually drawn — which on a
   * windowed register means inside the current slice, not merely present in
   * `rows`. A selection scrolled out of the window cannot hold the tab stop.
   */
  private get selectionIsRendered(): boolean {
    if (this.activeId === null) return false;
    const index = this.rows.findIndex((row) => row.id === this.activeId);
    const { start, end } = this.windowRange;
    return index >= start && index < end;
  }

  /**
   * Focus arriving on a row.
   *
   * Focusing the fallback tab stop is not choosing it: Tab returns to the
   * table without moving the cursor off the row that has it. A row focused
   * any other way — clicked, or tabbed to while the selection sits on it —
   * becomes the selection as it always did.
   */
  private handleRowFocusIn(row: RegisterTableRow, event: FocusEvent): void {
    if (this.activeId === row.id) return;
    // Focus landing on the flag button is a flag click or a Tab to it, not
    // a choice of row.
    const target = event.composedPath()[0];
    if (target instanceof HTMLElement && target.classList.contains('icon-button')) return;
    if (!this.selectionIsRendered && row.id === this.tabStopId) return;
    this.setActive(row.id);
  }

  private rowElement(id: number): HTMLElement | null {
    return this.shadowRoot?.querySelector<HTMLElement>(`tr[data-id="${id}"]`) ?? null;
  }

  /**
   * Correct an end-aligned scroll against the row's drawn box. The arithmetic
   * is only as good as the row height it had, and on a first load that can
   * still be the estimate.
   */
  private settleAtEnd(id: number): void {
    const delta = this.overhang(id);
    if (delta !== null && delta !== 0 && this.scroller) this.scroller.scrollTop += delta;
  }

  private keepBottomInView(id: number): void {
    const delta = this.overhang(id);
    if (delta !== null && delta > 0 && this.scroller) this.scroller.scrollTop += delta;
  }

  /**
   * How far a row's bottom edge sits below the band rows show in, negative
   * when it is above. Null when nothing can be measured (jsdom, a first
   * paint).
   */
  private overhang(id: number): number | null {
    const scroller = this.scroller;
    const row = this.rowElement(id);
    if (!scroller || !row) return null;

    const frame = scroller.getBoundingClientRect();
    if (frame.height <= 0) return null;
    const foot = this.shadowRoot?.querySelector('tfoot')?.getBoundingClientRect().height ?? 0;
    const delta = row.getBoundingClientRect().bottom - (frame.top + scroller.clientHeight - foot);
    return Number.isFinite(delta) ? delta : null;
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
    const rows = Math.floor(this.visibleRowsHeight() / this.rowHeight);
    // Zero means nothing could be measured, which is jsdom and a first paint.
    // One is a real answer: a window with room for one row pages by one.
    return rows > 0 ? rows : DEFAULT_PAGE_ROWS;
  }

  /**
   * The height rows are actually visible in: the scroller less the header and
   * the total row, which are sticky and painted *over* it. Paging by the whole
   * scroller skips the rows those two cover, every press.
   */
  private visibleRowsHeight(): number {
    const scroller = this.scroller?.clientHeight ?? 0;
    const foot = this.shadowRoot?.querySelector('tfoot')?.getBoundingClientRect().height ?? 0;
    return Math.max(0, scroller - this.stickyHeadHeight() - foot);
  }

  /** Zero in the stacked layout, where the header is kept for assistive technology only. */
  private stickyHeadHeight(): number {
    const head = this.shadowRoot?.querySelector('thead');
    if (!head) return 0;
    if (getComputedStyle(head).position === 'absolute') return 0;
    return head.getBoundingClientRect().height;
  }

  // -- keyboard -------------------------------------------------------------

  /**
   * Keys this table must never look at.
   *
   * A chord belongs to the browser or the OS: `Ctrl`/`Cmd+F` is find-in-page,
   * and a table that read it as the flag shortcut would answer a find with a
   * write against the database; `Ctrl+Home`/`End` are the document's.
   *
   * The split at a control inside a cell is the ARIA grid pattern's. The
   * *activation* keys are the control's, so the flag button answers `Enter`
   * and `Space` identically instead of `Enter` cancelling its click and
   * opening the row editor. The *navigation* keys stay the grid's, so the
   * arrows and the paging keys move between rows from wherever focus is,
   * including from a widget inside a cell.
   */
  private notOurs(event: KeyboardEvent): boolean {
    if (event.ctrlKey || event.metaKey || event.altKey) return true;
    if (!ACTIVATION_KEYS.has(event.key)) return false;
    const target = event.composedPath()[0];
    return (
      target instanceof HTMLElement &&
      target.closest('button, a, input, select, textarea') !== null
    );
  }

  private handleKeydown = (event: KeyboardEvent): void => {
    if (this.notOurs(event)) return;
    if (this.editingId !== null) return;
    if (this.rows.length === 0) return;

    // With nothing selected the keys start from the tab stop — the row focus
    // is on, which after scrollToEnd is the last one, not the first.
    const current = this.activeIndex;
    const stop = this.rows.findIndex((row) => row.id === this.tabStopId);
    const from = current === -1 ? Math.max(stop, 0) : current;

    switch (event.key) {
      case 'ArrowDown':
        this.focusIndex(current === -1 ? from : from + 1);
        break;
      case 'ArrowUp':
        this.focusIndex(current === -1 ? from : from - 1);
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
        // Editing and flagging act on the selection, never on a row the user
        // has not chosen.
        if (current === -1) return;
        this.activateRow(this.rows[from]);
        break;
      case 'Escape':
        // The TUI's clear-the-cursor. Consumed, so it does not also close a
        // popover or leave fullscreen; cancelling an *edit* is the two inline
        // editors' own Escape, and this handler never runs while one is open.
        this.activeId = null;
        event.stopPropagation();
        break;
      case 'f':
      case 'F':
        if (current === -1) return;
        this.requestFlag(this.rows[from]);
        break;
      case '/':
        this.dispatchEvent(
          new CustomEvent('nc-search-focus', { bubbles: true, composed: true }),
        );
        break;
      default:
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

  /**
   * A click selects; a tap also opens the row, because a touch screen has no
   * double-click to spare and no Enter key to press.
   */
  private handleRowClick(row: RegisterTableRow, event: MouseEvent): void {
    const pointerType = 'pointerType' in event ? (event as PointerEvent).pointerType : '';
    if ((pointerType || this.lastPointerType) === 'touch') this.activateRow(row);
    else this.setActive(row.id);
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
    this.lastWindowLength = this.windowLength;
    // Resolved once: the getter walks the rows, and a row must not pay for
    // that on a register of two thousand.
    const tabStop = this.tabStopId;
    const hasFooter = this.total !== undefined || this.footerNote !== '';

    return html`
      <div class="scroller" @scroll=${this.handleScroll}>
        <table role="grid" aria-rowcount=${this.rows.length + (hasFooter ? 2 : 1)}>
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
              <th scope="col" role="columnheader" class="flag">
                <span class="sr-only">Flag</span>
              </th>
              <th scope="col" role="columnheader">Date</th>
              <th scope="col" role="columnheader">Description</th>
              <th scope="col" role="columnheader">Category</th>
              <th scope="col" role="columnheader">Vendor</th>
              <th scope="col" role="columnheader" class="amount">Amount</th>
              ${this.showAccount
                ? html`<th scope="col" role="columnheader">Account</th>`
                : nothing}
            </tr>
          </thead>
          <tbody>
            ${this.renderBody(columns, tabStop)}
          </tbody>
          ${this.total === undefined && this.footerNote === ''
            ? nothing
            : html`
                <tfoot>
                  <tr role="row" aria-rowindex=${this.rows.length + 2}>
                    <td role="gridcell" colspan=${columns}>
                      <div class="foot">
                        <span class="foot-label">
                          ${this.footerLabel}
                          ${this.footerNote
                            ? html`<span class="note"
                                >${this.footerLabel ? '· ' : ''}${this.footerNote}</span
                              >`
                            : nothing}
                        </span>
                        ${this.total === undefined
                          ? nothing
                          : html`<span class="foot-total">
                              Total <wc-money .amount=${this.total}></wc-money>
                            </span>`}
                      </div>
                    </td>
                  </tr>
                </tfoot>
              `}
        </table>
      </div>
    `;
  }

  /**
   * The rows that go in the DOM: the window, plus the row being edited
   * wherever it is.
   *
   * The editor has to survive a scroll. Slicing it out destroys the input and
   * the half-typed vendor with it, so an editing row outside the window is
   * drawn in its own place with spacers on both sides of it. Every gap between
   * drawn rows becomes one spacer carrying the height of what it stands for,
   * which is what keeps the scrollbar the length of the whole register.
   */
  private renderBody(columns: number, tabStop: number | null) {
    const parts: { key: string; part: unknown }[] = [];
    let previous = -1;
    // Spacers are keyed by their ordinal, not by the rows they stand for:
    // there are at most three, and a key derived from the scroll position
    // would destroy and rebuild them on every frame.
    let gaps = 0;

    for (const index of this.renderedIndices) {
      const gap = index - previous - 1;
      if (gap > 0) {
        parts.push({
          key: `gap-${gaps++}`,
          part: this.renderSpacer(gap * this.rowHeight, columns),
        });
      }
      const row = this.rows[index];
      if (row) parts.push({ key: `row-${row.id}`, part: this.renderRow(row, index, tabStop) });
      previous = index;
    }

    const trailing = this.rows.length - previous - 1;
    if (trailing > 0) {
      parts.push({
        key: `gap-${gaps++}`,
        part: this.renderSpacer(trailing * this.rowHeight, columns),
      });
    }

    // Keyed, so a row moving between segments — which is what an editing row
    // does the moment the window scrolls past it — has its DOM moved rather
    // than rebuilt. Rebuilding it would replace the inputs and lose whatever
    // had been typed into them.
    return repeat(
      parts,
      (item) => item.key,
      (item) => item.part,
    );
  }

  /** Ascending, no duplicates. */
  private get renderedIndices(): number[] {
    const { start, end } = this.windowRange;
    const indices: number[] = [];
    for (let index = start; index < end; index += 1) indices.push(index);
    if (!this.windowing || this.editingId === null) return indices;

    const editing = this.rows.findIndex((row) => row.id === this.editingId);
    if (editing < 0 || (editing >= start && editing < end)) return indices;
    if (editing < start) indices.unshift(editing);
    else indices.push(editing);
    return indices;
  }

  /** Height standing in for the rows not drawn, so the scrollbar is the
      length of the whole register rather than of what is on screen. */
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
        @click=${(event: Event) => {
          // A flag click is about that row's flag, not about which row is
          // current: the row's own click must not hear it.
          event.stopPropagation();
          this.requestFlag(row);
        }}
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
        data-uncategorized=${row.categoryId === null ? 'true' : 'false'}
        ?data-editing=${editing}
        aria-selected=${selected ? 'true' : 'false'}
        aria-busy=${row.id === this.busyId ? 'true' : 'false'}
        aria-rowindex=${index + 2}
        tabindex=${row.id === tabStopId ? '0' : '-1'}
        @focusin=${(event: FocusEvent) => this.handleRowFocusIn(row, event)}
        @click=${(event: MouseEvent) => this.handleRowClick(row, event)}
        @dblclick=${() => this.activateRow(row)}
      >
        <td role="gridcell" class="flag">
          ${this.readonly ? this.renderFlagMark(row) : this.renderFlagButton(row)}
        </td>
        <td role="gridcell" class="date">${row.date}</td>
        <td role="gridcell" class="text description" title=${row.description}>
          ${row.description}
        </td>
        ${editing
          ? this.renderEditCells(row)
          : html`
              <td role="gridcell" class="text category" title=${row.category ?? ''}>
                ${row.category ?? html`<span class="muted uncategorized">Uncategorized</span>`}
              </td>
              <td
                role="gridcell"
                class=${row.vendor ? 'text vendor' : 'text vendor blank'}
                title=${row.vendor ?? ''}
              >
                ${row.vendor ?? ''}
              </td>
            `}
        <td role="gridcell" class="amount" title=${formatMoney(row.amount)}>
          <wc-money .amount=${row.amount} align="end"></wc-money>
        </td>
        ${this.showAccount
          ? html`<td
              role="gridcell"
              class=${editing ? 'account' : 'text account'}
              title=${row.accountName}
            >
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
      <td role="gridcell" class="category">
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
      <td role="gridcell" class="vendor">
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
