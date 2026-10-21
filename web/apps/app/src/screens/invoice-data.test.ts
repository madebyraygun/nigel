import { describe, it, expect } from 'vitest';
import { EMPTY_INVOICE_FORM, type InvoiceFormValue } from '@nigel/ui';

import {
  activeStatusFilter,
  clientFormFrom,
  clientPatch,
  daysBetween,
  detailActions,
  detailBalance,
  detailLineItems,
  dueNote,
  rowsWithStatus,
  statusChips,
  invoiceFormFrom,
  invoiceListParams,
  invoicePatch,
  invoiceTableRows,
  newClientRequest,
  newInvoiceRequest,
  payRequest,
  sendStepViews,
  today,
  voidConfirmationMessage,
  deleteConfirmationMessage,
} from './invoice-data.js';
import type {
  Client,
  ClientContact,
  InvoiceDetail,
  InvoiceListRow,
} from '../api/types.js';

const CLIENT: Client = {
  id: 1,
  name: 'Acme Co',
  email: 'ap@acme.test',
  billingAddress: '1 Main St',
  notes: null,
  archivedAt: null,
};

const DETAIL: InvoiceDetail = {
  id: 3,
  number: 1250,
  clientId: 1,
  status: 'draft',
  currency: 'USD',
  issueDate: '2026-02-20',
  dueDate: '2026-03-20',
  subtotal: 3200,
  tax: 0,
  total: 3200,
  notes: null,
  terms: 'Net 30',
  stripePaymentLinkId: null,
  stripePaymentLinkUrl: null,
  publishedAt: null,
  voidedAt: null,
  client: CLIENT,
  items: [
    {
      id: 1,
      invoiceId: 3,
      description: 'Consulting - February',
      quantity: 16,
      unitAmount: 175,
      lineTotal: 2800,
      position: 0,
    },
    {
      id: 2,
      invoiceId: 3,
      description: 'Hosting',
      quantity: 1,
      unitAmount: 400,
      lineTotal: 400,
      position: 1,
    },
  ],
  payments: [],
  paid: 0,
  balance: 3200,
  publicUrl: null,
  canEdit: true,
  canSend: true,
  canVoid: true,
  canPay: true,
  canDelete: false,
  deleteBlocked: null,
};

const FORM: InvoiceFormValue = invoiceFormFrom(DETAIL);

describe('invoiceListParams', () => {
  it('omits status when the filter is "all"', () => {
    // `all` is not one of the server's words: a status it does not know is a
    // 400 naming the legal set, not an ignored parameter.
    expect(invoiceListParams(new URLSearchParams('status=all'))).toEqual({});
    expect(invoiceListParams(new URLSearchParams())).toEqual({});
  });

  it('never sends a status, because the chips filter the whole list here', () => {
    expect(invoiceListParams(new URLSearchParams('status=paid'))).toEqual({});
    expect(invoiceListParams(new URLSearchParams('status=paid&clientId=3'))).toEqual({
      clientId: 3,
    });
  });

  it('carries clientId only when one is chosen', () => {
    expect(invoiceListParams(new URLSearchParams('clientId=3'))).toEqual({ clientId: 3 });
    expect(invoiceListParams(new URLSearchParams('clientId=nope'))).toEqual({});
    expect(invoiceListParams(new URLSearchParams('clientId=0'))).toEqual({});
  });

  it('reports which chip is on for a route that names none', () => {
    expect(activeStatusFilter(new URLSearchParams())).toBe('all');
    expect(activeStatusFilter(new URLSearchParams('status=paid'))).toBe('paid');
  });

  it('drops a status the list does not offer rather than sending it', () => {
    // The server answers 400 for a status it does not know, and the screen's
    // error state has no filter chips in it — so an unfiltered list beats a
    // dead end for a stale or hand-typed hash.
    expect(invoiceListParams(new URLSearchParams('status=pending'))).toEqual({});
    expect(invoiceListParams(new URLSearchParams('status=Open'))).toEqual({});
  });

  it('highlights the chip that matches what was actually requested', () => {
    expect(activeStatusFilter(new URLSearchParams('status=pending'))).toBe('all');
    expect(activeStatusFilter(new URLSearchParams('status=Open'))).toBe('all');
  });
});

