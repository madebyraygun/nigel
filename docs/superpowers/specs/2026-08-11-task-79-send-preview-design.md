# Seeing the invoice before it goes out

Task: TASK-79 (high), stream 2 of epic TASK-86.

## Problem

Send is the one irreversible step in the system. Once Mailgun accepts the
message the client has the invoice; nothing retries it and nothing recalls it.
Neither surface shows the operator the document before that happens.

**The CLI does not even ask.** `nigel invoice send 1248` takes one positional
argument, no flags, prints `Sent invoice #1248: <url>` after the fact. That is
out of step with the rest of the binary: `invoice void` prints a summary, prints
the published-void notice, and prompts `Void it? [y/N]` unless `--yes`;
`recategorize` by filter refuses without `--yes`; and the *TUI* already confirms
a send with a three-line summary naming the client, the total and the recipient
(`cli/invoice_manager.rs::send_confirmation`). The CLI is the only send surface
with no gate at all.

**The web explains but does not show.** `wc-send-dialog` lists what will happen
— create the payment link, publish to the host, email the recipient — and has a
`subject` property. It never renders the document. The one component that does,
`wc-invoice-preview`, is collapsed by default on the *detail* view, which is a
different place from the dialog that sends.

The capability exists and is simply not on the path: `render_invoice` is the
seam `send` publishes through, `nigel invoice preview` writes it locally with no
network call and no configuration, and `GET /api/invoices/{number}/preview`
serves it with no gateway in reach.

## Where the code is today

| Thing | Location |
|---|---|
| CLI send | `src/cli/invoice.rs::send` — `find_invoice` → `ensure_not_void` → `load_template` → `build_clients` → `send_invoice` |
| CLI preview | same file — `preview`, `preview_dir`, `preview_paths`, `pay_button_for`, `contact_email_for_preview`, `PREVIEW_CONTACT_PLACEHOLDER` |
| CLI void's confirmation | same file — `void`, `confirm_void` (`--yes`, non-TTY refusal, `Aborted.`) |
| clap | `src/cli/mod.rs::InvoiceCommands::{Send, Void, Preview}` |
| The seam | `src/invoicing/render.rs::render_invoice` → `RenderedInvoice { html, pdf: Option<Vec<u8>> }` |
| Send orchestration | `src/invoicing/send.rs` — `SendStep`, `send_invoice_traced`, `PDF_REQUIRED_MESSAGE` |
| API send | `src/server/routes/invoices.rs` — `send` (requires `{"confirm": true}`), `send_with`, `send_error` |
| API preview | same file — `…/preview` (HTML, `Content-Security-Policy: sandbox`, `X-Frame-Options: SAMEORIGIN`) and `…/preview.pdf` (501 without the feature) |
| The dialog | `web/packages/ui/src/components/wc-send-dialog.ts` (+ `.preview.ts`, `.test.ts`) |
| The frame | `web/packages/ui/src/components/wc-invoice-preview.ts` — `PREVIEW_SANDBOX`, `<details>` + lazy `<iframe src>` |
| The screen | `web/apps/app/src/screens/invoices.ts` — `openSend`/`handleSend`/`closeSend`, `sendBlockReason`, `hostOf` |
| Pure half | `web/apps/app/src/screens/invoice-data.ts` (`sendStepViews`, `SEND_STEP_LABELS`), `invoicing-errors.ts` (`sendFailureMessage`) |
| The api seam | `web/apps/app/src/api/client.ts` — `invoicePreviewUrl(number, 'html' \| 'pdf')`; guard test `src/__tests__/api-seam.test.ts` |

Two things the acceptance criteria assume that are already true, and one that
is the opposite of what it sounds like:

- **AC #3 and #4 hold at the seam already.** `render_invoice` reads the
  database and makes no network call; the preview routes construct no gateway.
  Nothing in this task may introduce a second render path — that is the whole
  constraint.
