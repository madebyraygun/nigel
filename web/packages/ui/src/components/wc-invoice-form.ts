import { LitElement, html, css, nothing, type PropertyValues } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import './wc-line-items.js';
import { controlsCss } from '@nigel/theme';
import {
  EMPTY_LINE_ITEM,
  isBlankLineItem,
  lineItemsSubtotal,
  parseLineNumber,
  type LineItemErrors,
  type LineItemValue,
  type NcLineItemsChangeDetail,
} from './wc-line-items.js';

/** A client the invoice can be billed to. */
export interface InvoiceClientOption {
  id: number;
  name: string;
  /** Null shows the "cannot be sent" hint under the picker. */
  email: string | null;
}

/** How the due date was chosen, which decides whether it moves on its own. */
export const DUE_TERM_VALUES = ['none', 'net7', 'net14', 'net30', 'custom'] as const;

export type DueTerm = (typeof DUE_TERM_VALUES)[number];

/** The choice a new invoice opens on, before anyone touches the control. */
export const DEFAULT_DUE_TERM: DueTerm = 'net14';

export interface InvoiceFormValue {
  /** The chosen client's id as a string — a select's value always is one. */
  clientId: string;
  issueDate: string;
  /** Empty means no due date, which is what stops it ever going overdue. */
  dueDate: string;
  /** Which of the due-date choices `dueDate` came from. */
  dueTerm: DueTerm;
  currency: string;
  notes: string;
  terms: string;
  items: LineItemValue[];
}

export interface InvoiceFormErrors {
  clientId?: string;
  issueDate?: string;
  dueDate?: string;
  currency?: string;
  /** A refusal about the list as a whole, not about one row. */
  items?: string;
  /** One entry per row, in row order. */
  itemErrors?: LineItemErrors[];
}

export interface NcInvoiceFormChangeDetail {
  value: InvoiceFormValue;
}

export type WcInvoiceFormMode = 'create' | 'edit';

const DATE_PATTERN = /^\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$/;
const CURRENCY_PATTERN = /^[A-Za-z]{3}$/;

export const EMPTY_INVOICE_FORM: InvoiceFormValue = {
  clientId: '',
  issueDate: '',
  dueDate: '',
  dueTerm: DEFAULT_DUE_TERM,
  currency: 'USD',
  notes: '',
  terms: '',
  items: [{ ...EMPTY_LINE_ITEM }],
};

/** How many days after the issue date each net preset falls. */
const NET_TERM_DAYS = { net7: 7, net14: 14, net30: 30 } as const;

type NetDueTerm = keyof typeof NET_TERM_DAYS;

export function isNetDueTerm(term: string): term is NetDueTerm {
  return term in NET_TERM_DAYS;
}

export function isDueTerm(term: string): term is DueTerm {
  return (DUE_TERM_VALUES as readonly string[]).includes(term);
}

/** What the due-date control calls each choice. */
export const DUE_TERM_LABELS: Record<DueTerm, string> = {
  none: 'No due date',
  net7: 'Net 7',
  net14: 'Net 14',
  net30: 'Net 30',
  custom: 'Custom date',
};

/** What a net preset writes into the terms field; the other choices write nothing. */
function termsTextFor(term: DueTerm): string {
  return isNetDueTerm(term) ? `Net ${NET_TERM_DAYS[term]}` : '';
}

/** What a net preset can say about its due date. */
export type DueDateOutcome =
  | { kind: 'ok'; dueDate: string }
  /** No issue date has been given yet. */
  | { kind: 'pending' }
  /** There is an issue date and it is not a day. */
  | { kind: 'unreadable' }
  /** The period lands past the last date this form can write. */
  | { kind: 'unreachable' };

/** The date an outcome names, or empty for the three that name none. */
export function dueDateOf(outcome: DueDateOutcome): string {
  return outcome.kind === 'ok' ? outcome.dueDate : '';
}

/**
 * A `YYYY-MM-DD` as a UTC timestamp, or null if that is not a real day.
 *
 * UTC rather than local, so arithmetic across a daylight-saving boundary
 * cannot land an hour short and round down to the previous day. `Date.UTC`
 * maps years 0-99 to 1900-1999, so the year is set back afterwards and checked
 * with the rest of the round trip — the API takes any four-digit year, and a
 * mistyped `0026` must not book a due date nineteen centuries out. The round
 * trip is also what rejects `2026-02-30`, which the shape check alone lets
 * through and `Date.UTC` would silently roll forward to March.
 */