describe('today', () => {
  it('reads the local day, not the UTC one', () => {
    // A user in UTC-5 filing a payment at 20:00 means their date, not
    // tomorrow's — the same reasoning `indexOfToday` keeps in the register.
    expect(today(new Date(2026, 2, 5, 20, 30))).toBe('2026-03-05');
  });
});

describe('newInvoiceRequest', () => {
  it('builds the request the create route takes', () => {
    expect(newInvoiceRequest(FORM)).toEqual({
      clientId: 1,
      issueDate: '2026-02-20',
      dueDate: '2026-03-20',
      currency: 'USD',
      items: [
        { description: 'Consulting - February', quantity: 16, unitAmount: 175 },
        { description: 'Hosting', quantity: 1, unitAmount: 400 },
      ],
      terms: 'Net 30',
    });
  });

  it('drops empty trailing rows', () => {
    const request = newInvoiceRequest({
      ...FORM,
      items: [...FORM.items, { description: '', quantity: '1', unitAmount: '' }],
    });
    expect(request?.items).toHaveLength(2);
  });

  it('omits an empty due date rather than sending null on a create', () => {
    expect(newInvoiceRequest({ ...FORM, dueDate: '' })).not.toHaveProperty('dueDate');
  });

  it('refuses a total of zero before the request is made', () => {
    expect(
      newInvoiceRequest({
        ...FORM,
        items: [{ description: 'Free', quantity: '0', unitAmount: '150' }],
      }),
    ).toBeNull();
  });

  it('refuses a form with no client or a malformed date', () => {
    expect(newInvoiceRequest({ ...FORM, clientId: '' })).toBeNull();
    expect(newInvoiceRequest({ ...FORM, issueDate: '2026-2-20' })).toBeNull();
  });

  it('upper-cases the currency the way validate_currency does', () => {
    expect(newInvoiceRequest({ ...FORM, currency: 'eur' })?.currency).toBe('EUR');
  });
});

describe('invoiceFormFrom', () => {
  it('round-trips an invoice into the editor and back with no patch', () => {
    expect(invoicePatch(DETAIL, invoiceFormFrom(DETAIL))).toEqual({});
  });

  it('renders an absent due date, note and term as empty fields', () => {
    const form = invoiceFormFrom({ ...DETAIL, dueDate: null, terms: null });
    expect(form.dueDate).toBe('');
    expect(form.terms).toBe('');
  });

  it('starts from the empty form, so a new field cannot arrive undefined', () => {
    expect(Object.keys(invoiceFormFrom(DETAIL)).sort()).toEqual(
      Object.keys(EMPTY_INVOICE_FORM).sort(),
    );
  });
});

describe('invoicePatch', () => {
  it('sends only changed fields — an all-absent PATCH is a 400', () => {
    expect(invoicePatch(DETAIL, { ...FORM, issueDate: '2026-02-21' })).toEqual({
      issueDate: '2026-02-21',
    });
  });

  it('sends dueDate: null to clear it, and omits it when unchanged', () => {
    expect(invoicePatch(DETAIL, { ...FORM, dueDate: '' })).toEqual({ dueDate: null });
    expect(invoicePatch(DETAIL, FORM)).not.toHaveProperty('dueDate');
  });

  it('clears notes and terms with null too', () => {
    expect(invoicePatch(DETAIL, { ...FORM, terms: '' })).toEqual({ terms: null });
    expect(invoicePatch(DETAIL, { ...FORM, notes: 'Thanks' })).toEqual({
      notes: 'Thanks',
    });
  });

  it('sends the whole items array when any row changed', () => {
    const patch = invoicePatch(DETAIL, {
      ...FORM,
      items: [FORM.items[0], { ...FORM.items[1], unitAmount: '450' }],
    });
    expect(patch.items).toEqual([
      { description: 'Consulting - February', quantity: 16, unitAmount: 175 },
      { description: 'Hosting', quantity: 1, unitAmount: 450 },
    ]);
  });

  it('sends the whole array when a row is removed or reordered', () => {
    expect(invoicePatch(DETAIL, { ...FORM, items: [FORM.items[0]] }).items).toHaveLength(1);
    expect(
      invoicePatch(DETAIL, { ...FORM, items: [FORM.items[1], FORM.items[0]] }).items?.[0]
        .description,
    ).toBe('Hosting');
  });

  it('does not send items when only their blank neighbours changed', () => {
    const patch = invoicePatch(DETAIL, {
      ...FORM,
      items: [...FORM.items, { description: '', quantity: '1', unitAmount: '' }],
    });
    expect(patch).not.toHaveProperty('items');
  });
});