- **AC #7 is already true of the ordering.** Both `cli::invoice::send` and
  `server::routes::invoices::send_with` call `load_template` *before* any client
  is built or called, so a broken override costs no Stripe link, no upload and
  no email (TASK-68.3's ordering). What this task adds is that the failure
  becomes *visible* at the moment the operator is deciding, instead of arriving
  as a failed send.
- **AC #6 is subtler than it reads.** A build without the `pdf` feature cannot
  *send at all* — `send_invoice_traced` hits `PDF_REQUIRED_MESSAGE` at the
  `render` step and the API answers 501. So "previews the HTML and says why
  there is no PDF, rather than blocking" is about the **preview** not blocking;
  the send is genuinely blocked, and both surfaces should say so up front
  instead of discovering it three steps in.

---

## The CLI

### The command

```
nigel invoice send <NUMBER> [--yes]
```

`--yes` is worded exactly as void's: *Send without confirmation (required when
stdin is not a TTY)*.

### What happens, in order

1. `find_invoice`, `ensure_not_void`, `get_client` — unchanged.
2. **`load_template`** — unchanged position, and the thing that makes a broken
   override an error here rather than after a Stripe link exists.
3. **Render through the seam**, with `pay_button_for(&invoice)` and the same
   `contact_email_for_preview` fallback `preview` uses. No config needed, no
   network.
4. **Write the artifacts** to `<data_dir>/previews/invoice-<number>.{html,pdf}`
   — the exact paths, permissions and `Wrote <path>` wording `preview` already
   produces, because it is the same code (extracted, see below). A build without
   `pdf` writes the HTML and prints `notice: PDF export requires the 'pdf'
   feature …` — AC #6.
5. **Print the confirmation block**, then prompt.
6. On `y`: `build_clients`, `send_invoice`, `Sent invoice #N: <url>`.
   On anything else: `Aborted.`, exit 0 — void's behaviour.

```
$ nigel invoice send 1248
Invoice #1248 — Acme Co, $1,850.00 USD, issued 2026-08-04.
Wrote /home/you/Documents/nigel/previews/invoice-1248.html
Wrote /home/you/Documents/nigel/previews/invoice-1248.pdf
Sending creates a Stripe payment link, publishes the page and PDF to
billing.example.com, and emails ap@acme.test. This cannot be undone.
Send it? [y/N]
```

The first line is `void_summary`'s shape with the recipient added, and it is
AC #5: the address the mail is going to and the amount being charged, stated
where the answer is given. The consequence sentence is the TUI's
`send_confirmation` in one paragraph, and it names the publish host from
`public_base_url` when there is one.

A resend (`published_at` set) swaps the middle sentence for the TUI's wording:
*The existing payment link is reused; the page and PDF are republished and the
client is emailed again.*

### Why the artifacts are written rather than shown

The alternatives were a rendered text form in the terminal and opening a
browser. Text is a *third* rendering of an invoice — after HTML and PDF — with
no seam behind it, so it would be the one thing that could drift; and Nigel
already has a text rendering of an invoice for the terminal in
`format_invoice_show`, which is what `nigel invoice show` is for. Opening a
browser was settled in TASK-68.2: `open` is gated behind the `serve` feature and
open-by-default cannot be taken back. Writing the same bytes `send` will publish
and printing the path is the honest middle: the operator can click the path in
any modern terminal, and re-running overwrites in place.

### `--yes` skips the prompt and the writing

Nothing is written and nothing is printed but the result. A non-interactive
caller has nobody to look at a file, and leaving artifacts behind on every
scripted send is litter. The **render still happens** — it is the same
`load_template` + seam call the send makes anyway — so a broken template is
still caught before the gateway.

Non-TTY without `--yes` is `confirm_void`'s refusal, worded for send:
`Refusing to send invoice #1248 without confirmation. Pass --yes.`

### The extraction

`cli::invoice::preview` is currently one function that renders and writes.
Split the writing half out so `send` can call it:

```rust
/// Write a rendered invoice beside itself and say where it went. The one place
/// preview paths, permissions and the `Wrote`/no-PDF wording live, so
/// `preview` and the confirmation shown by `send` cannot differ.
fn write_preview(rendered: &RenderedInvoice, number: i64, output_dir: Option<String>)
    -> Result<()>;
```

`preview()` becomes: resolve, notice, render, `write_preview`. `send()` calls
the same two lines before its prompt. No behaviour change for `preview`, and its
existing integration tests must pass untouched — that invariance is the proof.

---

## The web

### Decision 1: extract the frame, do not stretch the disclosure

