import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '../icons/icons.js';
import { controlsCss } from '@nigel/theme';
import './wc-row-badge.js';

/** A column in a manager list. `key` is for keying the cells, not for lookup. */
export interface ManagerColumn {
  key: string;
  label: string;
  align?: 'start' | 'end';
  /** Tabular figures — patterns, form lines and anything else read character by character. */
  mono?: boolean;
}

/** A per-row button. `name` is what comes back on the event. */
export interface ManagerAction {
  name: string;
  label: string;
  /** Tag name of a `wc-icon-*`. */
  icon?: string;
  variant?: 'default' | 'danger';
}

export interface ManagerRow {
  id: number;
  /** One per column, in column order. `null` renders as an em dash. */
  cells: (string | number | null)[];
  /**
   * A state worth marking but not worth a column — "Archived" on a client.
   * Rendered beside the first cell, because that is the one every list keys
   * its rows by.
   */
  badge?: string;
  /**
   * This row's buttons, replacing the table's own. For a list where the verb
   * depends on the row's state — Archive on an active client, Unarchive on an
   * archived one — which one array for the whole table cannot express.
   */
  actions?: ManagerAction[];
  /**
   * What this row is called, for the action buttons' labels. A column of
   * buttons that all read "Delete" is unusable with a screen reader.
   */
  label: string;
}

export interface NcManagerActionDetail {
  action: string;
  id: number;
}

/**
 * The list every manager screen is built on: columns, rows, per-row actions.
 *
 * Not `wc-report-table`, which renders section and total rows for read-only
 * financial output. These rows carry an id, a busy state and buttons, and never
 * a total; sharing one component would put write semantics into a report
 * renderer for the sake of a shared `<table>` tag.
 */
@customElement('wc-manager-table')
export class WcManagerTable extends LitElement {
  static styles = [
    controlsCss,
    css`
      /* The refresh's table card: the card ground, a 1px border and the
         card radius, scrolling sideways inside itself on a narrow window. */
      :host {
        display: block;
        overflow-x: auto;
        background: var(--wa-color-surface);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-lg);
      }

      /* Inside a container that is already a card, such as a wc-panel, the
         table drops its own card so the two do not nest. */
      :host([plain]) {
        background: none;
        border: 0;
        border-radius: 0;
      }

      table {
        width: 100%;
        border-collapse: collapse;
        font-size: var(--nc-font-size-body);
        color: var(--wa-color-text);
      }

      caption {
        position: absolute;
        width: 1px;
        height: 1px;
        overflow: hidden;
        clip-path: inset(50%);
        white-space: nowrap;
      }

      th,
      td {
        text-align: start;
        padding: 0 8px;
        vertical-align: middle;
      }

      /* The column every list keys its rows by keeps room for a short name
         on one line when the action buttons claim the rest of the width. */
      th:first-child,
      td:first-child {
        min-width: 14ch;
        padding-left: 16px;
      }

      th:last-child,
      td:last-child {
        padding-right: 12px;
      }

      th {
        height: 36px;
        background: var(--wa-color-surface-alt);
        border-bottom: 1px solid var(--wa-color-border);
        font-size: var(--nc-font-size-meta);
        font-weight: var(--nc-font-weight-table-heading);
        color: var(--wa-color-muted);
        white-space: nowrap;
      }

      /* A 44px row: a table cell's height does not count its padding. */
      td {
        height: 32px;
        padding-block: 6px;
        border-bottom: 1px solid var(--wa-color-border-soft);
        user-select: text;
      }

      tbody tr:last-child td {
        border-bottom: 0;
      }

      tbody tr:hover td {
        background: var(--wa-color-surface-alt);
      }

      td.end,
      th.end {
        text-align: end;
        font-variant-numeric: tabular-nums;
      }

      td.mono {
        font-family: var(--wa-font-family-mono);
        overflow-wrap: anywhere;
      }

      .muted {
        color: var(--wa-color-muted);
      }

      tr[aria-busy='true'] {
        opacity: 0.6;
      }

      .actions {
        display: flex;
        justify-content: flex-end;
        gap: 6px;
        white-space: nowrap;
        user-select: none;
      }

      th.actions-header {
        text-align: end;
      }

      /* The card's own width picks the layout (the stacked rules below). */
      .frame {
        container-type: inline-size;
      }

      /* -- the stacked layout --

         Below 720px the columns and the row's buttons no longer fit side by
         side, so each row becomes a card in the table's own width, as the
         register's rows do: the first column as the row's title, every other
         column as a labelled line under it, the buttons last. Nothing
         scrolls sideways.

         The elements keep their table roles explicitly, so the cells are
         still announced as cells of their columns; the header row is there
         for assistive technology and takes no room. */
      @container (max-width: 720px) {
        table,
        tbody,
        tr,
        td {
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

        tbody tr {
          display: grid;
          gap: 4px;
          padding: 12px 16px;
          border-bottom: 1px solid var(--wa-color-border-soft);
        }

        tbody tr:last-child {
          border-bottom: 0;
        }

        tbody tr:hover {
          background: var(--wa-color-surface-alt);
        }

        tbody tr:hover td {
          background: none;
        }

        td,
        td:first-child,
        td:last-child {
          height: auto;
          padding: 0;
          border-bottom: 0;
          min-width: 0;
        }

        td.end {
          text-align: start;
        }

        /* The first column is what the list keys its rows by: the card's
           title. */
        td:first-child {
          font-weight: var(--wa-font-weight-medium);
        }

        /* Every other column is a line of its own, its heading beside it. */
        td[data-label] {
          display: grid;
          grid-template-columns: minmax(0, 9rem) minmax(0, 1fr);
          gap: 12px;
          font-size: var(--nc-font-size-control);
          overflow-wrap: anywhere;
        }

        td[data-label]::before {
          content: attr(data-label);
          color: var(--wa-color-muted);
          font-size: var(--nc-font-size-meta);
          font-weight: var(--nc-font-weight-table-heading);
          line-height: inherit;
        }

        td.actions-cell {
          padding-top: 6px;
        }

        .actions {
          flex-wrap: wrap;
          justify-content: flex-start;
        }
      }
    `,
  ];