function utcDay(date: string): number | null {
  const trimmed = date.trim();
  if (!DATE_PATTERN.test(trimmed)) return null;
  const [year, month, day] = trimmed.split('-').map(Number);
  const at = new Date(Date.UTC(year, month - 1, day));
  at.setUTCFullYear(year);
  return at.getUTCFullYear() === year &&
    at.getUTCMonth() + 1 === month &&
    at.getUTCDate() === day
    ? at.getTime()
    : null;
}

/**
 * `date` plus `days`, zero-padded, or empty when that cannot be said.
 *
 * Empty covers both a date the rule cannot read and a sum past year 9999,
 * where the ISO string grows a sign and six digits and stops being something
 * the API would take.
 */
export function addDays(date: string, days: number): string {
  const stamp = utcDay(date);
  if (stamp === null) return '';
  const moved = new Date(stamp + days * 86_400_000).toISOString().slice(0, 10);
  return DATE_PATTERN.test(moved) ? moved : '';
}

/** The due date a net preset implies, or why it has none. */
export function netDueDateFor(issueDate: string, term: NetDueTerm): DueDateOutcome {
  if (issueDate.trim() === '') return { kind: 'pending' };
  if (utcDay(issueDate) === null) return { kind: 'unreadable' };
  const dueDate = addDays(issueDate, NET_TERM_DAYS[term]);
  return dueDate === '' ? { kind: 'unreachable' } : { kind: 'ok', dueDate };
}

/**
 * The choice an existing invoice opens as.
 *
 * Nothing is inferred from the dates. A due date thirty days after the issue
 * date may be a Net 30 or a date somebody picked that happens to land there,
 * and reading it as a preset would move a stored due date the moment the issue
 * date was edited. A stored date opens as a date; a preset is only ever an
 * explicit choice made in the session that makes it.
 */
export function dueTermFor(dueDate: string): DueTerm {
  return dueDate.trim() === '' ? 'none' : 'custom';
}

/**
 * The terms field after a due-date choice, leaving anything the form did not write.
 *
 * The reference invoices print `2026-09-05 (Net 30)`, so a net preset that did
 * not say so in the terms would raise an invoice whose page contradicts the
 * form. `written` is the text this form put there itself; text that matches it
 * by coincidence — a `Net 30` loaded off the invoice or typed by hand — is not
 * the form's to rewrite, which is why provenance is passed in rather than
 * guessed at from the value.
 */
export function prefilledTerms(
  terms: string,
  term: DueTerm,
  written: string | null,
): string {
  if (terms.trim() !== '' && terms !== written) return terms;
  return termsTextFor(term);
}

/**
 * The form with a new issue date, carrying a preset-derived due date along.
 *
 * A date picked by hand, and the absence of one, stay where they are: only a
 * net period is defined relative to the issue date. A period the new issue
 * date cannot date leaves the due date empty and the choice alone — the
 * operator is editing a date, not changing their terms, and the field's own
 * hint says what is missing.
 */
export function withIssueDate(
  value: InvoiceFormValue,
  issueDate: string,
): InvoiceFormValue {
  const next = { ...value, issueDate };
  if (!isNetDueTerm(value.dueTerm)) return next;
  return { ...next, dueDate: dueDateOf(netDueDateFor(issueDate, value.dueTerm)) };
}

/**
 * The form with a new due-date choice.
 *
 * `none` clears the date, which is the form's way of saying the invoice never
 * goes overdue. `custom` keeps whatever date is showing as the starting point
 * for the picker, so choosing Net 30 and then reaching for the calendar does
 * not blank the field first. A preset whose date is merely waiting on the
 * issue date is kept — `withIssueDate` fills it in — but one that could never
 * be dated falls back to `none` rather than leaving terms promising a period
 * the invoice does not have.
 */
export function withDueTerm(
  value: InvoiceFormValue,
  term: DueTerm,
  written: string | null = null,
): InvoiceFormValue {
  if (isNetDueTerm(term)) {
    const outcome = netDueDateFor(value.issueDate, term);
    if (outcome.kind === 'unreachable') return withDueTerm(value, 'none', written);
    return {
      ...value,
      dueTerm: term,
      dueDate: dueDateOf(outcome),
      terms: prefilledTerms(value.terms, term, written),
    };
  }

  return {
    ...value,
    dueTerm: term,
    dueDate: term === 'none' ? '' : value.dueDate,
    terms: prefilledTerms(value.terms, term, written),
  };
}