`wc-invoice-preview` is a `<details><summary>Preview</summary>` wrapper around a
lazily-created `<iframe sandbox=PREVIEW_SANDBOX>`, plus two links (open the HTML,
download the PDF) and a missing-config notice. Inside a dialog whose entire
purpose is showing the document, a disclosure the operator must open first is
the wrong chrome, and forcing `open` leaves the summary row sitting in the
dialog for no reason.

New component `wc-document-frame` in `@nigel/ui`:

```ts
@property({ type: String }) srcdoc = '';
@property({ type: String }) src = '';
@property({ type: String }) label = 'Document preview';
@property({ type: String }) height = '';   // maps to --nc-document-frame-height
@state() private loaded = false;
```

It owns the iframe, `PREVIEW_SANDBOX` (moved here, re-exported from
`wc-invoice-preview` so nothing importing it breaks), the spinner overlay while
`load` has not fired, and nothing else. `wc-invoice-preview` keeps its
disclosure, its links, its notice and its `src` contract, and delegates the
frame — its six existing specs and its four preview states must pass unchanged.

This is the component-first move the repo's checklist asks for: one iframe
implementation, one sandbox constant, one loading state, its own `.preview.ts`
states (`loading`, `loaded-srcdoc`, `loaded-src`) and `describePreviewA11y`.

### Decision 2: the dialog frames HTML it was given, not a URL

`wc-invoice-preview` points its iframe at
`ApiClient.invoicePreviewUrl(number, 'html')` and lets the browser fetch it.
That works because the route sets `X-Frame-Options: SAMEORIGIN` against the
app's blanket `DENY`, and it has one flaw the dialog cannot live with: **an
iframe cannot report a failure.** A broken custom template makes that route
answer an error envelope, and the frame would render `{"error":{...}}` as text
in a box — inside the very dialog whose job is to catch exactly that before the
send (AC #7).

So the send dialog takes `srcdoc`, and the screen fetches the HTML through the
api seam:

```ts
/** The rendered invoice page as HTML, for framing before a send. */
invoicePreviewHtml(number: number): Promise<string>;
```

Implemented in `FetchApiClient` against the same `…/preview` address
`invoicePreviewUrl` spells, answering text rather than JSON and raising the
usual `ApiError` on a non-2xx so the dialog can render the server's sentence.
The address stays inside `src/api/`, which is what `api-seam.test.ts` requires,
and the fake client gains the method.

The security posture is unchanged or better: `srcdoc` content inherits the
iframe's sandbox, which still omits `allow-same-origin` and `allow-scripts`, so
the document runs in an opaque origin with no scripting and no access to the
app's cookies or storage. The route's `Content-Security-Policy: sandbox` header
does not apply to `srcdoc`, which is why the attribute — not the header — has
always been the real control (`wc-invoice-preview`'s own test asserts on the
constant for this reason).

The detail view's `wc-invoice-preview` keeps `src`. It is a browsing surface,
not a decision surface, and a lazily-loaded iframe that costs nothing until
opened is right there.

### Decision 3: the preview belongs to the confirm phase

`renderBody()` branches: `confirm` renders the consequences, everything else
renders the step trace and the outcome. The preview goes in the confirm branch
and unmounts when the send starts.

Once a send is in flight the trace is what the operator is reading, and a
40-line document above it pushes it off screen. A retry re-enters `confirm`?
No — a retry fires `nc-send-confirm` from the failed phase, so the preview does
not come back. That is right: the document did not change, and re-fetching it
after a failed publish is a request nobody asked for.

The dialog's confirm body, in order:

1. `blocked` alert, when set (unchanged).
2. **The recipient-and-amount line** — `Invoice #1251 — Acme Co,
   $1,850.00 USD, to ap@acme.test`, using `wc-money` for the figure. AC #5, and
   the same sentence the CLI prints.
3. The `This will:` consequences list (unchanged).
4. `Subject: …` (unchanged, but now actually passed — see Decision 5).
5. **The framed document**, `wc-document-frame` with the fetched HTML, a
   spinner while the fetch is in flight, and a `wc-notice-bar variant="danger"`
   carrying the server's sentence when it failed.
6. The PDF affordance: when `pdfExport` is true, a `Download the PDF` link to
   `invoicePreviewUrl(number, 'pdf')`; when false, the existing
   `PDF export is not available in this build.` line — AC #6.
7. The caveat paragraph (unchanged).

New properties on `wc-send-dialog`: `previewHtml: string`,
`previewError: string`, `previewLoading: boolean`, `pdfHref: string`,
`pdfAvailable: boolean`. Everything rich stays `attribute: false`, matching the
component's existing convention.

### Decision 4: sizing, and the one scroll box

`invoices.ts` documents why the *editor* is a full view rather than a dialog:
an invoice inside `wa-dialog` is "a scrolling box inside a scrolling page". A
framed preview risks the same, so:

- The dialog widens when it carries a preview: `wa-dialog` gets a width of
  `min(60rem, 92vw)` set from the component's own stylesheet (the theme already
  reaches `wa-dialog::part(header|body|footer)`, so styling the dialog's parts
  is established ground).
