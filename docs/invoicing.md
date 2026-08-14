# Invoicing

Nigel bills clients end to end: draft an invoice, publish it as a static page and
PDF on Cloudflare R2, email it through Mailgun with a Stripe payment link, and
reconcile payments back into the books.

Invoicing is accounts-receivable only. Invoices and payments live in their own
tables (`clients`, `invoices`, `invoice_line_items`, `invoice_payments`) and never
touch the transaction register — record the bank deposit as a transaction the way
you would any other income.

Sending requires a build with the `pdf` feature (the default). Without it,
`nigel invoice send` stops at the render step — nothing is published or emailed,
because there is no PDF to upload or attach. `nigel invoice preview` is the
exception: without the feature it writes the HTML and says why there is no PDF,
rather than stopping.

## From the dashboard

Everything below is a terminal command, but the day-to-day half of it is on the
dashboard as well. Run `nigel` and press:

- `k` — **Clients.** The list, `a` to add one, `e` to edit the selected one.
  There is no delete: a client with invoices must not disappear from under them.
- `n` — **Invoices.** The list (number, status, client, total, balance, due) with
  `Enter` to open one and `a` — or `n`, if that is the mnemonic that comes to
  hand — to draft a new one. The actions live on the open
  invoice, not the list: `s` sends it, `p` records a payment against it, `v`
  voids it, and `d` deletes it when it is still an unsent draft — each with a
  confirmation, and each refused in the same words the CLI would use. `d` is
  advertised in the footer only for an invoice that can take it.

The draft form takes a client, an issue date, an optional due date (prefilled
Net 30), a currency, and as many line items as you need — `Ins` (or `F2`) adds a
row below the one you are on, `Del` (or `F3`) removes it, `Enter` creates the
draft and `Esc` throws it away. It creates **drafts only**. Sending is a
separate, confirmed action on the invoice itself, so writing an invoice and
mailing it to a client are never one keystroke apart. A refusal appears under
the field it is about, in the words `nigel invoice new` would have used.

The list shows the stored status, exactly as `nigel invoice list` does, so an
invoice that crossed its due date since it was last written still reads `sent`
rather than `overdue` until something touches it.

Sending from the dashboard blocks the terminal for the few seconds the three
network hops take. The screen says so while it waits, and keys pressed during
the wait are discarded rather than dismissing the result.

## Configuration

Secrets and endpoints resolve from the environment first, then from
`~/.config/nigel/settings.json`.

| settings.json key | Environment variable | Required for | Default |
|---|---|---|---|
| `stripe_secret_key` | `NIGEL_STRIPE_SECRET_KEY` | `send`, `sync` | — |
| `mailgun_api_key` | `NIGEL_MAILGUN_API_KEY` | `send` | — |
| `mailgun_domain` | `NIGEL_MAILGUN_DOMAIN` | `send` | — |
| `from_email` | `NIGEL_FROM_EMAIL` | `send` | — |
| `from_name` | `NIGEL_FROM_NAME` | — | the business name |
| `reply_to_email` | `NIGEL_REPLY_TO_EMAIL` | — | no Reply-To header |
| `contact_email` | `NIGEL_CONTACT_EMAIL` | — | `from_email` |
| `r2_account_id` | `NIGEL_R2_ACCOUNT_ID` | `send` | — |
| `r2_access_key` | `NIGEL_R2_ACCESS_KEY` | `send` | — |
| `r2_secret_key` | `NIGEL_R2_SECRET_KEY` | `send` | — |
| `r2_bucket` | `NIGEL_R2_BUCKET` | `send` | — |
| `public_base_url` | `NIGEL_PUBLIC_BASE_URL` | `send` | — |

A missing value is reported by name, e.g.
`missing invoicing config: r2_bucket (set it in settings.json or the matching NIGEL_ env var)`.

`public_base_url` is checked at send time as well as required. An address
without an `http://` or `https://` scheme, or with no host after it (`https://:8787/i`,
`https://user@/i`), is refused by name before any Stripe link is created,
anything is uploaded or any email goes out — `billing.example.com` is the common
mistake and reads as a relative address in a client's inbox. A **scheme-relative**
address, `//billing.example.com/i`, is refused for the same reason and is the one
worth calling out on upgrade: it resolves in a browser that already has a
scheme, and a link in an email has no page to inherit one from. Put `https:` in
front of it.

The refusal quotes the offending value in a terminal, where it is being read by
whoever just typed the command; over HTTP it names the key and the defect only,
because no API response carries a configured setting's value. A `publicUrl` on
`GET /api/invoices/{number}` is likewise `null` when the base URL cannot produce
a working link — an absent address beats a broken one. An address whose path does not end in `/i` still
sends, with a notice: Nigel writes every object under the `i/` prefix, so a base
URL pointing at the bucket root produces links that 404. The same notice appears
as `invoicing.publicBaseUrlWarning` on `GET /api/status`.

Environment variables keep credentials out of the settings file. If you do store
them in `settings.json`, note that Nigel writes that file with owner-only
permissions on Unix. Use Stripe test keys (`sk_test_…`) while trying things out.

```json
{
  "data_dir": "/home/you/Documents/nigel",
  "stripe_secret_key": "sk_test_...",
  "mailgun_api_key": "...",
  "mailgun_domain": "mg.example.com",
  "from_email": "billing@mg.example.com",
  "from_name": "Acme LLC",
  "reply_to_email": "sam@example.com",
  "contact_email": "accounts@example.com",
  "r2_account_id": "...",
  "r2_access_key": "...",
  "r2_secret_key": "...",
  "r2_bucket": "billing",
  "public_base_url": "https://billing.example.com/i"
}
```

### The letterhead

Who the invoice is *from* is not a secret and not an endpoint, so it lives in the
books rather than in `settings.json` — five `metadata` keys in the database,
edited from either front end:

| Metadata key | What it is |
|---|---|
| `company_name` | Your business name. The From block's first line, the PDF's wordmark and document title, and the name `/api/status` reports |
| `company_address` | Your address, one line per line |
| `company_phone` | Your phone. Printed as `ph. 619.555.0123` |
| `company_logo` | A PNG or JPEG as a `data:` URI |
| `payment_instructions` | Your own text about how to pay — see below |

```bash
nigel                          # dashboard -> p (Settings)
nigel serve                    # web UI -> Settings
```

The TUI's fields are single-line, so the address and the payment instructions
take `\n` as a two-character escape and store real newlines; reopening a field
shows the escape again. The web UI gives both a textarea and needs no escape.
The logo field takes a **path to an image file** in the terminal and a file
picker in the browser; either way the bytes are read, checked and stored as a
`data:` URI, so the image is part of the books and moves with them.

A logo must be a **PNG or a JPEG** and at most **128 KiB**. Nothing else is
accepted: a stored logo is base64-inflated by a third into every email body and
every published page, SVG will not render in most mail clients and cannot be
embedded in the PDF at all, and a file whose extension disagrees with its bytes
is refused rather than mislabelled into somebody's inbox. A refusal names the
problem and leaves the stored value alone.