describe('payRequest', () => {
  it('omits the amount when the field is empty, which means the whole balance', () => {
    expect(payRequest({ amount: '', date: '2026-03-15', method: 'ach' })).toEqual({
      date: '2026-03-15',
      method: 'ach',
    });
  });

  it('sends a typed amount with its commas stripped', () => {
    expect(
      payRequest({ amount: '1,200.50', date: '2026-03-15', method: 'direct_deposit' }),
    ).toEqual({ amount: 1200.5, date: '2026-03-15', method: 'direct_deposit' });
  });

  it('omits a method the CHECK constraint would refuse rather than sending it', () => {
    // The route would answer 400 naming the legal set; letting the server's
    // default stand is the honest thing for a value no control can produce.
    expect(payRequest({ amount: '', date: '2026-03-15', method: 'bitcoin' })).toEqual({
      date: '2026-03-15',
    });
  });
});

describe('client requests', () => {
  const CONTACTS: ClientContact[] = [
    {
      id: 1,
      clientId: 1,
      email: 'ap@acme.test',
      name: 'Ada Payne',
      title: null,
      isBilling: true,
      position: 0,
    },
    {
      id: 2,
      clientId: 1,
      email: 'dana@acme.test',
      name: null,
      title: null,
      isBilling: false,
      position: 1,
    },
  ];

  it('sends an empty optional field as null, not as an empty string', () => {
    expect(
      newClientRequest({
        name: ' Acme Co ',
        contacts: [],
        billingIndex: 0,
        billingAddress: '',
        notes: '',
      }),
    ).toEqual({ name: 'Acme Co', billingAddress: null, notes: null });
  });

  it('sends the contact list whole, with exactly one billing recipient', () => {
    // `email` is never sent beside `contacts` — the route refuses both.
    expect(
      newClientRequest({
        name: 'Acme Co',
        contacts: [
          { email: ' ap@acme.test ', name: 'Ada', title: '' },
          { email: 'dana@acme.test', name: '', title: 'Design' },
        ],
        billingIndex: 1,
        billingAddress: '',
        notes: '',
      }),
    ).toEqual({
      name: 'Acme Co',
      billingAddress: null,
      notes: null,
      contacts: [
        { email: 'ap@acme.test', name: 'Ada', title: null, isBilling: false },
        { email: 'dana@acme.test', name: null, title: 'Design', isBilling: true },
      ],
    });
  });

  it('drops a row nobody filled in', () => {
    const request = newClientRequest({
      name: 'Acme Co',
      contacts: [
        { email: 'ap@acme.test', name: '', title: '' },
        { email: '   ', name: '', title: '' },
      ],
      billingIndex: 0,
      billingAddress: '',
      notes: '',
    });
    expect(request.contacts).toHaveLength(1);
  });

  it('patches only what moved, and clears with null', () => {
    const form = clientFormFrom(CLIENT, CONTACTS);
    expect(clientPatch(CLIENT, form, CONTACTS)).toEqual({});
    expect(clientPatch(CLIENT, { ...form, name: 'Acme Corp' }, CONTACTS)).toEqual({
      name: 'Acme Corp',
    });
  });

  it('sends contacts only when the list actually changed', () => {
    const form = clientFormFrom(CLIENT, CONTACTS);

    // A reorder is a change, because position is what the cc order is.
    const reordered = {
      ...form,
      contacts: [form.contacts[1], form.contacts[0]],
      billingIndex: 1,
    };
    expect(clientPatch(CLIENT, reordered, CONTACTS).contacts).toHaveLength(2);
    expect(clientPatch(CLIENT, reordered, CONTACTS).contacts?.[0].email).toBe(
      'dana@acme.test',
    );

    // Moving the billing flag is a change even when the rows do not move.
    expect(
      clientPatch(CLIENT, { ...form, billingIndex: 1 }, CONTACTS).contacts?.[1].isBilling,
    ).toBe(true);

    // Clearing the list is how a client loses every address.
    expect(clientPatch(CLIENT, { ...form, contacts: [] }, CONTACTS).contacts).toEqual([]);
  });
});

