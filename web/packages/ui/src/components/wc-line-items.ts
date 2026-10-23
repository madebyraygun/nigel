import { LitElement, html, css, nothing } from 'lit';
import { roundHalfEven } from './round-half-even.js';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/textarea/textarea.js';
import '../icons/icons.js';
import './wc-money.js';
import { controlsCss } from '@nigel/theme';

/**
 * One line as it is being edited.
 *
 * Strings, not numbers: these are the characters in three inputs, and a field
 * a user has cleared is `''` rather than a `0` nobody typed. Parsing happens
 * once, in `lineItemAmount` and in the screen that builds the request.
 */
export interface LineItemValue {
  description: string;
  quantity: string;
  unitAmount: string;
  /**
   * The row's total as the server states it, for rows nobody is editing.
   *
   * The editor omits it: there, the amount is whatever the two typed figures
   * currently multiply to. A read-only row supplies it because `quantity` is
   * rendered to two decimals and re-multiplying a rounded quantity is not the
   * figure the invoice was billed at.
   */
  amount?: string;
}

export interface LineItemErrors {
  description?: string;
  quantity?: string;
  unitAmount?: string;
}

export interface NcLineItemsChangeDetail {
  items: LineItemValue[];
}

export const EMPTY_LINE_ITEM: LineItemValue = {
  description: '',
  quantity: '1',
  unitAmount: '',
};

/** A typed figure as a number, or null when it is not one. */
export function parseLineNumber(raw: string): number | null {
  const cleaned = raw.replace(/,/g, '').trim();
  if (cleaned === '') return null;
  const parsed = Number(cleaned);
  return Number.isFinite(parsed) ? parsed : null;
}

/**
 * What a row comes to, or null when either figure is missing or unreadable.
 *
 * A stated `amount` wins over the product: the quantity column is rounded for
 * display, so recomputing would contradict the figure the row was billed at.
 */
export function lineItemAmount(item: LineItemValue): number | null {
  if (item.amount !== undefined) return parseLineNumber(item.amount);
  const quantity = parseLineNumber(item.quantity);
  const unit = parseLineNumber(item.unitAmount);
  if (quantity === null || unit === null) return null;
  const amount = quantity * unit;
  return Number.isFinite(amount) ? amount : null;
}

/** The sum of every readable row. Unreadable rows contribute nothing. */
export function lineItemsSubtotal(items: LineItemValue[]): number {
  return items.reduce((sum, item) => sum + (lineItemAmount(item) ?? 0), 0);
}

/** A row nobody has typed anything into — dropped before the request. */
export function isBlankLineItem(item: LineItemValue): boolean {
  return item.description.trim() === '' && item.unitAmount.trim() === '';
}

/**
 * The repeatable line-item rows an invoice is built from.
 *
 * Reordering is up/down buttons rather than drag and drop: a drag handle has
 * no keyboard equivalent that passes axe without building the buttons anyway,
 * and an invoice has five rows, not five hundred.
 *
 * Controlled, like every other form component here — it renders `items` and
 * emits the whole array on every edit, because the screen owns the request.
 */