**The Gmail caveat.** The page is the email body, and the logo travels in it as
a `data:` URI. Gmail does not render those. A Gmail reader therefore sees the
business name — the image's `alt` text — where the logo would be, while the PDF
attached beside it carries the real image. Everything else in the body renders
normally. An operator who needs the logo visible in a Gmail body can host the
image and put an absolute `<img src="https://…">` in their own
`templates/invoice.html`; Nigel does not host images, because that would make
sending an invoice depend on a second piece of infrastructure staying up.

`payment_instructions` is your text, printed under the foot rule on **both**
documents, one line per line, with a `Payment` heading. Set it to your bank
details, or to "Checks payable to …", or leave it unset — an installation that
takes no bank transfers prints nothing at all, no heading and no block. Nigel
never writes a sentence about how to pay you.

Leaving it unset is a decision, so it is one you make rather than one you
discover. `nigel invoice preview` and `nigel invoice send` print a single line on
stderr when a document would go out with no way to pay on it:

```
notice: no payment_instructions are set, so neither document says how to pay — set them in Settings, or leave them unset deliberately
```

Nothing is blocked and nothing is invented; say what you want in Settings, or
carry on. The notice is silent once instructions are set, and silent for a custom
`templates/invoice.html` — that page is yours, it may say whatever it likes about
paying, and Nigel cannot read it.

**Upgrading.** Earlier versions hardcoded a bank-transfer paragraph on the stock
page. If your books have already sent invoices and you have a `contact_email` or
`from_email` configured, the upgrade writes that sentence into
`payment_instructions` for you, so nothing your clients were reading disappears.
It is ordinary text from that moment on: edit it, or clear it. Books that have
never sent an invoice, and books where the key is already set, are left alone.

## Clients

```bash
nigel client add "Acme Co" --email ap@acme.test --address "1 Main St, Portland OR"
nigel client list
```

`--email` is optional at creation, but an invoice cannot be sent to a client
without one. `nigel client list` prints the client IDs that `invoice new` takes.

### Who receives an invoice

A client holds a **list** of contacts, each an email address with an optional
name and title. Exactly one of them is the **billing contact**: that address is
the invoice's `To`, the one `nigel client list` shows, and the one the published
page prints. Every other contact is copied — `Cc` on the same message.

```bash
nigel client edit 1 --contact "ap@acme.test:Ada Payne:AP Manager" \
                    --contact "dana@acme.test:Dana Chen:Design Lead"
nigel client show 1
```

`--contact "email[:name[:title]]"` is repeatable and **replaces the whole
list** — the same whole-list shape `invoice new --item "desc:qty:unit"` has,
split the same way, so a title containing a colon keeps its remainder. The
first one given is the billing recipient.

`--email` and `--contact` cannot be used together: one sets a single field and
the other replaces the collection, so applying both would make the order they
were applied in visible. `--email` on its own still means what it always did —
set the billing address, leave the other contacts alone.

An address is not shape-checked, on any surface: `nigel client add --email`
never has, and a form that refused what the CLI accepts would make the two
disagree about what a client is. What *is* refused is a blank address, the same
address twice on one client (case-insensitively — a cc that is also the `To` is
a duplicate delivery, not a second recipient), two billing contacts, and a line
break in any field, because these strings become mail headers.

A refusal writes nothing at all. Adding a client and its contacts is one
transaction, and so is editing one: a contact list that is turned down leaves
no client row behind and no half-applied rename.

The one exception is `nigel invoice import`, which takes what the source
database has: an address carrying a character a mail header may not is copied
verbatim, counted, and reported at the end of the run, exactly as an
unparseable date is. Refusing it would abort a whole migration over a value
nobody can correct until it has been imported. A send to that client refuses
later, by name.

**Everyone on the list receives the same document and can pay it.** One render,
one message: the identical HTML body and the identical PDF go to the `To` and
every `Cc`, Pay button included, and the published page at
`public_base_url/i/{token}/` is the same page for all of them. That is
deliberate — a second, button-less render for the cc list would create an
artifact that is not what was published, and it would achieve nothing, because
the token URL is forwardable by design. If only one person should be able to
pay, give the client only one contact.

The published page names the **billing contact only**. It is a static object on
a public URL, and printing an organisation's whole contact list onto it would
publish internal addresses to anyone the link reaches.

A name is required and must be unique: an empty one and a name another client
already has are both refused, on `client add` and on a `client edit` that
renames. Renaming a client to the name it already has is not a collision.

That rule lives in the data layer (`add_client`/`update_client`), not in the
schema: `clients.name` carries no `UNIQUE` index, matching `accounts.name` and
`categories.name`. Two requests racing each other in the web UI can therefore
both pass the check and both insert; the result is two clients with one name,
which nothing resolves by name and which you can fix by renaming one on the
clients screen. The InvoiceShelf import deliberately does not apply the rule at
all — it copies your old customer list as it stands.

### Inspecting and editing a client

```bash
nigel client show 1
nigel client edit 1 --email billing@acme.test
nigel client edit 1 --name "Acme Corporation" --address "500 Market St"
```

`client show` prints the client's details, every invoice it has ever had (newest
number first) and the balance still open against it — void and fully paid
invoices contribute nothing.

`client edit` takes `--name`, `--email`, `--address`, `--notes` and
`--contact`; the flags you leave off are left alone, and passing none at all is
an error rather than a silent no-op. A blank `--name` is refused, since the column is required. `--notes` is
internal and never appears on an invoice.

Edits take effect on the **next** send. Published pages are static snapshots on
R2, so a corrected address reaches the client when the invoice is next sent —
including a re-send of the same invoice, which overwrites the same URL. Emails
already delivered keep the old details.

### Deleting a client

```bash
nigel client delete 1
nigel client delete 1 --yes        # skip the confirmation
```

Refused while **any** invoice bills the client, of any status:

```
Cannot delete: client has 8 invoices
Run `nigel client show 1` to see them.
```

Void and fully paid invoices count too. Each one names the client on a page
that has already been sent, and an invoice whose client row is gone is a state
nothing in Nigel is allowed to create. For a client you have finished with but
have billed, archive is the operation you want.

Delete asks first. Without a terminal and without `--yes` it refuses rather
than guessing, exactly as `invoice void` does.

### Archiving a client

```bash
nigel client archive 7             # Archived client 7: Globex
nigel client unarchive 7           # Restored client 7: Globex
nigel client list                  # active clients only
nigel client list --all            # with the archived ones, and the date
```

Archiving is **not** deletion. It writes one timestamp on the client row and
touches nothing else: every invoice, payment and history row stays exactly
where it was, and no figure anywhere changes. An archived client keeps
appearing wherever its invoices do — the invoice list, the A/R aging report,
every total.

What archiving does is take the client out of the working list. It is hidden
from `nigel client list`, from the dashboard's Clients screen (`A` shows them
again) and from `GET /api/clients`, and a **new invoice for an archived client
is refused**:

```
client 'Globex' is archived — unarchive it before invoicing
```

Unarchiving is one command and makes the client invoiceable again. There is no
confirmation on either, because both are reversible in a keystroke.