describe('invoiceTableRows', () => {
  const rows: InvoiceListRow[] = [
    {
      id: 1,
      number: 1250,
      status: 'partial',
      clientId: 1,
      clientName: 'Acme Co',
      issueDate: '2026-02-20',
      dueDate: '2026-03-20',
      currency: 'USD',
      total: 3200,
      paid: 2000,
      balance: 1200,
    },
    {
      id: 2,
      number: 1247,
      status: 'void',
      clientId: 2,
      clientName: null,
      issueDate: '2026-01-05',
      dueDate: null,
      currency: 'USD',
      total: 500,
      paid: 0,
      balance: 500,
    },
  ];

  it('drops a void invoice’s balance so the table shows an em dash', () => {
    const mapped = invoiceTableRows(rows);
    expect(mapped[0].balance).toBe(1200);
    expect(mapped[1].balance).toBeNull();
  });

  it('links each row at its own number', () => {
    expect(invoiceTableRows(rows)[0].href).toBe('#/invoices/1250');
  });

  it('keeps a null client name rather than inventing one', () => {
    expect(invoiceTableRows(rows)[1].clientName).toBeNull();
  });
});

describe('detailLineItems', () => {
  it('prints quantities to two decimals, as format_invoice_show does', () => {
    expect(detailLineItems(DETAIL).map((item) => item.quantity)).toEqual(['16.00', '1.00']);
  });

  it('carries the row total the response states, not the rounded product', () => {
    // 1.755 h at $200.00 is billed at $351.00. Rounding the quantity for the
    // column and re-multiplying it would print $350.00.
    const detail: InvoiceDetail = {
      ...DETAIL,
      items: [
        {
          ...DETAIL.items[0],
          quantity: 1.755,
          unitAmount: 200,
          lineTotal: 351,
        },
      ],
    };
    const [row] = detailLineItems(detail);
    // The column shows 1.75, whose product with the unit is 350 — one cent
    // short of what the row was billed at, which is the whole point.
    expect(row.quantity).toBe('1.75');
    expect(Number(row.quantity) * detail.items[0].unitAmount).toBe(350);
    expect(row.amount).toBe('351');
  });
});

describe('detailBalance', () => {
  it('reports a void invoice as owing nothing, matching the list', () => {
    expect(detailBalance({ ...DETAIL, status: 'void', total: 3200, balance: 3200 })).toBe(
      null,
    );
  });

  it('reports a live invoice at its balance', () => {
    expect(detailBalance({ ...DETAIL, status: 'sent', balance: 1200 })).toBe(1200);
  });
});

describe('voidConfirmationMessage', () => {
  it('promises nothing about a page for an invoice that was never published', () => {
    const message = voidConfirmationMessage({ ...DETAIL, publishedAt: null });
    expect(message).toContain('cannot be edited, sent or paid');
    expect(message).not.toContain('published page');
  });

  it('names what void does to a published invoice, conditionally on the config', () => {
    const message = voidConfirmationMessage({ ...DETAIL, publishedAt: '2026-02-20' });
    expect(message).toContain('replaces the published page');
    expect(message).toContain('deactivates its Stripe payment link');
    // What is actually configured is the server's to report, afterwards.
    expect(message).toContain('wherever each of those is configured');
  });
});

describe('deleteConfirmationMessage', () => {
  it('names the invoice, its line items, and that the number will stay a gap', () => {
    const message = deleteConfirmationMessage(DETAIL);
    expect(message).toContain('Invoice #1250');
    expect(message).toContain(`${DETAIL.items.length} line items`);
    expect(message).toContain('numbers are not reused');
    expect(message).toContain('#1250 will stay a gap');
  });

  it('counts one line item in the singular', () => {
    const message = deleteConfirmationMessage({ ...DETAIL, items: [DETAIL.items[0]] });
    expect(message).toContain('1 line item will be removed');
  });
});