  @property({ attribute: false })
  columns: ManagerColumn[] = [];

  @property({ attribute: false })
  rows: ManagerRow[] = [];

  @property({ attribute: false })
  actions: ManagerAction[] = [];

  /** The row with a request in flight: its buttons go inert. */
  @property({ type: Number, attribute: false })
  busyId: number | null = null;

  /** Drop the table card, for a list placed inside a card of its own. */
  @property({ type: Boolean, reflect: true })
  plain = false;

  /** Visually hidden table caption — what this list is. */
  @property({ type: String })
  caption = '';

  private handleAction(action: string, id: number): void {
    this.dispatchEvent(
      new CustomEvent<NcManagerActionDetail>('nc-manager-action', {
        detail: { action, id },
        bubbles: true,
        composed: true,
      }),
    );
  }

  render() {
    // Whether there is an Actions column at all: this table's own buttons, or
    // any row's. Computed once and passed down, because a getter that walks
    // every row from inside every row is quadratic.
    const hasActions =
      this.actions.length > 0 || this.rows.some((row) => (row.actions?.length ?? 0) > 0);

    // The roles are stated because the stacked layout sets the table's
    // elements to display: block, which some engines take as leaving the
    // table: the rows have to stay rows of cells for a screen reader.
    return html`
      <div class="frame">
        <table role="table">
          ${this.caption ? html`<caption>${this.caption}</caption>` : nothing}
          <thead role="rowgroup">
            <tr role="row">
              ${this.columns.map(
                (column) => html`
                  <th
                    scope="col"
                    role="columnheader"
                    class=${column.align === 'end' ? 'end' : ''}
                  >
                    ${column.label}
                  </th>
                `,
              )}
              ${hasActions
                ? html`<th scope="col" role="columnheader" class="actions-header">Actions</th>`
                : nothing}
            </tr>
          </thead>
          <tbody role="rowgroup">
            ${this.rows.map((row) => this.renderRow(row, hasActions))}
          </tbody>
        </table>
      </div>
    `;
  }

  private renderRow(row: ManagerRow, hasActions: boolean) {
    const busy = this.busyId === row.id;
    const actions = row.actions ?? this.actions;

    return html`
      <tr role="row" data-row=${row.id} aria-busy=${busy ? 'true' : 'false'}>
        ${this.columns.map((column, index) => {
          const value = row.cells[index] ?? null;
          const classes = [
            column.align === 'end' ? 'end' : '',
            column.mono ? 'mono' : '',
            value === null ? 'muted' : '',
          ]
            .filter(Boolean)
            .join(' ');
          // Every column but the first carries its heading, which the
          // stacked layout shows beside the value.
          return html`<td
            role="cell"
            class=${classes}
            data-label=${index === 0 ? nothing : column.label}
          >
            ${value === null ? '—' : value}
            ${index === 0 && row.badge
              ? html`<wc-row-badge label=${row.badge}></wc-row-badge>`
              : nothing}
          </td>`;
        })}
        ${hasActions
          ? html`
              <td role="cell" class="actions-cell">
                <div class="actions">
                  ${actions.map(
                    (action) => html`
                      <wa-button
                        data-action=${action.name}
                        size="s"
                        appearance="outlined"
                        variant=${action.variant === 'danger' ? 'danger' : 'neutral'}
                        ?disabled=${busy}
                        @click=${() => this.handleAction(action.name, row.id)}
                      >
                        ${action.icon
                          ? html`<span slot="start">${iconFor(action.icon)}</span>`
                          : nothing}
                        ${action.label}<span class="sr-only"> ${row.label}</span>
                      </wa-button>
                    `,
                  )}
                </div>
              </td>
            `
          : nothing}
      </tr>
    `;
  }
}

/**
 * The three glyphs a manager row can carry.
 *
 * A lookup rather than `unsafeStatic`, because the tag name arrives from a
 * caller and building an element out of caller-supplied text is how a template
 * injection starts. The set is small and closed on purpose.
 */
function iconFor(tag: string) {
  switch (tag) {
    case 'wc-icon-edit':
      return html`<wc-icon-edit></wc-icon-edit>`;
    case 'wc-icon-trash':
      return html`<wc-icon-trash></wc-icon-trash>`;
    case 'wc-icon-plus':
      return html`<wc-icon-plus></wc-icon-plus>`;
    default:
      return nothing;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-manager-table': WcManagerTable;
  }
}