## Creating an invoice

```bash
nigel invoice new --client 1 --issue 2026-08-04 --due 2026-09-03 \
  --item "Consulting:10:150" \
  --item "Hosting:1:45"
```

- `--item "desc:qty:unit"` is repeatable; at least one is required. The line total
  is `qty × unit`, and the invoice total is the sum of the lines. Descriptions
  cannot contain a colon.
- `--due` is optional. An invoice with no due date never goes overdue and ages
  from its issue date.
- `--currency` defaults to `USD` and must be a 3-letter code; it is stored
  uppercase.
- `--notes` and `--terms` are free text, e.g.
  `--notes "Thanks for the work this quarter."` and
  `--terms "Net 30. Late payments accrue 1.5% monthly."`. Both render under
  their own headings on the invoice page and on the PDF, and are omitted
  entirely when unset.
- Numbers are assigned sequentially, starting at 1248, and are not reused.

New invoices are drafts. Nothing has been rendered, uploaded, or emailed yet.

```bash
nigel invoice list            # number, status, client, total, due date
nigel invoice show 1248       # line items, amount paid, balance, payment link
```

## Previewing

```bash
nigel invoice preview 1248
nigel invoice preview 1248 --output-dir ~/Desktop
```

Preview renders exactly what `send` would publish — the same HTML page, the same
PDF — to local files, and prints where they landed:

```
Wrote /home/you/Documents/nigel/previews/invoice-1248.html
Wrote /home/you/Documents/nigel/previews/invoice-1248.pdf
```

| | Path |
|---|---|
| Default | `<data_dir>/previews/invoice-<number>.html` and `.pdf` |
| `--output-dir DIR` | `DIR/invoice-<number>.html` and `.pdf` |

The filenames carry no date. An exported report is a period snapshot you keep
beside its neighbours; a preview is a scratch view of one invoice, so
re-previewing after an edit overwrites in place and a browser reload shows the
new render. The default directory is created 0700 and every file 0600, the same
handling `nigel report` gives an export; a directory you name yourself is not
re-permissioned.

The Pay button is the only thing that can differ from what a client receives:

| Invoice state | Preview renders |
|---|---|
| Has a Stripe payment link, not void | The real link, exactly as sent |
| No link yet, not void | An inert placeholder where the button will go |
| Void | Nothing — even if the invoice still carries a live link |

A void invoice previews rather than refusing, with
`notice: invoice #1248 is void — this preview is for reference only.` on stderr.
Looking at what you cancelled is legitimate; offering a working payment link for
it is not, which is why the button is dropped even when the Stripe URL is still
in the row.

Preview is the one invoicing command that works on a fresh install: it needs no
Stripe, R2, or Mailgun configuration and makes no network call. The stock page
prints no contact line at all, so an unconfigured install previews cleanly. A
custom template that uses `{{CONTACT}}` with neither `contact_email` nor
`from_email` set renders `(contact_email not configured)` and the command says so
on stderr — the page is still complete enough to check the figures and the
layout.

In a build without the `pdf` feature the HTML is written, no PDF is, and the exit
status is still 0:

```
Wrote /home/you/Documents/nigel/previews/invoice-1248.html
notice: PDF export requires the 'pdf' feature — build with `cargo build --features pdf`
```

A PDF left over from an earlier `pdf`-enabled run is left alone rather than
deleted — it may have been kept deliberately, and the notice already explains why
it was not refreshed.

## Editing a draft invoice

```bash
nigel invoice edit 1248 --due 2026-09-30
nigel invoice edit 1248 --item "Discovery:1:2000" --item "Build:40:150"
nigel invoice edit 1248 --currency EUR --terms "Net 15"
nigel invoice edit 1248 --clear-due
```

| Flag | Effect |
|---|---|
| `--issue <YYYY-MM-DD>` | New issue date |
| `--due <YYYY-MM-DD>` | New due date |
| `--clear-due` | Remove the due date, so the invoice never goes overdue |
| `--currency <CODE>` | New 3-letter currency code, stored uppercase |
| `--notes <s>` | Replace the notes |
| `--terms <s>` | Replace the terms |
| `--item "desc:qty:unit"` | **Replaces every line item**, repeatable |

`--item` is all or nothing: leave it off and the existing lines stand, supply it
and the whole set is rewritten and the subtotal and total recomputed. There is no
way to leave an invoice with no lines. Passing no flags at all is an error.

Editing is **draft only**. A published invoice answers
`Invoice #1248 has already been sent and cannot be edited. Void it and issue a new
one.`, and a void one answers `Invoice #1248 is void and cannot be edited.` An
invoice with any payment recorded against it is also refused, whatever its status
— the client has settled against those figures, and restating them under a
recorded payment would misdescribe what was paid. That is also why a currency
change after a payment is unreachable.

If the edit moves the total or the currency on an invoice that already carried a
Stripe payment link — which happens when a send failed after the link was created
— the link is cleared, and the next `send` makes a fresh one at the right amount.

## Voiding an invoice

```bash
nigel invoice void 1248
nigel invoice void 1248 --yes
```

Void cancels an invoice. On a terminal it names the invoice and asks
`Void it? [y/N]`; anything but `y` prints `Aborted.` and changes nothing. Without
a terminal, `--yes` is required — a script gets a refusal rather than a silently
cancelled invoice.

A voided invoice leaves the aging buckets and stops being polled for Stripe
payments, but stays in `invoice list` with status `void`, and its number is never
reused. Void is terminal: there is no unvoid, and a void invoice refuses send,
pay, and edit. An invoice with payments recorded against it cannot be voided —
cancel the money side by recording the offsetting movement in the transaction
register, which is where cash actually lives.

### What void takes down

A cancelled invoice with a live payment link is the one way voiding can cost you
money: a client who pays through it pays into an invoice `sync` no longer polls,
so the payment goes unrecorded. Void therefore tears down what the invoice put
out in the world, in this order and always **after** the cancellation is
committed:

1. **The Stripe payment link** is deactivated (`active=false`; Stripe has no
   delete for payment links). The URL keeps resolving and stops taking money.
2. **The published page** is replaced with a short "This invoice has been
   voided" notice. The PDF beside it is left alone and the address keeps
   working — the document the client was sent is still the document they were
   sent.

Neither of those can fail the void. The invoice is cancelled in your books
whatever Stripe and R2 answer; a failure prints a warning naming what is still
live, with the payment link's own URL so you can deactivate it by hand.

What runs depends on what is configured, and nothing is required — void is the
one invoicing command that works on an installation with no keys at all:

| The invoice has | Configured | What void does |
| --- | --- | --- |
| no payment link | anything | nothing, silently |
| a payment link | `stripe_secret_key` | deactivates it; a failure warns and prints the URL |
| a payment link | no Stripe key | warns and prints the URL — the link is live either way |
| nothing published | anything | nothing, silently |
| a published page | the four `r2_*` keys and `public_base_url` | replaces the page; a failure warns |
| a published page | R2 incomplete or unset | warns: the page stays live and still offers to take payment |

