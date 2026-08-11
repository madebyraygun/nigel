# The invoice page and the invoice PDF, saying the same things

Task: TASK-78 (medium), stream 2 of epic TASK-86.

## Problem

A client gets two documents: the page at the published URL and the PDF attached
to the email. They are rendered from the same row and they disagree.

The page prints the client's **name** and nothing else about them — a billing
address entered against a client renders nowhere, which is how this surfaced
after backfilling addresses onto twenty clients. It prints a total with no
subtotal and no tax line. It never names the business sending it.

The PDF prints the business name, the client's name, both dates, subtotal and
tax (when tax is non-zero), the total, notes and terms. It omits the client's
address and email, and it offers no way to pay.

So the attachment and the page differ in both directions, and an invoice with
no company block and no client address is unusual for a real business document
— which every Nigel user gets by default rather than by choosing it.

## Where the code is today

**`render_html.rs` already expands all eighteen placeholders.** The task text
says the page "uses 11 of the 18 placeholders `render_html.rs` declares", which
is right about the *stock template* and wrong about the renderer:
`render_invoice_html` supplies a value for every key in `PLACEHOLDERS`,
including `CLIENT_EMAIL`, `CLIENT_ADDRESS`, `COMPANY`, `DUE_DATE`, `SUBTOTAL`,
`TAX` and `PAY_URL`, and there is a test
(`every_placeholder_in_the_vocabulary_expands`) that keeps it that way. The
seven unused keys are unused by `src/invoicing/templates/invoice.html` — which
is 17 lines long and prints:

```
Invoice #{{NUMBER}} / Billed to: {{CLIENT}} / Issued: {{ISSUE}}{{DUE}}
table({{ROWS}}) / Total: {{CURRENCY}} {{TOTAL}} / {{PAY}} / {{NOTES}} {{TERMS}}
Direct deposit … Contact {{CONTACT}} …
```

So most of this task is a template rewrite plus the handful of new *fragment*
placeholders that a language with no conditionals needs in order to omit a
block.

**`render_invoice_pdf` already takes the company.** The task's implementation
note says it "takes invoice, client and items but no company, so the metadata
`company_name` has to be plumbed in". TASK-68.3 already did that: the signature
is `render_invoice_pdf(invoice, client, items, company)`, the name is drawn
bold under the invoice number and repeated in the document Info title, and
`render.rs` threads it out of `Branding`. AC #3's company half is already true;
the address and email halves are not.

| Thing | Location |
|---|---|
| Placeholder vocabulary, validation | `src/invoicing/render_html.rs` — `PLACEHOLDERS`, `REQUIRED`, `validate_template`, `load_template` |
| The stock page | `src/invoicing/templates/invoice.html` |
| The HTML renderer | `render_html.rs` — `render_invoice_html(branding, invoice, client, items, pay)`, the `block()` closure, `esc`, single-pass `expand` |
| The PDF | `src/pdf.rs` — `render_invoice_pdf`, `money()` from `crate::fmt`, `table_row`/`table_row_wrapped`/`section_label` |
| The seam both go through | `src/invoicing/render.rs` — `render_invoice(conn, invoice, client, pay, branding)` |
| Payments | `src/invoicing/invoices.rs` — `paid_amount(conn, invoice_id)` |
| The documented vocabulary | `docs/invoicing.md` — "Placeholders" table, text vs fragment |

## Design

### Decision 1: omission is the renderer's job, through fragment placeholders

`expand` is single-pass and has no conditionals, loops or includes — a property
worth keeping (a client named `Acme {{ROWS}} Co` is literal text because of it).
The existing answer to "empty when unset" is a **fragment**: a placeholder whose
value is pre-built markup, empty when there is nothing to say. `{{DUE}}` is
`<br>Due: 2026-09-03` or nothing; `{{NOTES}}` is `<h3>Notes</h3><p>…</p>` or
nothing. `docs/invoicing.md` already documents the text/fragment split.

So AC #4 ("a null or empty value omits its block rather than printing an empty
label") is satisfied by adding fragments, not by adding template syntax. Four
new keys:

| New key | Kind | Value | Empty when |
|---|---|---|---|
| `{{COMPANY_BLOCK}}` | fragment | `<p class="company">Acme LLC</p>` | no `company_name` |
| `{{CLIENT_ADDRESS_BLOCK}}` | fragment | `<br>123 Main St<br>Springfield, IL` | address null/blank |
| `{{CLIENT_EMAIL_BLOCK}}` | fragment | `<br>ap@acme.test` | email null/blank |
| `{{TOTALS}}` | fragment | the money `<tr>` rows (below) | never — it always carries the total |

