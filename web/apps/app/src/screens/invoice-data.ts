import {
  EMPTY_INVOICE_FORM,
  invoiceFormItems,
  parseLineNumber,
  validateInvoiceForm,
  type InvoiceFormValue,
  type LineItemValue,
  type SendStepState,
  type SendStepView,
} from '@nigel/ui';

import type {
  Client,
  ClientContact,
  ClientPatch,
  InvoiceDetail,
  InvoiceListParams,
  InvoiceListRow,
  InvoicePatch,
  NewClientRequest,
  NewContact,
  NewInvoiceRequest,
  NewLineItem,
  PayInvoiceRequest,
  PaymentMethod,
  SendStep,
  SendStepResult,
} from '../api/types.js';
import { PAYMENT_METHODS, SEND_STEPS } from '../api/types.js';
import type { ClientFormValue, PaymentFormValue } from '@nigel/ui';

/** The status filters the list offers, and what each one asks the server for. */
export const STATUS_FILTERS = [
  { value: 'all', label: 'All' },
  { value: 'draft', label: 'Draft' },
  { value: 'open', label: 'Open' },
  { value: 'paid', label: 'Paid' },
  { value: 'void', label: 'Void' },
] as const;

/**
 * The list request a route asks for.
 *
 * An absent filter is omitted rather than sent empty: the server rejects a
 * `status` it does not know instead of ignoring it, so `all` — which is not
 * one of its words — must never reach the query string. A status outside the
 * chips is dropped for the same reason, so a stale or hand-typed hash reads as
 * the unfiltered list rather than as the 400 the server would answer.
 */
export function invoiceListParams(params: URLSearchParams): InvoiceListParams {
  const request: InvoiceListParams = {};

  const status = params.get('status');
  if (status && status !== 'all' && isStatusFilter(status)) request.status = status;

  const clientId = Number(params.get('clientId'));
  if (Number.isInteger(clientId) && clientId > 0) request.clientId = clientId;

  return request;
}

/** Whether a route's `status` is one the list actually offers. */
export function isStatusFilter(value: string): boolean {
  return STATUS_FILTERS.some((filter) => filter.value === value);
}

/**
 * Which filter chip is on, for a route that names none.
 *
 * An unrecognised status highlights `all`, matching what `invoiceListParams`
 * asked the server for — the chips may not claim a filter that is not applied.
 */
export function activeStatusFilter(params: URLSearchParams): string {
  const status = params.get('status');
  return status !== null && isStatusFilter(status) ? status : 'all';
}

/** A `YYYY-MM-DD` for the local day, which is what the server calls today. */
export function today(now: Date = new Date()): string {
  const month = String(now.getMonth() + 1).padStart(2, '0');
  const day = String(now.getDate()).padStart(2, '0');
  return `${now.getFullYear()}-${month}-${day}`;
}

function toLineItems(rows: LineItemValue[]): NewLineItem[] {
  return rows.map((row) => ({
    description: row.description.trim(),
    quantity: parseLineNumber(row.quantity) ?? 0,
    unitAmount: parseLineNumber(row.unitAmount) ?? 0,
  }));
}

/**
 * The create request, or null when the form would not survive the round trip.
 *
 * Validation and construction are the same function on purpose: a request the
 * form has already decided is invalid should not be representable, and
 * `validate_items` refuses a total of zero at the far end anyway.
 */
export function newInvoiceRequest(value: InvoiceFormValue): NewInvoiceRequest | null {
  if (Object.keys(validateInvoiceForm(value)).length > 0) return null;

  const dueDate = value.dueDate.trim();
  const notes = value.notes.trim();
  const terms = value.terms.trim();

  return {
    clientId: Number(value.clientId),
    issueDate: value.issueDate.trim(),
    ...(dueDate === '' ? {} : { dueDate }),
    currency: value.currency.trim().toUpperCase(),
    items: toLineItems(invoiceFormItems(value)),
    ...(notes === '' ? {} : { notes }),
    ...(terms === '' ? {} : { terms }),
  };
}