So voiding an ordinary draft says nothing beyond `Voided invoice #1248.`, and
every extra line you do see names something that is still out there. The same
sentences appear in the dashboard's invoice screen and on the void response in
the web UI.

## Deleting a draft entered by mistake

```bash
nigel invoice delete 1252
nigel invoice delete 1252 --yes    # skip the confirmation
```

Void is a statement — it writes `voided_at`, deactivates the payment link and
republishes the page as a voided notice, precisely so a URL a client filed still
resolves to something honest. A draft created by mistake — the wrong client, a
mis-keyed command run twice, a test row on real books — has published nothing
and told nobody, and deserves to leave without a tombstone. That is what delete
is for; it is the same distinction as `client delete` against `client archive`.

**Only a draft that was never sent and has no payments can be deleted.**
Everything else refuses, in one sentence:

```
Cannot delete: invoice has been sent, paid or voided — only an unsent draft with no payments can be deleted
Run `nigel invoice void 1252` to cancel it instead.
```

| The invoice | Delete |
| --- | --- |
| a draft, never published, no payments | removes it and its line items |
| sent, partial, overdue or paid | refused — its URL and its emailed PDF are already in somebody's hands |
| void | refused — it is a record that something happened |
| a draft with any payment against it | refused — money arrived against those figures |

The guard lives in the data layer beside `ensure_editable` and `ensure_voidable`,
so a caller reaching `delete_invoice` directly cannot get past it, and the same
sentence is what the CLI prints, the dashboard puts on its status line and the
API answers as a `409` with `details.reason = "not_deletable"`.

The invoice and its line items go in one transaction. Payments are not cascaded:
the guard means a deletable invoice has none, and the delete asserts that rather
than assuming it.

Delete asks first. On a terminal it names the invoice and asks
`Delete it? [y/N]`; anything but `y` prints `Aborted.` and changes nothing.
Without a terminal, `--yes` is required, exactly as `invoice void` and
`client delete` are.

### The number is not reused

`next_invoice_number` stays where it is. Deleting the newest draft leaves a gap
in the sequence, and that is the intended outcome:

```
$ nigel invoice delete 1252 --yes
Deleted invoice #1252.
Invoice numbers are not reused — the next draft will be #1253.
```

A gap in a numbering sequence is normal and auditable. Handing #1252 out again
is not: the number may already have been quoted in an email, exported to a
spreadsheet, or referenced in a ledger somewhere Nigel cannot see, and two
different invoices sharing one number is a problem that surfaces months later
with no way to tell which was meant.

## Sending

```bash
nigel invoice send 1248
nigel invoice send 1248 --yes        # skip the confirmation (and the files)
```

**It asks first.** Before anything is created, uploaded or emailed, Nigel
renders the invoice through the same seam `send` publishes through, writes the
same two files `nigel invoice preview` writes, states who it is going to and
for how much, and waits:

```
Invoice #1248 — Acme Co, $1,850.00 USD, issued 2026-08-04. To ap@acme.test.
Wrote /home/you/Documents/nigel/previews/invoice-1248.html
Wrote /home/you/Documents/nigel/previews/invoice-1248.pdf
Sending creates a Stripe payment link, publishes the page and PDF to
billing.example.com, and emails ap@acme.test. This cannot be undone.
Send it? [y/N]
```

Anything but `y` prints `Aborted.` and exits 0 — `void`'s behaviour, and
`--yes` skips the prompt exactly as it does there. `--yes` also writes no
files: a scripted send has nobody to look at them, and leaving artifacts behind
on every run is litter. The **render still happens** either way, so a broken
custom template is caught before any gateway is called.

A non-TTY without `--yes` is refused — `Refusing to send invoice #1248 without
confirmation. Pass --yes.` — before the summary is printed and before anything
is written.

One command does the whole publish:

1. Creates a Stripe Payment Link for the invoice total, if the invoice does not
   already have one. Resending reuses the existing link, so a client who bookmarked
   it can still pay.
2. Renders the invoice to HTML and PDF — the same `render_invoice` seam
   `nigel invoice preview` writes locally, so a preview cannot disagree with
   what is published.
3. Uploads both to R2 as `i/{token}/index.html` and `i/{token}/invoice.pdf`, where
   `token` is the invoice's random 16-character identifier. The address Nigel
   hands out names the `index.html` object itself — see "Hosting" below.
4. Emails the client through Mailgun — HTML body, PDF attached, subject
   `Invoice #1248 from Acme LLC`, or plain `Invoice #1248` when no business name
   is set. The name comes from the same setting the dashboard's settings screen
   edits. The From carries a display name and, when one is configured, a
   Reply-To — see "Who the email is from" below.
5. Marks the invoice published, which moves it from `draft` to `sent` (or straight
   to `overdue` if its due date has already passed).

If any step fails the invoice stays a draft and no email goes out, so a failed
send is safe to retry. The command prints the public URL on success:
`Sent invoice #1248: https://billing.example.com/i/aBc123.../index.html`.

The published page shows your letterhead, the invoice metadata, who it is for,
the line items, the money block, a Pay button linking to Stripe, and — under the
foot rule — the notes, the terms and whatever `payment_instructions` says. It
carries no contact line unless a custom template asks for one with
`{{CONTACT}}`.

### Who the email is from

Four settings decide what a client sees at the top of the message, and they are
four different jobs:

| Setting | What it is |
|---|---|
| `from_email` | the address Mailgun sends from |
| `from_name` | the display name beside it; unset means the business name |
| `reply_to_email` | the `Reply-To`; unset means the message carries no such header |
| `contact_email` | what `{{CONTACT}}` prints, for a custom template that uses it |

```
From: Acme LLC <billing@mg.example.com>
Reply-To: sam@example.com
To: ap@acme.test
Subject: Invoice #1248 from Acme LLC
```

`from_email` should be on `mailgun_domain`. When it is not, the send reports a
warning and goes ahead: a Mailgun domain of `mg.example.com` sending for
`billing@example.com` is a common, deliverable setup, and only your Mailgun
account knows which senders it has verified. The warning appears on stderr from
the CLI, on the status line in the dashboard, and as `configWarnings` on the
send response. The reply-to is not checked at all — Mailgun constrains what a
message is sent *from*, not where a human replies to it.

`from_email` must be a **bare address**. Putting the display name in it —
`Acme LLC <billing@mg.example.com>`, which is how you would have had to do it
before `from_name` existed — is refused, because the name goes in `from_name`
now and composing both would produce a header Mailgun rejects.

A display name containing a comma or a quote is encoded for you
(`"Carter, Sam" <billing@mg.example.com>`); you never quote it yourself. A
control character in `from_email`, `from_name`, the business name or
`reply_to_email` is refused by naming where it came from, and the send stops
before any network call: a header carrying a newline can add recipients nobody
chose.

### From the web UI

`nigel serve` sends the same five steps through
`POST /api/invoices/{number}/send`, with two differences worth knowing.

**It asks first, and the asking is enforced.** The request body must carry
`{"confirm": true}`; without it the server answers `400` and sends nothing. A
confirm dialog on a screen is a convention the next screen can forget, so the
flag makes the dialog the only way to reach the endpoint.