describe('sendStepViews', () => {
  it('shows every step, not only the ones that ran', () => {
    const views = sendStepViews({ completed: ['config', 'load'], running: 'precheck' });
    expect(views).toHaveLength(8);
    expect(views.map((view) => view.state)).toEqual([
      'ok',
      'ok',
      'running',
      'pending',
      'pending',
      'pending',
      'pending',
      'pending',
    ]);
  });

  it('infers config from the step list the server actually sends', () => {
    // `config` belongs to the caller — the settings are resolved before the
    // orchestration is reachable — so the server never lists it as completed.
    // Reading the wire literally showed it as never-run on a send that worked.
    const views = sendStepViews({
      completed: [
        'load',
        'precheck',
        'payment_link',
        'render',
        'publish',
        'email',
        'record',
      ],
    });
    expect(views.every((view) => view.state === 'ok')).toBe(true);
  });

  it('still reports config as the failure when it is the one that failed', () => {
    const views = sendStepViews({ completed: [], failed: 'config' });
    expect(views.find((view) => view.step === 'config')?.state).toBe('failed');
  });

  it('leaves config pending before the request goes out', () => {
    const views = sendStepViews({ running: 'config' });
    expect(views.find((view) => view.step === 'config')?.state).toBe('running');
    expect(views.filter((view) => view.state === 'pending')).toHaveLength(7);
  });

  it('marks a reused payment link as reused rather than as fresh work', () => {
    const views = sendStepViews({
      completed: [
        { step: 'config', outcome: 'ok' },
        { step: 'payment_link', outcome: 'reused' },
      ],
    });
    expect(views.find((view) => view.step === 'payment_link')?.state).toBe('reused');
  });

  it('marks the failed step and leaves everything after it pending', () => {
    const views = sendStepViews({
      completed: ['config', 'load', 'precheck', 'payment_link', 'render'],
      failed: 'publish',
    });
    expect(views.find((view) => view.step === 'publish')?.state).toBe('failed');
    expect(views.find((view) => view.step === 'email')?.state).toBe('pending');
  });

  it('gives every step a label in our own words', () => {
    expect(sendStepViews({}).every((view) => view.label.length > 0)).toBe(true);
  });
});

function row(number: number, status: string): InvoiceListRow {
  return {
    id: number,
    number,
    status,
    clientId: 1,
    clientName: 'Acme Co',
    issueDate: '2026-02-20',
    dueDate: '2026-03-20',
    currency: 'USD',
    total: 100,
    paid: 0,
    balance: 100,
  };
}

describe('status chips', () => {
  const rows = [row(7, 'paid'), row(6, 'paid'), row(5, 'overdue'), row(4, 'void')];

  it('filters the list it was given, and treats an unknown status as all', () => {
    expect(rowsWithStatus(rows, 'paid').map((r) => r.number)).toEqual([7, 6]);
    expect(rowsWithStatus(rows, 'all')).toHaveLength(4);
    expect(rowsWithStatus(rows, 'open')).toHaveLength(4);
  });

  it('counts every status from the whole list, in the handoff’s order', () => {
    const chips = statusChips(rows, (status) => (status ? `#/invoices?status=${status}` : '#/invoices'));
    expect(chips.map((chip) => [chip.label, chip.count])).toEqual([
      ['All', 4],
      ['Draft', 0],
      ['Sent', 0],
      ['Partial', 0],
      ['Overdue', 1],
      ['Paid', 2],
      ['Void', 1],
    ]);
    expect(chips[0].href).toBe('#/invoices');
    expect(chips[5].href).toBe('#/invoices?status=paid');
  });
});

describe('dueNote', () => {
  it('counts the days to a due date', () => {
    expect(daysBetween('2026-10-19', '2026-10-20')).toBe(1);
    expect(daysBetween('2026-10-19', '2026-09-30')).toBe(-19);
    expect(dueNote({ status: 'sent', dueDate: '2026-10-19' }, '2026-10-19')).toBe('Due today');
    expect(dueNote({ status: 'partial', dueDate: '2026-10-20' }, '2026-10-19')).toBe(
      'Due tomorrow',
    );
    expect(dueNote({ status: 'sent', dueDate: '2026-11-05' }, '2026-10-19')).toBe(
      'Due in 17 days',
    );
  });

  it('says how late an overdue invoice is', () => {
    expect(dueNote({ status: 'overdue', dueDate: '2026-09-30' }, '2026-10-19')).toBe(
      '19 days overdue',
    );
    expect(dueNote({ status: 'overdue', dueDate: '2026-10-18' }, '2026-10-19')).toBe(
      '1 day overdue',
    );
  });

  it('says nothing for an invoice nobody is waiting on', () => {
    for (const status of ['draft', 'paid', 'void']) {
      expect(dueNote({ status, dueDate: '2026-09-30' }, '2026-10-19')).toBe('');
    }
    expect(dueNote({ status: 'sent', dueDate: null }, '2026-10-19')).toBe('');
  });
});