/**
 * The form a new invoice opens with, dated by its issue date.
 *
 * The default term is carried through `withIssueDate` rather than written into
 * the constant, because a form showing Net 14 over an empty due date would
 * raise an invoice that never goes overdue. The terms field is left alone: this
 * form writes a label only for a choice made in the session that makes it, and
 * a default nobody has touched is not one.
 */
export function newInvoiceForm(issueDate: string): InvoiceFormValue {
  return withIssueDate(EMPTY_INVOICE_FORM, issueDate);
}

/** The rows that will actually be sent: everything nobody left blank. */
export function invoiceFormItems(value: InvoiceFormValue): LineItemValue[] {
  return value.items.filter((item) => !isBlankLineItem(item));
}

/**
 * What the form refuses before the server sees it.
 *
 * The line rules are `validate_items`': at least one line, finite figures, and
 * a finite total above zero — checked after the arithmetic, because
 * `1e308 * 1e308` is infinity and serde renders a non-finite float as null
 * against a number. The date rule is the API's stricter one, not
 * `validate_date`'s: `2026-4-1` is a 400 over HTTP.
 */
export function validateInvoiceForm(value: InvoiceFormValue): InvoiceFormErrors {
  const errors: InvoiceFormErrors = {};

  if (value.clientId.trim() === '') errors.clientId = 'Choose a client';

  if (value.issueDate.trim() === '') errors.issueDate = 'Issue date is required';
  else if (!DATE_PATTERN.test(value.issueDate.trim())) {
    errors.issueDate = 'Issue date must be YYYY-MM-DD';
  }

  if (value.dueDate.trim() !== '' && !DATE_PATTERN.test(value.dueDate.trim())) {
    errors.dueDate = 'Due date must be YYYY-MM-DD';
  }

  if (!CURRENCY_PATTERN.test(value.currency.trim())) {
    errors.currency = 'Currency must be a three-letter code';
  }

  const rows = invoiceFormItems(value);
  if (rows.length === 0) {
    errors.items = 'An invoice needs at least one line item.';
    return errors;
  }

  const itemErrors: LineItemErrors[] = value.items.map((item) => {
    if (isBlankLineItem(item)) return {};
    const row: LineItemErrors = {};
    if (item.description.trim() === '') row.description = 'Description is required';

    const quantity = parseLineNumber(item.quantity);
    if (quantity === null) row.quantity = 'Quantity must be a number';

    const unit = parseLineNumber(item.unitAmount);
    if (unit === null) row.unitAmount = 'Unit amount must be a number';
    else if (quantity !== null && !Number.isFinite(quantity * unit)) {
      row.unitAmount = 'That line is too large to record';
    }
    return row;
  });

  if (itemErrors.some((row) => Object.keys(row).length > 0)) {
    errors.itemErrors = itemErrors;
    return errors;
  }

  const total = lineItemsSubtotal(rows);
  if (!Number.isFinite(total) || total <= 0) {
    errors.items = 'An invoice must total more than zero.';
  }

  return errors;
}

/**
 * The invoice editor's field group.
 *
 * Rendered as a full view rather than in a dialog, which is the one place the
 * invoicing screens depart from the manager pattern: `wc-manager-dialog` fits
 * a rule's six fields, and an invoice with eight line items inside a dialog is
 * a scrolling box inside a scrolling page. The *client* form stays a dialog,
 * because four fields is exactly what that pattern is for.
 */