@customElement('wc-line-items')
export class WcLineItems extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: block;
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
      }

      .scroll {
        overflow-x: auto;
      }

      table {
        width: 100%;
        border-collapse: collapse;
        font-size: var(--wa-font-size-s);
      }

      caption {
        text-align: start;
        font-weight: var(--wa-font-weight-medium);
        padding-bottom: var(--wa-space-xs);
      }

      caption.visually-hidden {
        position: absolute;
        width: 1px;
        height: 1px;
        overflow: hidden;
        clip-path: inset(50%);
        white-space: nowrap;
      }

      th,
      td {
        padding: var(--wa-space-2xs) var(--wa-space-xs);
        border-bottom: 1px solid var(--wa-color-border);
        text-align: start;
        vertical-align: top;
      }

      th {
        color: var(--wa-color-muted);
        font-weight: var(--nc-font-weight-table-heading);
        white-space: nowrap;
      }

      td.end,
      th.end {
        text-align: end;
      }

      /* The description takes the row; every other column is as wide as
         what it holds. A width of 1% on a cell in an auto-layout table is
         "no wider than the content", which is what lets 100% mean "the rest". */
      .description {
        width: 100%;
        min-width: 12rem;
        overflow-wrap: anywhere;
        white-space: normal;
      }

      .numeric {
        width: 1%;
        white-space: nowrap;
      }

      /* Qty is about four characters, Unit a typical amount with its $. */
      td.qty wa-input {
        width: 5.5rem;
      }

      td.unit wa-input {
        width: 9.5rem;
      }

      td.figure wa-input::part(input) {
        text-align: end;
        font-variant-numeric: tabular-nums;
      }

      td.description wa-textarea {
        width: 100%;
      }

      /* In the editor every cell is top-aligned, and the text of a cell with
         no field in it is pushed down to the first line of the fields beside
         it, so a wrapped description grows the row without moving the
         amount or the row controls off its first line. */
      tr.editing td.amount {
        padding-top: calc(var(--wa-space-2xs) + 5px);
      }

      /* The label names each field for a screen reader; the column header
         shows a sighted user the same thing, so it takes no space. */
      td :is(wa-input, wa-textarea)::part(form-control-label) {
        position: absolute;
        width: 1px;
        height: 1px;
        overflow: hidden;
        clip-path: inset(50%);
        white-space: nowrap;
      }

      .prefix {
        color: var(--wa-color-muted);
      }

      .error {
        margin: var(--wa-space-2xs) 0 0;
        color: var(--wa-color-danger);
        font-size: var(--wa-font-size-s);
      }

      .row-actions {
        display: flex;
        gap: var(--wa-space-2xs);
        justify-content: flex-end;
        white-space: nowrap;
      }

      tr[data-emphasis] td {
        font-weight: var(--wa-font-weight-medium);
        border-bottom: 0;
      }

      tr[data-emphasis='total'] td {
        font-weight: var(--wa-font-weight-bold);
        border-top: 2px solid var(--wa-color-border);
      }

      footer {
        margin-top: var(--wa-space-s);
      }

      /* The detail view's read-only table: a 12px muted header over a full
         rule, soft rules between rows, and the Total in 600. */
      :host([readonly]) table {
        font-size: var(--nc-font-size-body);
      }

      :host([readonly]) th {
        padding: 0 0 8px;
        font-size: var(--nc-font-size-meta);
        border-bottom: 1px solid var(--wa-color-border);
      }

      :host([readonly]) td {
        padding: 12px 0;
        border-bottom: 1px solid var(--wa-color-border-soft);
        user-select: text;
      }

      :host([readonly]) th + th,
      :host([readonly]) td + td {
        padding-left: 12px;
      }

      /* Narrow figure columns sit close together; a wider gutter keeps a rate
         and its amount from reading as one number. */
      :host([readonly]) th.numeric,
      :host([readonly]) td.numeric {
        padding-left: 24px;
      }

      :host([readonly]) tr[data-emphasis] td {
        border-bottom: 0;
      }

      :host([readonly]) tr[data-emphasis='total'] td {
        border-top: 0;
        font-weight: var(--wa-font-weight-bold);
      }

      .list-error {
        margin: var(--wa-space-xs) 0 0;
        color: var(--wa-color-danger);
        font-size: var(--wa-font-size-s);
      }

      /*
       * The row buttons are glyphs, and a glyph is not a name. The name is
       * slotted text so it reaches the button Web Awesome renders inside its own
       * shadow root — an aria-label on the host would never get there.
       */
      .sr-only {
        position: absolute;
        width: 1px;
        height: 1px;
        overflow: hidden;
        clip-path: inset(50%);
        white-space: nowrap;
      }
    `,
  ];

  @property({ attribute: false })
  items: LineItemValue[] = [];

  /** One entry per row, in row order. Missing entries mean no errors. */
  @property({ attribute: false })
  errors: LineItemErrors[] = [];

  /** A list-level refusal, e.g. "An invoice needs at least one line". */
  @property({ type: String, attribute: 'list-error' })
  listError = '';

  /** Read-only rows: the detail view's line-item table. */
  @property({ type: Boolean, reflect: true })
  readonly = false;

  /** Every control inert while a save is in flight. */
  @property({ type: Boolean })
  disabled = false;

  /**
   * The invoice's total, when there is one to show beneath the subtotal.
   *
   * Null on the editor, where the total is whatever the rows currently add up
   * to and a second identical figure would only be noise.
   */
  @property({ attribute: false })
  total: number | null = null;

  /** The invoice's currency, so every figure in the table reads in it. */
  @property({ type: String })
  currency = 'USD';

  @property({ type: String })
  caption = 'Line items';

  @property({ type: Boolean, attribute: 'caption-hidden' })
  captionHidden = false;

  private emit(items: LineItemValue[]): void {
    this.dispatchEvent(
      new CustomEvent<NcLineItemsChangeDetail>('nc-line-items-change', {
        detail: { items },
        bubbles: true,
        composed: true,
      }),
    );
  }

  /**
   * A description is one line on the invoice: the page and the PDF print it
   * as a single run of text, so a newline would be lost on the way out.
   * Enter therefore adds nothing, and a pasted newline becomes a space; the
   * field still wraps on screen to show all of it.
   */
  private handleDescriptionKeydown = (event: KeyboardEvent): void => {
    if (event.key === 'Enter') event.preventDefault();
  };

  private handleDescription(index: number) {
    return (event: Event) => {
      const field = event.target as HTMLTextAreaElement;
      const value = field.value.replace(/\r?\n/g, ' ');
      if (value !== field.value) field.value = value;
      this.emit(this.items.map((item, i) => (i === index ? { ...item, description: value } : item)));
    };
  }

  private handleField(index: number, field: keyof LineItemValue) {
    return (event: Event) => {
      const input = event.target as HTMLInputElement;
      this.emit(
        this.items.map((item, i) => (i === index ? { ...item, [field]: input.value } : item)),
      );
    };
  }

  private addRow = (): void => {
    this.emit([...this.items, { ...EMPTY_LINE_ITEM }]);
  };

  private removeRow(index: number): void {
    this.emit(this.items.filter((_, i) => i !== index));
  }

  private move(index: number, delta: number): void {
    const target = index + delta;
    if (target < 0 || target >= this.items.length) return;
    const next = [...this.items];
    const [row] = next.splice(index, 1);
    next.splice(target, 0, row);
    this.emit(next);
  }

  render() {
    const subtotal = lineItemsSubtotal(this.items);
    // Read-only, a Subtotal equal to the Total beneath it is the same figure
    // twice; it earns its row only when something (tax) sits between them.
    const hideSubtotal =
      this.readonly && this.total !== null && roundHalfEven(subtotal, 2) === roundHalfEven(this.total, 2);

    return html`
      <div class="scroll">
        <table>
          <caption class=${this.captionHidden ? 'visually-hidden' : ''}>
            ${this.caption}
          </caption>
          <thead>
            <tr>
              <th scope="col" class="description">Description</th>
              <th scope="col" class="end numeric">Qty</th>
              <th scope="col" class="end numeric">${this.readonly ? 'Rate' : 'Unit'}</th>
              <th scope="col" class="end numeric">Amount</th>
              ${this.readonly ? nothing : html`<th scope="col" class="end numeric">Row</th>`}
            </tr>
          </thead>
          <tbody>
            ${this.items.map((item, index) => this.renderRow(item, index))}
          </tbody>
          <tfoot>
            ${hideSubtotal
              ? nothing
              : html`<tr data-emphasis="subtotal">
              <td colspan="3">Subtotal</td>
              <td class="end">
                <wc-money
                  .amount=${subtotal}
                  .currency=${this.currency}
                  variant="plain"
                  align="end"
                ></wc-money>
              </td>
              ${this.readonly ? nothing : html`<td></td>`}
            </tr>`}
            ${this.total === null
              ? nothing
              : html`
                  <tr data-emphasis="total">
                    <td colspan="3">Total</td>
                    <td class="end">
                      <wc-money
                        .amount=${this.total}
                        .currency=${this.currency}
                        variant="plain"
                        align="end"
                      ></wc-money>
                    </td>
                    ${this.readonly ? nothing : html`<td></td>`}
                  </tr>
                `}
          </tfoot>
        </table>
      </div>

      ${this.listError
        ? html`<p class="list-error" role="alert">${this.listError}</p>`
        : nothing}
      ${this.readonly
        ? nothing
        : html`
            <footer>
              <wa-button
                data-add-row
                size="s"
                appearance="outlined"
                ?disabled=${this.disabled}
                @click=${this.addRow}
              >
                <wc-icon-plus slot="start"></wc-icon-plus>
                Add row
              </wa-button>
            </footer>
          `}
    `;
  }

  private renderRow(item: LineItemValue, index: number) {
    const amount = lineItemAmount(item);
    const errors = this.errors[index] ?? {};
    const label = item.description.trim() || `row ${index + 1}`;

    if (this.readonly) {
      return html`
        <tr data-row=${index}>
          <td class="description">${item.description}</td>
          <td class="end numeric">${item.quantity}</td>
          <td class="end numeric">
            <wc-money
              .amount=${parseLineNumber(item.unitAmount) ?? 0}
              .currency=${this.currency}
              variant="plain"
              align="end"
            ></wc-money>
          </td>
          <td class="end numeric">
            <wc-money
              .amount=${amount ?? 0}
              .currency=${this.currency}
              variant="plain"
              align="end"
            ></wc-money>
          </td>
        </tr>
      `;
    }

    return html`
      <tr data-row=${index} class="editing">
        <td class="description">
          <wa-textarea
            data-description
            size="s"
            rows="1"
            resize="auto"
            autocomplete="off"
            label=${`Description, line ${index + 1}`}
            .value=${item.description}
            ?disabled=${this.disabled}
            @keydown=${this.handleDescriptionKeydown}
            @input=${this.handleDescription(index)}
          ></wa-textarea>
          ${errors.description
            ? html`<p class="error" role="alert">${errors.description}</p>`
            : nothing}
        </td>
        <td class="figure end numeric qty">
          <wa-input
            data-quantity
            size="s"
            inputmode="decimal"
            autocomplete="off"
            spellcheck="false"
            label=${`Quantity, line ${index + 1}`}
            .value=${item.quantity}
            ?disabled=${this.disabled}
            @input=${this.handleField(index, 'quantity')}
          ></wa-input>
          ${errors.quantity
            ? html`<p class="error" role="alert">${errors.quantity}</p>`
            : nothing}
        </td>
        <td class="figure end numeric unit">
          <wa-input
            data-unit
            size="s"
            inputmode="decimal"
            autocomplete="off"
            spellcheck="false"
            label=${`Unit amount, line ${index + 1}`}
            .value=${item.unitAmount}
            ?disabled=${this.disabled}
            @input=${this.handleField(index, 'unitAmount')}
          >
            <span slot="start" class="prefix" data-prefix aria-hidden="true">$</span>
          </wa-input>
          ${errors.unitAmount
            ? html`<p class="error" role="alert">${errors.unitAmount}</p>`
            : nothing}
        </td>
        <td class="end numeric amount">
          ${amount === null
            ? html`<span class="prefix">—</span>`
            : html`<wc-money
                .amount=${amount}
                .currency=${this.currency}
                variant="plain"
                align="end"
              ></wc-money>`}
        </td>
        <td class="numeric">
          <div class="row-actions">
            <wa-button
              data-up
              size="s"
              appearance="plain"
              ?disabled=${this.disabled || index === 0}
              @click=${() => this.move(index, -1)}
            >
              <span aria-hidden="true">↑</span>
              <span class="sr-only">${`Move ${label} up`}</span>
            </wa-button>
            <wa-button
              data-down
              size="s"
              appearance="plain"
              ?disabled=${this.disabled || index === this.items.length - 1}
              @click=${() => this.move(index, 1)}
            >
              <span aria-hidden="true">↓</span>
              <span class="sr-only">${`Move ${label} down`}</span>
            </wa-button>
            <wa-button
              data-remove
              size="s"
              appearance="plain"
              variant="danger"
              ?disabled=${this.disabled}
              @click=${() => this.removeRow(index)}
            >
              <wc-icon-trash></wc-icon-trash>
              <span class="sr-only">${`Remove ${label}`}</span>
            </wa-button>
          </div>
        </td>
      </tr>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-line-items': WcLineItems;
  }
  interface HTMLElementEventMap {
    'nc-line-items-change': CustomEvent<NcLineItemsChangeDetail>;
  }
}