- The frame is bounded — `--nc-document-frame-height`, `24rem` inside the
  dialog — so the document scrolls inside the frame and the dialog body scrolls
  once, not twice.
- Below `48rem` the dialog is full-width and the frame drops to `16rem`.

If the review says it is still cramped, the fallback is a link out ("Open the
full page") beside the frame, which the detail view already has.

### Decision 5: what blocks a send, said before it is attempted

`sendBlockReason` today covers a client with no email and unset invoicing keys.
Two additions, both facts the server would answer with anyway:

- **No `pdf` feature** (`appStore.status.pdfExport === false`): *This build
  cannot send — PDF support is not compiled in, and the invoice PDF is attached
  to the email.* The preview still renders (AC #6); the confirm button is inert.
- **`invoicing.publicBaseUrlWarning`** from TASK-67, rendered as a warning line
  in the confirm body rather than as `blocked` — it is a caution, not a
  refusal.

And the subject line, which the component supports and the screen has never
passed: `emailSubject(number, companyName)` in `invoice-data.ts`, mirroring
`send.rs`'s rule (`Invoice #N from {company}`, or `Invoice #N` when the company
is empty), fed from `appStore.status.companyName`. It is two lines of knowingly
duplicated logic, called out here because the repo already duplicates
`SEND_STEP_LABELS` across the same boundary; the alternative — having the server
say what the subject will be — is a new field on a payload for one string.

### What the preview is not

- Not the email body's chrome. Mailgun sends the page HTML as the body, so the
  frame *is* the email body; there is no wrapper to show.
- Not the Stripe checkout page. Preview creates no link (AC #4), so a draft
  frames the inert `PayButton::Placeholder` — the documented single difference
  between a preview and what gets published.
- Not the PDF, rendered. The PDF is offered as a download; embedding a PDF
  viewer in a dialog is a second document renderer and a much larger component.

## The TUI

Out of scope. `cli/invoice_manager.rs` already confirms a send with the client,
the total and the recipient (`send_confirmation`), which is AC #5 satisfied
there, and a ratatui screen cannot render an HTML document. If a preview is
wanted from the TUI later, the honest form is a key that writes the preview
files and shows the paths — the CLI's behaviour, from the dashboard.

## Out of scope

- Any change to the send orchestration, the step vocabulary, or the failure
  mapping.
- Previewing an invoice that does not exist yet.
- A second render path of any kind. Everything shown comes from
  `render_invoice`.
- Scheduling, drafts-with-approval, or a send queue.

## Open questions for Sam

1. **`srcdoc` vs `src` in the dialog.** Recommended `srcdoc` via a new
   `invoicePreviewHtml` seam method, because it is the only way a broken
   template shows up as a sentence rather than as JSON in a box. The cost is one
   extra api method and holding the page HTML in memory. Confirm.
2. **CLI writes preview files on every interactive send.** Recommended, because
   AC #6 wants an HTML preview in a `pdf`-less build and a file is what a
   terminal can offer. The alternative is rendering in memory (still catching a
   broken template) and printing only the summary, with `--preview` to write.
3. **`--yes` skipping the write.** Recommended. Say if you would rather scripted
   sends still leave the artifacts behind.
4. **Dialog width.** `min(60rem, 92vw)` is a real departure from every other
   dialog in the app. The alternative is keeping the default width and a
   narrower frame, which will letterbox the invoice.
5. **The duplicated subject rule.** Two lines of TS mirroring `send.rs`, or a
   new field on the preview/status payload. I recommend the two lines.
</content>