describe('detailActions', () => {
  const ready = { sendBlock: '', busy: false };
  const by = (actions: ReturnType<typeof detailActions>) =>
    Object.fromEntries(actions.map((action) => [action.key, action]));

  it('offers a draft everything but a payment, with Delete in the danger slot', () => {
    const actions = by(detailActions({ ...DETAIL, canDelete: true }, ready));
    expect(Object.keys(actions)).toEqual(['send', 'pay', 'edit', 'duplicate', 'delete']);
    expect(actions.send.label).toBe('Send…');
    expect(actions.send.disabled).toBe(false);
    expect(actions.pay.disabled).toBe(true);
    expect(actions.pay.reason).toBe('A payment can be recorded once the invoice is sent.');
    expect(actions.edit.disabled).toBe(false);
    expect(actions.delete.disabled).toBe(false);
  });

  it('turns Send into Resend once the invoice has gone out, and swaps Delete for Void', () => {
    const actions = by(
      detailActions(
        { ...DETAIL, status: 'sent', publishedAt: '2026-02-20', canEdit: false, canDelete: false },
        ready,
      ),
    );
    expect(actions.send.label).toBe('Resend…');
    expect(actions.pay.disabled).toBe(false);
    expect(actions.edit.reason).toContain('A sent invoice cannot be edited');
    expect(actions.delete).toBeUndefined();
    expect(actions.void.disabled).toBe(false);
  });

  it('requires the server’s flag and the handoff’s rule both', () => {
    // canPay is true on a draft server-side; the handoff only offers it once sent.
    expect(by(detailActions({ ...DETAIL, canPay: true }, ready)).pay.disabled).toBe(true);
    // And a sent invoice the server will not take a payment on stays off.
    expect(
      by(detailActions({ ...DETAIL, status: 'sent', canPay: false }, ready)).pay.disabled,
    ).toBe(true);
  });

  it('turns off Send and Record payment on a paid invoice and says why once', () => {
    const actions = by(
      detailActions(
        {
          ...DETAIL,
          status: 'paid',
          publishedAt: '2026-02-20',
          paid: 3200,
          balance: 0,
          canEdit: false,
          canVoid: false,
          canPay: false,
        },
        ready,
      ),
    );
    expect(actions.send.disabled).toBe(true);
    expect(actions.send.reason).toBe('This invoice is paid in full.');
    expect(actions.pay.reason).toBe('This invoice is paid in full.');
    expect(actions.edit.reason).toBe(
      'An invoice with a payment recorded cannot be edited or voided.',
    );
    expect(actions.void.reason).toBe(actions.edit.reason);
    expect(actions.duplicate.disabled).toBe(false);
  });

  it('explains a schedule-generated draft’s Delete and puts Void beside it', () => {
    const actions = by(
      detailActions(
        {
          ...DETAIL,
          canDelete: false,
          canVoid: true,
          deleteBlocked: { reason: 'from_schedule', message: 'Cannot delete: …' },
        },
        ready,
      ),
    );
    expect(actions.delete.disabled).toBe(true);
    expect(actions.delete.reason).toBe(
      'A recurring schedule generated this invoice and keeps it as the record of what that period billed. Void it instead.',
    );
    expect(actions.void.disabled).toBe(false);
  });

  it('names what the installation is missing before anything about the invoice', () => {
    const actions = by(
      detailActions(DETAIL, { sendBlock: 'Sending needs r2_bucket, which is not set.', busy: false }),
    );
    expect(actions.send.disabled).toBe(true);
    expect(actions.send.reason).toBe('Sending needs r2_bucket, which is not set.');
  });

  it('holds every action while a write is in flight, without inventing a reason', () => {
    const actions = detailActions(DETAIL, { sendBlock: '', busy: true });
    expect(actions.every((action) => action.disabled)).toBe(true);
    expect(actions.some((action) => action.reason)).toBe(false);
  });
});