**It shows the document.** The dialog frames the rendered invoice page above
the confirm button — the same bytes the send will publish, fetched through the
preview route, which constructs no gateway and creates no Stripe link. The PDF
is offered as a download beside it. A build that cannot render a PDF says so
and refuses the send up front, because the PDF is attached to the email; the
page still renders. A broken custom template arrives as a sentence naming the
path, not as an error envelope drawn inside the frame.

**It says which step failed.** Where the CLI prints one error, the response
names the step (`config`, `load`, `precheck`, `payment_link`, `render`,
`publish`, `email`, `record`), the service behind it, the steps that did
complete, and — the one that matters — whether the email had already gone out.
Everything before the email is safe to retry; a failure at `record` after it is
not, because the client already has the invoice. Nothing retries automatically.
A completed send answers with the same trace, so a screen can say what happened
rather than just "done".

Every outbound call is bounded (10s to connect, 30s in total, each), which
`nigel invoice send` benefits from too: before that, a connection that was
accepted and never answered hung the terminal. A send makes five of them — two
to Stripe, two to R2, one to Mailgun — so the slowest possible send is about
150 seconds rather than forever.

## In the browser

`nigel serve` covers the whole of the above. **Clients** in the sidebar is the
client manager: add, edit, delete (refused while any invoice bills them, of any
status), and a jump to each client's invoices. **Invoices** is the list, with
an A/R aging strip above it and status filters that are links, so a filtered
list is an address you can keep or bookmark.

Opening an invoice gives you its totals, its line items, its payment history,
the addresses it was published to, and a collapsed preview of the page a client
opens — rendered by the same code `invoice send` publishes, so it is the real
document rather than an impression of it. The actions on that screen are the
CLI's: **Send…**, **Record payment…**, **Edit**, **Void…** and **Delete…**, each
enabled by the server's own guards rather than by a status the browser reasoned
about. Delete is offered only for a draft nobody has seen; deleting one returns
to the list with a toast, since there is no longer an invoice to return to.

Sending asks first, and the confirmation names every consequence — the payment
link and its amount, the host the page will be published to, and the address
the email is going to — before anything happens. If a step fails, the dialog
stays open and shows where it stopped, in plain terms, with the gateway's own
message underneath it. A failure after the email went out offers no retry, and
says why: the client already has the invoice.

Drafting a new invoice is a full screen rather than a dialog, with repeatable
line items and a running subtotal. A/R aging lives under **Reports** as its
ninth entry, since it is a report to read rather than a thing to act on; the
strip on the invoice list links straight to it.

Two things the browser does not do, both because the CLI does not either: it
never guesses at an email address's shape, and it cannot un-void an invoice.

## Customizing the invoice page

The page a client opens is yours to change without rebuilding Nigel. Put a file
here and it renders instead of the built-in one:

```
<data_dir>/templates/invoice.html
```

No file there means the built-in page renders, exactly as it always has.

```bash
nigel invoice template export                      # write the built-in page out to edit
nigel invoice template export --output ~/mine.html # somewhere else
nigel invoice template export --force              # overwrite an existing template
nigel invoice template path                        # where Nigel looks, and what it found
```

`export` refuses to overwrite an existing file without `--force`, because the
file it would clobber is your own work. Neither command opens the database, so
both run on a machine that has never seen `nigel init`.

### The iteration loop

```bash
nigel invoice template export
$EDITOR ~/Documents/nigel/templates/invoice.html
nigel invoice preview 1248 && open ~/Documents/nigel/previews/invoice-1248.html
```

`preview` renders through the same code `send` publishes through, makes no
network call, and needs no Stripe, R2, or Mailgun configuration — so a template
can go through twenty revisions before anything is ever sent.

### Placeholders

A template is plain HTML with `{{KEY}}` placeholders. There are no conditionals,
loops, or includes: the fragment placeholders are already empty when they have
nothing to say.

| Placeholder | Kind | Value |
|---|---|---|
| `{{NUMBER}}` | text | Invoice number (**required**) |
| `{{CLIENT}}` | text | Client name (**required**) |
| `{{CLIENT_EMAIL}}` | text | The billing contact's address, empty when the client has none |
| `{{CLIENT_EMAIL_BLOCK}}` | fragment | `<br>ap@acme.test`, empty when the client has none |
| `{{CLIENT_ADDRESS}}` | text | Client billing address, empty when unset |
| `{{CLIENT_ADDRESS_BLOCK}}` | fragment | One `<br>`-prefixed line per typed line, empty when unset or blank |
| `{{COMPANY}}` | text | Your business name, empty when unset |
| `{{COMPANY_ADDRESS}}` | text | Your address as typed, newlines and all, empty when unset |
| `{{COMPANY_PHONE}}` | text | Your phone, empty when unset |
| `{{COMPANY_BLOCK}}` | fragment | The whole ruled From block — name, address lines, `ph.` line — empty when name, address and phone are all unset |
| `{{LOGO}}` | fragment | `<img class="logo" src="data:…">`, empty when no logo is configured or the stored one cannot be used |
| `{{ISSUE}}` | text | Issue date |
| `{{DUE_DATE}}` | text | Due date, empty when there is none |
| `{{DUE}}` | fragment | `<br>Due: …`, empty when there is none |
| `{{META_ROWS}}` | fragment | The invoice metadata as `<tr><th>Label</th><td>value</td></tr>` rows — Invoice ID, Issue Date, and Due Date when there is one. Never empty: an invoice always has a number and an issue date |
| `{{ROWS}}` | fragment | The line-item `<tr>` rows (**required**) |
| `{{CURRENCY}}` | text | Currency code |
| `{{SUBTOTAL}}` | text | Subtotal, two decimals |
| `{{TAX}}` | text | Tax, two decimals |
| `{{TOTAL}}` | text | Total, two decimals (**required**, or `{{TOTALS}}`) |
| `{{TOTALS}}` | fragment | The whole money block as `<tr>` rows — see below. Never empty: it always carries the total |
| `{{NOTES}}` | fragment | Notes block, empty when unset |
| `{{TERMS}}` | fragment | Terms block, empty when unset |
| `{{TERMS_BLOCK}}` | fragment | Terms block, empty when unset **or** when the terms already rode beside the due date in `{{META_ROWS}}` |
| `{{PAYMENT_INSTRUCTIONS}}` | text | Your payment instructions as typed, empty when unset |
| `{{PAYMENT_BLOCK}}` | fragment | `<h3>Payment</h3>` and one line per typed line, empty when unset |
| `{{PAY_URL}}` | text | Stripe payment link, empty when there is none |
| `{{PAY}}` | fragment | The Pay button, empty when there is no link |
| `{{CONTACT}}` | text | Contact address (`contact_email`, or `from_email`) |