/** The form as it should look when an existing invoice is opened for editing. */
export function invoiceFormFrom(detail: InvoiceDetail): InvoiceFormValue {
  return {
    ...EMPTY_INVOICE_FORM,
    clientId: String(detail.clientId),
    issueDate: detail.issueDate,
    dueDate: detail.dueDate ?? '',
    currency: detail.currency,
    notes: detail.notes ?? '',
    terms: detail.terms ?? '',
    items: detail.items.map((item) => ({
      description: item.description,
      quantity: String(item.quantity),
      unitAmount: String(item.unitAmount),
    })),
  };
}

function sameItems(detail: InvoiceDetail, rows: LineItemValue[]): boolean {
  const sent = toLineItems(rows);
  if (sent.length !== detail.items.length) return false;
  return sent.every((item, index) => {
    const current = detail.items[index];
    return (
      item.description === current.description &&
      item.quantity === current.quantity &&
      item.unitAmount === current.unitAmount
    );
  });
}

/**
 * Only what changed.
 *
 * An all-absent PATCH is a 400, so a save with nothing changed must not be
 * sent at all — the same rule the categories manager keeps. `dueDate: null`
 * clears the column and omitting it leaves it, which is `double_option` on the
 * other side of the wire; and `items` is a whole-list replacement, so it goes
 * in its entirety or not at all.
 */
export function invoicePatch(
  detail: InvoiceDetail,
  value: InvoiceFormValue,
): InvoicePatch {
  const patch: InvoicePatch = {};

  const issueDate = value.issueDate.trim();
  if (issueDate !== detail.issueDate) patch.issueDate = issueDate;

  const dueDate = value.dueDate.trim();
  const currentDue = detail.dueDate ?? '';
  if (dueDate !== currentDue) patch.dueDate = dueDate === '' ? null : dueDate;

  const currency = value.currency.trim().toUpperCase();
  if (currency !== detail.currency) patch.currency = currency;

  const notes = value.notes.trim();
  if (notes !== (detail.notes ?? '')) patch.notes = notes === '' ? null : notes;

  const terms = value.terms.trim();
  if (terms !== (detail.terms ?? '')) patch.terms = terms === '' ? null : terms;

  const rows = invoiceFormItems(value);
  if (!sameItems(detail, rows)) patch.items = toLineItems(rows);

  return patch;
}

/** The payment request. An empty amount means the whole outstanding balance. */
export function payRequest(value: PaymentFormValue): PayInvoiceRequest {
  const amount = value.amount.trim() === '' ? null : parseLineNumber(value.amount);
  const method = (PAYMENT_METHODS as readonly string[]).includes(value.method)
    ? (value.method as PaymentMethod)
    : undefined;

  return {
    date: value.date.trim(),
    ...(amount === null ? {} : { amount }),
    ...(method === undefined ? {} : { method }),
  };
}

/** Empty means "no value", which on the wire is null rather than "". */
function orNull(value: string): string | null {
  const trimmed = value.trim();
  return trimmed === '' ? null : trimmed;
}

/**
 * The form's rows as the server takes them: trimmed, blank rows dropped, and
 * exactly one `isBilling`.
 *
 * `email` is never sent alongside `contacts` — the route refuses both — so a
 * screen built on this form always speaks in whole lists.
 */
export function contactsRequest(value: ClientFormValue): NewContact[] {
  const rows = value.contacts
    .map((contact, index) => ({ contact, index }))
    .filter(({ contact }) => contact.email.trim() !== '');
  const billing = rows.find(({ index }) => index === value.billingIndex) ?? rows[0];

  return rows.map(({ contact, index }) => ({
    email: contact.email.trim(),
    name: orNull(contact.name),
    title: orNull(contact.title),
    isBilling: index === billing?.index,
  }));
}