@customElement('wc-invoice-form')
export class WcInvoiceForm extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: block;
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
      }

      .fields {
        display: grid;
        gap: var(--wa-space-m, 12px);
      }

      .row {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(11rem, 1fr));
        gap: var(--wa-space-m, 12px);
      }

      .due {
        display: grid;
        gap: var(--wa-space-2xs, 4px);
        align-content: start;
      }

      .due .error,
      .due .hint {
        margin: 0;
      }

      .error {
        margin: var(--wa-space-2xs, 4px) 0 0;
        color: var(--wa-color-danger, #b3261e);
        font-size: var(--wa-font-size-s, 13px);
      }

      .hint {
        margin: var(--wa-space-2xs, 4px) 0 0;
        color: var(--wa-color-muted);
        font-size: var(--wa-font-size-s, 13px);
      }

      .stacked {
        display: grid;
        gap: var(--wa-space-2xs, 4px);
      }

      .label {
        font-size: var(--wa-font-size-s, 13px);
        font-weight: var(--wa-font-weight-medium, 500);
      }

      textarea {
        font: inherit;
        width: 100%;
        box-sizing: border-box;
        padding: var(--wa-space-xs, 6px) var(--wa-space-s, 8px);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-m, 8px);
        background: var(--wa-color-surface);
        color: inherit;
        resize: vertical;
      }

      textarea:focus-visible {
        outline: 2px solid var(--wa-color-focus);
        outline-offset: 1px;
      }

      h3 {
        margin: var(--wa-space-s, 8px) 0 0;
        font-size: var(--wa-font-size-s, 13px);
        font-weight: var(--wa-font-weight-medium, 500);
        text-transform: uppercase;
        letter-spacing: 0.04em;
        color: var(--wa-color-muted);
      }
    `,
  ];

  @property({ type: String, reflect: true })
  mode: WcInvoiceFormMode = 'create';

  @property({ attribute: false })
  value: InvoiceFormValue = EMPTY_INVOICE_FORM;

  @property({ attribute: false })
  errors: InvoiceFormErrors = {};

  @property({ attribute: false })
  clients: InvoiceClientOption[] = [];

  @property({ type: Boolean })
  disabled = false;

  /**
   * The terms text this form wrote, so it can tell its own prefill from a
   * `Net 30` that came off the invoice or out of somebody's keyboard. Only the
   * former is ever rewritten.
   */
  private writtenTerms: string | null = null;

  /** The last value emitted, which is how a value from elsewhere is recognised. */
  private emitted: InvoiceFormValue | null = null;

  willUpdate(changed: PropertyValues<this>): void {
    // A value the form did not produce is a different invoice, or the same one
    // reloaded: nothing in it is this form's prefill.
    if (changed.has('value') && this.value !== this.emitted) this.writtenTerms = null;
  }

  private emit(next: Partial<InvoiceFormValue>): void {
    this.emitValue({ ...this.value, ...next });
  }

  private emitValue(value: InvoiceFormValue): void {
    this.emitted = value;
    this.dispatchEvent(
      new CustomEvent<NcInvoiceFormChangeDetail>('nc-invoice-form-change', {
        detail: { value },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handleField(field: keyof InvoiceFormValue) {
    return (event: Event) => {
      const input = event.target as HTMLInputElement;
      this.emit({ [field]: input.value } as Partial<InvoiceFormValue>);
    };
  }

  private handleIssueDate = (event: Event): void => {
    const input = event.target as HTMLInputElement;
    this.emitValue(withIssueDate(this.value, input.value));
  };

  /** An option outside the set is ignored rather than stored as a term. */
  private handleDueTerm = (event: Event): void => {
    const select = event.target as HTMLSelectElement;
    if (!isDueTerm(select.value)) return;

    const next = withDueTerm(this.value, select.value, this.writtenTerms);
    if (next.terms !== this.value.terms) this.writtenTerms = next.terms;
    this.emitValue(next);
  };

  /** Terms edited by hand are the operator's, whatever they now say. */
  private handleTerms = (event: Event): void => {
    this.writtenTerms = null;
    this.emit({ terms: (event.target as HTMLTextAreaElement).value });
  };

  private handleItems = (event: Event): void => {
    const detail = (event as CustomEvent<NcLineItemsChangeDetail>).detail;
    this.emit({ items: detail.items });
  };

  private get chosenClient(): InvoiceClientOption | undefined {
    return this.clients.find((client) => String(client.id) === this.value.clientId);
  }

  /**
   * The due date as terms rather than a calendar.
   *
   * The common case is a net period counted from the issue date, so the
   * control offers those and computes the date; the picker only appears for a
   * date that is nobody's net period.
   */
  private renderDue() {
    const term = this.value.dueTerm;

    return html`
      <div class="due">
        <wa-select
          data-due-term
          label="Due date"
          value=${term}
          ?disabled=${this.disabled}
          @change=${this.handleDueTerm}
        >
          ${DUE_TERM_VALUES.map(
            (option) =>
              html`<wa-option value=${option}>${DUE_TERM_LABELS[option]}</wa-option>`,
          )}
        </wa-select>

        ${term === 'custom'
          ? html`<wa-input
              data-due
              type="date"
              label="Date"
              placeholder="YYYY-MM-DD"
              autocomplete="off"
              spellcheck="false"
              value=${this.value.dueDate}
              ?disabled=${this.disabled}
              @input=${this.handleField('dueDate')}
            ></wa-input>`
          : nothing}
        ${this.errors.dueDate
          ? html`<p class="error" role="alert">${this.errors.dueDate}</p>`
          : this.dueHint()}
      </div>
    `;
  }

  /**
   * What the due-date control says under itself.
   *
   * A preset that has no date says which of the three reasons it is, because
   * "set an issue date" is wrong advice for an issue date that is already set
   * and simply unreadable.
   */
  private dueHint() {
    const term = this.value.dueTerm;

    if (isNetDueTerm(term)) {
      const days = NET_TERM_DAYS[term];
      const outcome = netDueDateFor(this.value.issueDate, term);
      const message =
        outcome.kind === 'ok'
          ? `Due ${outcome.dueDate}.`
          : outcome.kind === 'pending'
            ? 'Set an issue date.'
            : outcome.kind === 'unreadable'
              ? 'The issue date must be set first.'
              : `${days} days after that issue date is past the last date an invoice can carry.`;
      return html`<p class="hint" data-due-hint>${message}</p>`;
    }

    return this.value.dueDate === ''
      ? html`<p class="hint" data-due-hint>Never goes overdue.</p>`
      : nothing;
  }

  render() {
    const client = this.chosenClient;

    return html`
      <div class="fields">
        <div>
          <wa-select
            data-client
            label="Client"
            value=${this.value.clientId}
            ?disabled=${this.disabled || this.mode === 'edit'}
            @change=${this.handleField('clientId')}
          >
            <wa-option value="">Choose a client…</wa-option>
            ${this.clients.map(
              (option) => html`<wa-option value=${String(option.id)}>${option.name}</wa-option>`,
            )}
          </wa-select>
          ${this.errors.clientId
            ? html`<p class="error" role="alert">${this.errors.clientId}</p>`
            : this.mode === 'edit'
              ? html`<p class="hint">
                  An invoice stays with the client it was raised for.
                </p>`
              : client && client.email === null
                ? html`<p class="hint" data-no-email>
                    ${client.name} has no email address, so this invoice cannot be sent
                    until one is added.
                  </p>`
                : nothing}
        </div>

        <div class="row">
          <div>
            <wa-input
              data-issue
              type="date"
              label="Issue date"
              placeholder="YYYY-MM-DD"
              autocomplete="off"
              spellcheck="false"
              value=${this.value.issueDate}
              ?disabled=${this.disabled}
              @input=${this.handleIssueDate}
            ></wa-input>
            ${this.errors.issueDate
              ? html`<p class="error" role="alert">${this.errors.issueDate}</p>`
              : nothing}
          </div>
          ${this.renderDue()}
          <div>
            <wa-input
              data-currency
              label="Currency"
              autocomplete="off"
              spellcheck="false"
              maxlength="3"
              value=${this.value.currency}
              ?disabled=${this.disabled}
              @input=${this.handleField('currency')}
            ></wa-input>
            ${this.errors.currency
              ? html`<p class="error" role="alert">${this.errors.currency}</p>`
              : nothing}
          </div>
        </div>

        <h3>Line items</h3>
        <wc-line-items
          data-items
          caption="Line items"
          caption-hidden
          .items=${this.value.items}
          .errors=${this.errors.itemErrors ?? []}
          list-error=${this.errors.items ?? ''}
          ?disabled=${this.disabled}
          @nc-line-items-change=${this.handleItems}
        ></wc-line-items>

        <label class="stacked">
          <span class="label">Notes</span>
          <textarea
            data-notes
            rows="2"
            .value=${this.value.notes}
            ?disabled=${this.disabled}
            @input=${this.handleField('notes')}
          ></textarea>
        </label>

        <label class="stacked">
          <span class="label">Terms</span>
          <textarea
            data-terms
            rows="2"
            .value=${this.value.terms}
            ?disabled=${this.disabled}
            @input=${this.handleTerms}
          ></textarea>
        </label>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-invoice-form': WcInvoiceForm;
  }
  interface HTMLElementEventMap {
    'nc-invoice-form-change': CustomEvent<NcInvoiceFormChangeDetail>;
  }
}