**Text** placeholders are HTML-escaped values you can put in element content or
inside a quoted attribute value. **Fragment**
placeholders are pre-built markup and are content-only — putting one in an
attribute produces broken HTML. The two kinds pair up: the fragment is the block
that vanishes when there is nothing to say, and the text key beside it is the
escaped value for an author who would rather place it themselves. `{{DUE_DATE}}`
/ `{{DUE}}`, `{{COMPANY}}` / `{{COMPANY_BLOCK}}`, `{{CLIENT_ADDRESS}}` /
`{{CLIENT_ADDRESS_BLOCK}}`, `{{CLIENT_EMAIL}}` / `{{CLIENT_EMAIL_BLOCK}}`,
`{{PAYMENT_INSTRUCTIONS}}` / `{{PAYMENT_BLOCK}}` and `{{PAY_URL}}` / `{{PAY}}`
are all that pairing.

The stock page uses `{{META_ROWS}}`, `{{TERMS_BLOCK}}` and `{{PAYMENT_BLOCK}}`.
`{{DUE}}`, `{{DUE_DATE}}`, `{{TERMS}}` and `{{CONTACT}}` are unchanged and remain
available: a template that carries them renders exactly what it always rendered —
`{{DUE}}` is still `<br>Due: 2026-09-05` with no terms folded into it, and
`{{TERMS}}` is still the block whenever terms are set. The newer keys are the
ones that keep the two documents saying the same thing, which is why the stock
page moved to them.

`{{TOTALS}}` is the money block: one `<tr><td colspan="3">Label</td><td>$250.00
</td></tr>` per line, the emphasised ones carrying `class="total"`. Which
lines exist is decided in one place for both documents — Subtotal and Tax only
when there is tax, Total always, Paid and Balance due once anything has been
paid, and Credit when someone has paid more than the invoice asked for — so the
page and the PDF cannot disagree about the same invoice. The stock page puts it
in a `<tfoot>` of the line-item table, which is what lines the amounts up under
the Amount column, and pads its first row by two body sizes so the block stands
off the last item row instead of reading as one more of them. The PDF stands its
own block off the same way.

A balance is never negative. An invoice settled to within half a cent is settled
— the same test `refresh_status` uses to call it `paid`, so a page can never
print a balance under a status that says otherwise — and anything paid beyond
the total is a **Credit** line rather than a minus sign on the amount owed.

`{{TOTAL}}` remains available and remains what a template must carry, except
that a template using `{{TOTALS}}` satisfies that requirement too: the block is
the total, plus whatever else is true.

Where a placeholder may go:

> Placeholders are safe in element content and inside quoted attribute values.
> Do not put one inside `<script>`, `<style>`, an unquoted attribute, or a
> position where the value becomes a URL scheme.

Every value is escaped on the way in, so a client named `Acme <script> Co` is
text on the page and never markup. The template itself is **not** sanitized —
your `<script>` and your web fonts are the feature, and anyone who can write the
file can already run programs as you.

### When a template is refused

A template that is there but broken is an error naming the path. Nigel does not
quietly mail the stock page instead: an invoice you did not approve reaching a
client is worse than a send you have to retry. On `send` the template is loaded
before the Stripe link is created, so a broken one costs nothing — no link, no
upload, no email.

| Condition | What you get |
|---|---|
| Cannot be read (permissions, a directory, invalid UTF-8) | `Cannot read invoice template <path>: …` |
| Empty or whitespace only | `Invoice template <path> is empty.` |
| Larger than 1 MiB | `Invoice template <path> is <n> bytes; the limit is 1 MiB.` |
| Missing `{{NUMBER}}`, `{{CLIENT}}`, `{{ROWS}}`, or both `{{TOTAL}}` and `{{TOTALS}}` | `… is missing required placeholder(s): {{TOTAL}}. …` |
| Uses a `{{KEY}}` that is not in the table above | `… uses unknown placeholder(s): {{TOTL}}. …` |

The four required placeholders are what an invoice is — which invoice, who owes,
for what, how much. An unknown one is always a typo, and refusing is how you find
out before `{{TOTL}}` appears on a page someone is reading.

That list does not grow. New placeholders are always optional, so a template you
exported from an older Nigel keeps loading and keeps rendering exactly what it
rendered before — it simply gains nothing until you edit it in.

Only `{{` + `SCREAMING_SNAKE` + `}}` counts as a placeholder, so a CSS rule, a JS
template literal, or a `{{ not a key }}` aside passes through as literal text.
Checking happens when the template is loaded, which is why `nigel invoice
template path` and `nigel invoice preview` both report the problem.

## Customizing the invoice PDF

The PDF has no template. It is drawn by code, from the same letterhead and the
same shared decisions the page draws from, and it carries the same blocks in the
same order:

```
[logo]                          From
                                | Bluepeak LLC
                                | P.O. Box 1234
                                | Springfield, CA 90001
                                | ph. 619.555.0123

Invoice ID   1248               Invoice For
Issue Date   2026-08-04         | Acme Co
Due Date     2026-09-05         | 123 Main St
             (Net 30)           | Springfield, IL 62704
                                | ap@acme.test


Description        Quantity  Unit Price     Amount
==================================================
Design                    2     $100.00    $200.00
--------------------------------------------------
Research                  4     $100.00    $400.00   <- shaded
--------------------------------------------------
                                       Total   $600.00

--------------------------------------------------
Notes
Terms
Payment
```

There is **no title line**. The letterhead is the masthead and the metadata band
carries the identifier, so `Invoice ID  1248` says it once rather than printing
a heading over a row that repeats it. The number is still the document's file
title — what a viewer puts in its window and what a browser suggests as a
filename — and still the page's `<title>`; it is simply not drawn twice on the
document itself.

Line-item rows are ruled and every other one is tinted, which is what lets a
reader follow one row across four columns. Both documents stripe the *same*
rows: which ones is `document::row_is_shaded`'s decision, taken once. The
striping and the row rules continue correctly onto a second page — the row that
does not fit starts the new page and paints its band, its cells and its rule
there.

Every block is drawn only when there is something in it. A client with a name
and nothing else gets no empty rows and no labels with nothing after them; an
installation with no letterhead gets no `From` heading, no rule and no bare
`ph.`; an invoice with no due date has no Due Date row. The address is drawn at
most six lines deep with `...` for the rest — this document has no page-break
logic under that block, and a client block running off the bottom margin is
never what anyone wanted. The page clamps identically, so the two still agree.

Nothing runs off the right edge. Address lines wrap inside their column; a
business name too wide for the space beside the From block is set smaller until
it fits, and cut with `...` only if shrinking it would take it below the body
size; a metadata value too long for its column — a due date carrying a whole
sentence of terms — is cut the same way. The page has no such limits, so a
letterhead that is comfortable there can still be tight here: the preview is
where you find that out.

Single-line terms ride beside the due date as `2026-09-05 (Net 30)` on both
documents. Terms that run to a paragraph stay their own block under the foot
rule instead, because a paragraph in parentheses after a date reads as neither.
Whichever way they fall, they appear once.