export function newClientRequest(value: ClientFormValue): NewClientRequest {
  const contacts = contactsRequest(value);
  return {
    name: value.name.trim(),
    billingAddress: orNull(value.billingAddress),
    notes: orNull(value.notes),
    ...(contacts.length > 0 ? { contacts } : {}),
  };
}

/** Two contact lists that would write the same rows in the same order. */
function sameContacts(a: NewContact[], b: NewContact[]): boolean {
  if (a.length !== b.length) return false;
  return a.every((left, i) => {
    const right = b[i];
    return (
      left.email === right.email &&
      (left.name ?? null) === (right.name ?? null) &&
      (left.title ?? null) === (right.title ?? null) &&
      Boolean(left.isBilling) === Boolean(right.isBilling)
    );
  });
}

/** Only the fields that moved, for the same reason `invoicePatch` sends only those. */
export function clientPatch(
  current: Client,
  value: ClientFormValue,
  currentContacts: ClientContact[] = [],
): ClientPatch {
  const patch: ClientPatch = {};

  const name = value.name.trim();
  if (name !== current.name) patch.name = name;

  const contacts = contactsRequest(value);
  if (!sameContacts(contacts, contactsRequest(clientFormFrom(current, currentContacts)))) {
    patch.contacts = contacts;
  }

  const billingAddress = orNull(value.billingAddress);
  if (billingAddress !== current.billingAddress) patch.billingAddress = billingAddress;

  const notes = orNull(value.notes);
  if (notes !== current.notes) patch.notes = notes;

  return patch;
}

/**
 * The form for one client. Contacts come from the detail route, because the
 * list row is a bare `Client` and does not carry them.
 */
export function clientFormFrom(
  client: Client,
  contacts: ClientContact[] = [],
): ClientFormValue {
  const billingIndex = Math.max(
    contacts.findIndex((contact) => contact.isBilling),
    0,
  );
  return {
    name: client.name,
    contacts: contacts.map((contact) => ({
      email: contact.email,
      name: contact.name ?? '',
      title: contact.title ?? '',
    })),
    billingIndex,
    billingAddress: client.billingAddress ?? '',
    notes: client.notes ?? '',
  };
}

/** Our words for each step of a send, in execution order. */
export const SEND_STEP_LABELS: Record<SendStep, string> = {
  config: 'Reading the invoicing settings',
  load: 'Loading the invoice',
  precheck: 'Checking the invoice can be sent',
  payment_link: 'Creating the Stripe payment link',
  render: 'Rendering the invoice',
  publish: 'Publishing the invoice page',
  email: 'Emailing the client',
  record: 'Recording the send',
};

/**
 * The whole step list with a state on each, from whatever the server said.
 *
 * Every step is shown, not only the ones that ran: the trace is most useful
 * exactly when it stopped early, and a list that grows as it goes cannot show
 * what did not happen.
 *
 * `config` is inferred rather than reported. It belongs to the caller — the
 * settings are resolved and the three clients built before the orchestration is
 * reachable — so the server never lists it as completed and names it only as
 * the step that *failed*. Any other step having an outcome therefore means
 * config succeeded, and reading it off the wire literally would show it as
 * never-run on every send that worked.
 */
export function sendStepViews(options: {
  completed?: SendStepResult[] | SendStep[];
  running?: SendStep | null;
  failed?: SendStep | null;
}): SendStepView[] {
  const outcomes = new Map<string, SendStepState>();
  for (const entry of options.completed ?? []) {
    if (typeof entry === 'string') outcomes.set(entry, 'ok');
    else outcomes.set(entry.step, entry.outcome === 'reused' ? 'reused' : 'ok');
  }

  const reachedOrchestration = outcomes.size > 0 || (options.failed ?? null) !== null;
  if (reachedOrchestration && options.failed !== 'config' && !outcomes.has('config')) {
    outcomes.set('config', 'ok');
  }

  return SEND_STEPS.map((step) => ({
    step,
    label: SEND_STEP_LABELS[step],
    state:
      step === options.failed
        ? ('failed' as SendStepState)
        : step === options.running
          ? ('running' as SendStepState)
          : (outcomes.get(step) ?? ('pending' as SendStepState)),
  }));
}