The seven previously-unused **text** keys keep their current meanings and stay
in the vocabulary, unchanged: `{{COMPANY}}`, `{{CLIENT_ADDRESS}}`,
`{{CLIENT_EMAIL}}`, `{{DUE_DATE}}`, `{{SUBTOTAL}}`, `{{TAX}}`, `{{PAY_URL}}`
are the escaped values an author can place themselves — the `{{DUE_DATE}}` /
`{{DUE}}` pairing this repo already ships, applied consistently. Nothing is
removed and nothing changes meaning, which is most of AC #5.

The address block joins its lines with `<br>` after escaping each one, so a
two-line address stays two lines and `<script>` in an address is text.

### Decision 2: one money vocabulary, shared by both renderers

The page and the PDF must agree about which money lines exist (AC #2), and
after TASK-64 both must show what has been paid. That is one decision, so it
lives in one place: a new `src/invoicing/document.rs`.

```rust
/// One line of the money block, in the order both documents print them.
pub struct MoneyLine {
    pub label: &'static str,
    pub amount: f64,
    /// The line a reader's eye should land on: the total, and the balance when
    /// there is one.
    pub emphasis: bool,
}

/// The figures both documents draw, and the rules about which of them appear.
pub struct MoneySummary {
    pub subtotal: f64,
    pub tax: f64,
    pub total: f64,
    pub paid: f64,
    pub balance: f64,
}

impl MoneySummary {
    pub fn of(invoice: &Invoice, paid: f64) -> Self;
    /// A line appears when it has something to say.
    pub fn lines(&self) -> Vec<MoneyLine>;
}
```

`lines()`:

| Line | Appears when | Emphasis |
|---|---|---|
| Subtotal | `tax != 0.0` | no |
| Tax | `tax != 0.0` | no |
| Total | always | yes |
| Paid | `paid > 0.0` | no |
| Balance due | `paid > 0.0` | yes |

The subtotal/tax rule is the one `render_invoice_pdf` already applies — a
one-line invoice with no tax prints one figure, which is right, and printing
`Subtotal $100.00 / Tax $0.00 / Total $100.00` on every invoice Nigel has ever
issued would be a downgrade. Adopting the PDF's existing rule for the page is
what makes AC #2 true, and it makes it true *by construction*: one function,
two consumers, and a test asserting the two documents contain the same labels
for the same invoice.

Paid and Balance are here rather than in TASK-64 deliberately. They are
document design, they belong in the side-by-side review AC #7 asks for, and
TASK-64 (republish after a payment) is then pure orchestration with nothing to
render. See that spec's "Decision 5".

**The seam supplies `paid`, and no signature above it changes.**
`render_invoice` already takes `&Connection` and already loads the line items
itself so that every caller gets the same rows; it now also calls
`paid_amount(conn, invoice.id)` and builds the `MoneySummary`. Preview, send,
republish and the two API preview routes pick it up with no edit — the same
property that made TASK-68.3's template override free for `preview`.

`render_invoice_html` and `render_invoice_pdf` each grow one parameter
(`money: &MoneySummary`), which is the whole cost.

### Decision 3: currency glyphs stay as each document already prints them

The page prints `USD 250.00` (`{{CURRENCY}} {{TOTAL}}`, `{:.2}`); the PDF prints
`$250.00` via `fmt::money` with `Total (USD)` as the label. AC #2 is about the
breakdown, and I read "consistent" as *the same lines with the same figures*,
not the same glyph.

I recommend leaving both. `fmt::money` hardcodes `$`, which is wrong for a
non-USD invoice, so pushing it onto the page would export a bug; pushing the
page's `USD 250.00` into the PDF would change every other PDF's convention or
make invoices the odd one out. Flagged as an open question — it is the kind of
thing the side-by-side review will have an opinion about.

### Decision 4: the stock page, rewritten

```html
<!doctype html>
<html><head><meta charset="utf-8"><title>Invoice {{NUMBER}}</title>
<style>…existing rules, plus .company, .bill-to, tfoot td, .totals-label…</style>
</head><body>
{{COMPANY_BLOCK}}
<h1>Invoice #{{NUMBER}}</h1>
<p class="bill-to">Billed to: {{CLIENT}}{{CLIENT_ADDRESS_BLOCK}}{{CLIENT_EMAIL_BLOCK}}<br>
Issued: {{ISSUE}}{{DUE}}</p>
<table>
  <thead><tr><th>Description</th><th>Qty</th><th>Unit</th><th>Amount</th></tr></thead>
  <tbody>{{ROWS}}</tbody>
  <tfoot>{{TOTALS}}</tfoot>
</table>
{{PAY}}
{{NOTES}}
{{TERMS}}
<h3>Direct deposit</h3>
<p>To pay by bank transfer, reference invoice <strong>#{{NUMBER}}</strong>. Contact {{CONTACT}} for account details.</p>
</body></html>
```

`{{TOTALS}}` renders `<tr><td colspan="3">Label</td><td>USD 250.00</td></tr>`
rows, the emphasised ones carrying the existing `class="total"`. Putting them in
a `<tfoot>` of the line-item table is what makes the amounts line up under the
Amount column, which is the whole reason a real invoice has a money block
rather than a sentence.

`{{TOTAL}}` stays required and stays available; the stock page no longer prints
it as a standalone paragraph because `{{TOTALS}}` includes the total line.

**`{{PAY}}` stays exactly where it is** — after the table, before notes. That
is AC #6's first half.

### Decision 5: the PDF grows the address, the email, and the pay link

Under `Billed to: {name}`, at `SUBTITLE_SIZE`, each omitted when absent:

```
Invoice #1248
Bluepeak LLC                     ← already there
Billed to: Acme Co
123 Main St                    ← new
Springfield, IL 62704          ← new
ap@acme.test                   ← new
Issued: 2026-08-04
Due: 2026-09-03
```

The address is split on newlines and drawn one line per row, matching how the
page renders it.

**And the PDF gets a pay line.** `render_invoice_pdf` grows a `pay:
PayButton<'_>` parameter (the seam already holds one) and, when it is
`Link(url)`, draws `Pay online: <url>` below the money block. That is AC #6's
second half — "the pay link placement is settled deliberately, not left unused"
— read against the real complaint in the problem statement: the two documents
disagree, and the attachment currently offers no way to pay at all. A PDF is
the thing a client forwards to their AP department, and a URL in it is
clickable in every modern reader.

`Placeholder` and `Omitted` draw nothing: an inert grey button makes sense on a
draft preview page and makes none in a PDF.

### Decision 6: templates written before this change keep working

`validate_template` requires `{{NUMBER}}`, `{{CLIENT}}`, `{{ROWS}}`,
`{{TOTAL}}` and refuses keys outside `PLACEHOLDERS`. Adding four keys to
`PLACEHOLDERS` cannot break an existing template, because validation never
requires a key to be *used*, and none of the four is added to `REQUIRED`.

The rule for this task and every later one: **`REQUIRED` does not grow.** A
template exported from an older Nigel renders exactly as it did, minus nothing.
AC #5 gets a regression test that pins this rather than trusting it — the
pre-change stock template, checked into the test module as a literal, must
`validate_template` clean and render with the same figures.

The three previously-unused text keys change nothing for an author who was
already using them.

### Decision 7: the review is a task, not a hope

AC #7 asks for a rendered example of both documents reviewed side by side
before this is called done. The plan's last task before documentation is a
**HALT**: seed a scratch data directory, render three invoices through
`nigel invoice preview`, and hand Sam six paths.

| Fixture | Exercises |
|---|---|
| Rich | company set, two-line billing address, email, tax, due date, notes, terms, a Stripe link, three line items |
| Sparse | no company, no address, no email, no tax, no due date, no notes, no terms, one line item, no link (draft) |
| Part-paid | rich, plus one payment of half the total — the Paid/Balance rows and the surviving Pay button |

The sparse one is the AC #4 case: nothing may render as an empty label, an
empty `<p>`, a stray `<br>`, or a heading with nothing under it.

## Out of scope

- Logos. Settled in TASK-68.3 and recorded in CLAUDE.md: `printpdf`'s image
  path pulls nine crates and sizes soft masks wrong, so a logo belongs on the
  HTML page, via a custom template, and the settings key stays undefined.
- A template for the PDF. Its customization stays `company_name`.
- Restyling the page beyond what the new blocks need. This is a field-parity
  task, not a redesign; the review may of course produce a follow-up.
- The email body. It is the page's HTML, so it inherits everything here.
- Rendering the payment *history* (dates, methods) on either document. The paid
  total and the balance are what a client needs; the ledger is the operator's.

## Open questions for Sam

1. **Currency glyph.** Recommended: page keeps `USD 250.00`, PDF keeps
   `$250.00`. If you want one of them, say which — `fmt::money` is `$`-only, so
   "both use `fmt::money`" means USD-only invoices print correctly and others do
   not.
2. **The pay URL in the PDF.** Recommended above. It is the one thing on this
   list that puts a *live payment link* into a document a client may keep after
   the invoice is settled — the same argument that makes void deactivate links.
   Worth a yes/no from you.
3. **Where the company block goes.** Recommended: above the `Invoice #N`
   heading, as a plain name (the PDF puts it just below the heading). Say if you
   would rather the page match the PDF's order.
4. **Paid/Balance rows landing here rather than in TASK-64.** They are what
   makes TASK-64's republish meaningful, and reviewing the document once is
   better than twice. Confirm.
5. **Address on the page.** Recommended inside the "Billed to" paragraph as
   `<br>`-joined lines. The alternative is a separate `<address>` block on the
   right, which is more invoice-like and more layout work.
</content>