The three figure columns are sized to what the invoice being rendered actually
holds — the wider of the heading and the widest figure in the column, plus the
gutters that face a divider — and **Description takes every millimetre left
over**. A figure is a short string that never wraps, so width beyond what it
sets in is slack, and sizing every column for the longest form a figure might
take spent that slack out of the one column with prose in it: a description that
set in three lines on the page took five here. A dollar invoice gives
Description about 99 mm of the 178 mm measure; the same invoice in euros, whose
figures carry a `EUR ` prefix, gives it about 88 mm. Neither clips.

Below the line items the PDF prints the same money block the page does, from the
same rule: Subtotal and Tax only when there is tax, Total always, Paid and
Balance due once anything has been paid, and Credit on an overpayment. Its
figures end on the same edge the Amount column's do, which is the right text
margin, so the two columns of figures read as one. It stands three body sizes
clear of the table's last rule, and the page pads its first totals row by two,
so on neither document does the block read as another row.

Every figure on both documents reads one way: thousands separators, two
decimals, and the currency named the same in the item table as in the money
block. A dollar invoice prints `$6,600.00`; every other currency is prefixed
with its code, `EUR 6,600.00`. A code is unambiguous where a symbol is not — `$`
alone cannot say US, Canadian or Australian — and it survives the PDF's built-in
fonts intact, which not every currency symbol does. Nothing says the currency
twice: the Total row is labelled `Total`, not `Total (USD)`.

Every line of that block is set at one size on both documents, and **only the
bottom one is bold** — whichever line that is. On an unpaid invoice it is Total;
once something has been paid it is Balance due; on an overpayment it is Credit.
The bottom line is what the invoice actually leaves owing, and a column of
figures with one of them picked out reads as a bill, where two lines set large
with a small one between them reads as two headlines and a whisper.

The blocks under the foot rule — Notes, Terms, Payment — run to the **full
printable width**. They are prose, and prose set to the description column's
measure runs to three short lines where it should run to one.

`company_name` also becomes the PDF's document title (`Bluepeak LLC - Invoice
#1248`), which is what a viewer puts in its window and what a browser suggests as
a filename. Leave it unset and the document is headed by the invoice number
alone — nothing is invented and no placeholder appears.

Everything else about the PDF — typography, the measure, the order of the
blocks — is fixed, and the column widths are the renderer's own arithmetic
rather than a setting. Customize the HTML page instead; it is the artifact
clients open, and the PDF rides along as the attachment.

### The logo in the PDF

The PDF embeds the **real logo**, top left, fitted into a box that is a share of
this document's printable width — `document::LOGO_WIDTH_FRACTION` by
`LOGO_HEIGHT_FRACTION`, about 36 × 10 mm — with its aspect ratio kept. A wide
wordmark fills the width and a tall mark fills the height, and neither is
stretched. The page bounds its `<img>` by the same two fractions of its own
measure, so the mark reads at the same size on both. It ends level with the From
block beside it.

Two things made that affordable:

- **Cost, measured rather than estimated.** `printpdf`'s `embedded_images`
  feature pulls nine crates — `image`, `png`, `gif`, `jpeg-decoder`, `tiff`,
  `color_quant`, `fdeflate`, `bytemuck` and `bitflags` — because its `image`
  dependency hard-enables every format; PNG alone is not on offer. On identical
  source, with the same web assets embedded on both sides, turning the feature on
  cost **84,496 bytes** of release binary (26,075,256 → 26,159,752) — about
  83 KiB on a 25 MiB binary.
- **The soft-mask defect, made unreachable.** printpdf 0.7 sizes a transparent
  image's mask from the image's *width*, so a wide transparent wordmark — the
  shape a logo actually is — embeds wrong. Nothing here ever hands printpdf an
  RGBA image: any alpha is composited onto white first, in the one function that
  builds what printpdf receives, so the broken path is not reachable rather than
  merely avoided. White because a PDF page is white, and compositing onto the
  surface the image will sit on is the only choice that is not an invention about
  someone's brand.

**A logo problem never fails a render or a send.** A stored value that is not a
data URI, not a PNG or JPEG, over the size cap, undecodable, or zero-sized, and
an image the encoder will not take, all end the same way: the PDF draws the
business name as a text wordmark and the page renders no `<img>` at all. An
invoice is a document about money; a logo is decoration on it, and decoration
does not get to stop the money.

The PDF carries **no payment link and no URL of any kind**, deliberately. An
emailed attachment cannot be recalled or republished, so a live charge link
inside one would outlive the settlement it was created for — the same reasoning
that makes voiding an invoice deactivate its Stripe link, and nothing deactivates
a link when an invoice is paid. Printing the page's address instead was
considered and rejected: a tokenized URL as unclickable text is sixty characters
of noise beside the figure that matters, and the email already carries the link.
Paying online belongs to the published page, which is the artifact Nigel can
correct after the fact.

## Recording payments

Stripe payments are pulled in, not pushed by webhook:

```bash
nigel invoice sync
```

`sync` walks every open invoice (`sent`, `partial`, or `overdue`) that has a
payment link, asks Stripe for that link's completed checkout sessions, and records
any it has not seen. Payments are keyed by checkout session ID, so re-running it
records nothing twice. It prints `Recorded N new payment(s)`, and a notice per
invoice Stripe refused — a deleted payment link 404s forever, and one of those
must not stop the rest of the run.

`POST /api/invoices/sync` is the same run over HTTP. It answers with the count,
how many invoices were checked, `recordedInvoices` (the numbers a payment landed
against — what a browser needs to say *which* invoices moved), and those
per-invoice failures as data rather than as stderr a browser cannot read. Only a
run where *every* invoice failed is an error. Each invoice it moved has its
published page corrected, with `republishWarnings` carrying anything that could
not be.

Payments made outside Stripe are entered by hand:

```bash
nigel invoice pay 1248 --date 2026-08-20                        # the whole balance
nigel invoice pay 1248 --date 2026-08-20 --amount 500           # a partial payment
nigel invoice pay 1248 --date 2026-08-20 --method ach
```

`--amount` defaults to the outstanding balance and must be positive; overpayments
are allowed, since banks make them. `--method` is one of `stripe`, `ach`,
`direct_deposit` (the default), or `other`.

`--date` must be a real date in `YYYY-MM-DD`; anything else is refused rather
than recorded. A month or day you typed without its leading zero is accepted and
stored padded — `--date 2026-8-9` lands in the books as `2026-08-09`, which is
also what `--issue` and `--due` do on `invoice new` and `invoice edit`. Dates are
stored one way so they compare and sort as dates.

### The published page is corrected

A payment against a **published** invoice re-renders the page and the PDF and
puts them back where the client is looking, so following a bookmarked link shows
the balance that is actually outstanding — and, once the invoice is settled, no
Pay button at all. A payment against an unpublished invoice has no page to
correct and reaches nothing.

It is **best-effort**, exactly as void's teardown is. The payment is committed
first and nothing afterwards can undo it:

| What happened | What you get |
|---|---|
| Nothing published | nothing, silently |
| Republished | nothing — the page is right |
| No R2 keys configured | `Warning: invoice #1248 was paid but the R2 publisher is not configured, so its published page still shows the old balance.` |
| R2 refused | `Warning: could not republish invoice #1248's page (r2 403: …). It still shows the old balance.` |