/**
 * Outstanding as a reader should see it, or null for nothing outstanding ever.
 *
 * A void invoice owes nothing and never will, so it reports an em dash rather
 * than a figure: `$0.00` would read as settled, and its own total would report
 * a receivable the aging report has already excluded. One function so the list
 * and the detail view cannot answer this differently for the same invoice.
 */
export function outstandingOrNull(invoice: { status: string; balance: number }): number | null {
  return invoice.status === 'void' ? null : invoice.balance;
}

/**
 * What the void dialog says voiding will do, for this invoice.
 *
 * A published invoice is the one that has anything out there: voiding replaces
 * its page and deactivates its payment link wherever those are configured, and
 * whatever is not configured is reported afterwards rather than promised here.
 * A draft has nothing published and nothing to say about it.
 */
export function voidConfirmationMessage(detail: InvoiceDetail): string {
  const terminal = 'A void invoice cannot be edited, sent or paid.';
  if (detail.publishedAt === null) return terminal;
  return `${terminal} This invoice has been published: voiding it replaces the published page with a voided notice and deactivates its Stripe payment link, wherever each of those is configured.`;
}

/**
 * What the delete dialog says deleting will do, for this invoice.
 *
 * Two facts and no hedging, because the whole point of delete is that there is
 * nothing out there to take down: the row and its lines go, and the number does
 * not come back. The gap is deliberate — reissuing a number that may already
 * have been exported or quoted is the thing this avoids — so the dialog names
 * it rather than letting it be discovered later.
 */
export function deleteConfirmationMessage(detail: InvoiceDetail): string {
  const lines = detail.items.length;
  return `Invoice #${detail.number} and its ${lines} line item${
    lines === 1 ? '' : 's'
  } will be removed for good. Invoice numbers are not reused, so #${detail.number} will stay a gap in the sequence.`;
}

/** The detail view's outstanding figure. */
export function detailBalance(detail: InvoiceDetail): number | null {
  return outstandingOrNull(detail);
}

/** What the invoice table needs, from what the list route answers with. */
export function invoiceTableRows(rows: InvoiceListRow[]) {
  return rows.map((row) => ({
    number: row.number,
    status: row.status,
    clientName: row.clientName,
    total: row.total,
    balance: outstandingOrNull(row),
    dueDate: row.dueDate,
    currency: row.currency,
    href: `#/invoices?number=${row.number}`,
  }));
}

/** The line-item rows the read-only detail table shows, quantities and all. */
export function detailLineItems(detail: InvoiceDetail): LineItemValue[] {
  return detail.items.map((item) => ({
    description: item.description,
    // Two decimals, matching `format_invoice_show`'s quantity column.
    quantity: item.quantity.toFixed(2),
    unitAmount: String(item.unitAmount),
    // The row's own total, not the product of the rounded quantity above it —
    // 1.755 hours at $200.00 was billed at $351.00, and a table that recomputed
    // it would print $350.00 under a Total of $351.00.
    amount: String(item.lineTotal),
  }));
}

/**
 * The subject line the email will carry.
 *
 * Two lines of TypeScript mirroring `send.rs`'s own rule, and knowingly
 * duplicated: the alternative is a new field on a payload for one string, and
 * this boundary already carries `SEND_STEP_LABELS` the same way. `companyName`
 * is the same business name the server composes it from.
 */
export function emailSubject(number: number, companyName: string): string {
  const company = companyName.trim();
  return company ? `Invoice #${number} from ${company}` : `Invoice #${number}`;
}