Either way the payment is recorded. A build without the `pdf` feature replaces
the page only, leaving the attachment the client was actually sent — the same
rule void follows when it takes a page down.

`POST /api/invoices/{number}/pay` therefore **makes network calls** when the
invoice is published: two uploads, bounded like every other invoicing call, so
about a minute at worst. It answers the refreshed invoice as it always did, plus
`republishWarnings` when there are any.

### Sync on launch

Every subcommand that reads or writes the books runs a sync first, as long as a
Stripe secret key is configured. It is best-effort: it prints
`notice: recorded 2 new invoice payment(s)` when it finds something and
`notice: invoice sync skipped: <reason>` when Stripe or the network is unavailable,
and either way the command you typed runs normally. A payment it finds against a
published invoice corrects that page too, printing the same `notice:` warnings
when it cannot. `init`, `demo`, `load`,
`update`, `password`, `restore`, `completions`, `invoice sync` itself, and
`invoice preview` skip the hook — preview is defined to make no network call, and
the launch sync would make that false on a configured machine.

## Trying it end to end in test mode

With Stripe test-mode keys and a scratch R2 bucket exported, the whole round trip
can be rehearsed against a throwaway data directory:

```bash
nigel client add "Test Client" --email you@example.com
nigel invoice new --client 1 --issue 2026-08-04 --item "Consulting:1:100"
nigel invoice send 1248        # publishes to R2, emails, creates the Stripe link
# pay via the emailed link with Stripe's test card 4242 4242 4242 4242, then:
nigel invoice sync             # records the payment and flips the invoice to paid
nigel invoice aging            # the settled invoice is out of the buckets
```

## Status

Status is derived from what has happened to the invoice, and is recalculated
whenever it is published or paid:

| Status | Meaning |
|---|---|
| `draft` | Created but never published |
| `sent` | Published, nothing paid |
| `partial` | Published, paid in part |
| `overdue` | Published, past its due date, with a balance |
| `paid` | Paid in full (settled to within half a cent) |
| `void` | Cancelled; cannot be sent, paid, or edited |

Status is **stored**, not computed when you read it. It is re-derived only by a
write to the invoice, and each write names the day it derives against: `invoice
send` uses the publish date, a payment uses the payment's own date (so entering
last month's cheque today does not advance anything else), and an edit or a void
uses the day the command runs. `invoice sync` re-derives only for an invoice it
records a new Stripe payment against.

An invoice that simply passes its due date with nothing else happening to it
therefore keeps the status it was last written with — nothing recomputes it in
the background, and reading the list does not recompute it either. **A/R aging is
the report to trust for how late something is**: it measures every open invoice
against today's date directly rather than reading the stored word.

## A/R aging

```bash
nigel invoice aging                    # print the table
nigel report aging                     # the same report, browsable
nigel report aging --mode export       # PDF into <data_dir>/exports/
```

Buckets the outstanding balance of every open invoice by how long it has been due
— `current`, `1-30`, `31-60`, `61-90`, `90+` days past due. Invoices with no due
date age from their issue date. Drafts, void invoices and anything settled in
full are left out. Both commands are always as of today; there is no as-of date
to pass. The JSON API's `GET /api/invoices/aging` does take an optional `asOf`,
which is the one place that differs — see [`api.md`](api.md).

Both commands print the same numbers — `invoice aging` prints the report's own
table:

```
Acme Consulting LLC

A/R Aging — as of 2026-08-07

Summary
+-------------------+----------+-----------+
| Bucket            | Invoices | Amount    |
+==========================================+
| current           | 1        | $1,500.00 |
|-------------------+----------+-----------|
| 1-30              | 1        | $1,500.00 |
|-------------------+----------+-----------|
| 31-60             | 0        | $0.00     |
|-------------------+----------+-----------|
| 61-90             | 0        | $0.00     |
|-------------------+----------+-----------|
| 90+               | 1        | $3,200.00 |
|-------------------+----------+-----------|
| Total Outstanding | 3        | $6,200.00 |
+-------------------+----------+-----------+

Open Invoices
+---------+---------+------------+------+-----------+
| Invoice | Client  | Due        | Days | Balance   |
+===================================================+
| #1250   | Initech | 2026-05-04 | 95   | $3,200.00 |
|---------+---------+------------+------+-----------|
| #1249   | Acme Co | 2026-07-20 | 18   | $1,500.00 |
|---------+---------+------------+------+-----------|
| #1248   | Acme Co | 2026-08-27 | —    | $1,500.00 |
+---------+---------+------------+------+-----------+
```

`nigel report aging` opens the interactive view on a terminal (scroll with
↑/↓, `q` closes) and falls back to this text when piped. The dashboard offers
it under both `v` (view a report) and `e` (export a report), and the home
screen shows the outstanding total whenever any invoice is open.

## Importing from InvoiceShelf

```bash
nigel invoice import --from-invoiceshelf ~/invoiceshelf/database.sqlite
```

Copies customers, invoices, line items, and payments out of an InvoiceShelf SQLite
database, converting its integer cents to Nigel's dollar amounts. Imported
invoices keep their original numbers and arrive already published — `paid` or
`sent`, following InvoiceShelf's paid status — with their payments recorded under
method `other`. The next number Nigel assigns continues above the highest number
imported. The command reports what it moved:
`Imported 12 clients, 87 invoices, 91 payments. Next invoice number: 1361`.

Run it once, against a fresh Nigel database — it does not reconcile against
invoices that already exist.

## Hosting: billing.example.com → R2

Published invoices are static files in an R2 bucket. Nigel uploads them with the
S3 API to `https://{r2_account_id}.r2.cloudflarestorage.com`, using the R2 access
key pair, and builds client-facing links from `public_base_url`.

The two sides meet at Cloudflare: expose the bucket at a hostname you control —
a custom domain on the bucket, or an equivalent route into it — so an object
stored at key `i/{token}/index.html` is served at, for example,
`https://billing.example.com/i/{token}/index.html`, and set `public_base_url` to
`https://billing.example.com/i`. Keep the `i/` prefix in `public_base_url` aligned
with that mapping — Nigel writes keys under `i/`, and the base URL only tells it
what public address that prefix answers on.

Nigel names the file, not its directory: every link it prints, returns and
reports ends in `/index.html`. A plain R2 custom domain serves objects by key
and has no directory-index behaviour, so `…/{token}/` would 404 while the object
beside it resolves. The file form works on a bare custom domain, on S3 static
hosting, behind a Worker, and on a synced local copy, without asking anything of
the host.

If you would rather hand out the directory form, add an edge rewrite — a
Cloudflare transform rule or a Worker that appends `index.html` to a path ending
in `/`. That is an option, not a requirement: with the rewrite in place both
addresses resolve to the same object, and Nigel keeps linking to the file.

Tokens are random and unguessable, and nothing enumerates the bucket, so an
invoice is readable only by someone holding its link.

To bill from a different domain, point that hostname at your bucket and set
`public_base_url` (or `NIGEL_PUBLIC_BASE_URL`) to its `…/i` prefix.
