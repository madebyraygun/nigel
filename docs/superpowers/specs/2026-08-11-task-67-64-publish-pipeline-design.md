# The publish pipeline: the address a client gets, and keeping it true

Tasks: TASK-67 (high, bug) and TASK-64 (medium), stream 2 of epic TASK-86.

## Problem

Two failures of the same pipeline, both found against the real
`billing.example.com` bucket during pre-merge testing of PR #172.

**The address 404s.** `invoicing::r2::public_url` builds
`{public_base_url}/{token}/`, and that is what `nigel invoice send` prints, what
`SendOutcome.public_url` carries, and what `GET /api/invoices/{number}` reports
as `publicUrl`. A plain R2 custom domain serves objects by key and has no
directory-index behaviour, so `…/{token}/` is a 404 while
`…/{token}/index.html` — the key `AssetPublisher::publish` actually wrote — is
the document. Every link Nigel has ever emitted for a bucket without an edge
rewrite is dead.

**A wrong `public_base_url` is accepted silently.** `billing.example.com` — no
scheme, no `/i` prefix — travels through `require()` in
`cli::invoice::build_clients` (which only checks that the key is *set*), into
`R2Publisher`, and out the other end as `billing.example.com/aBc123.../`, a relative
address in a client's inbox. Nothing in the send path looks at the value.

**And the page goes stale.** The page and the PDF are rendered once, at send.
When a payment lands — `nigel invoice pay`, the launch sync, `nigel invoice
sync`, `POST /api/invoices/{number}/pay`, the TUI's `p` — nothing touches R2, so
a client following their bookmark sees the invoice they already settled, still
offering a Pay button.

## Where the code is today

| Thing | Location |
|---|---|
| The address, built in one place | `src/invoicing/r2.rs` — `public_url(public_base_url, token)` |
| The keys written | `src/invoicing/r2.rs` — `object_key(token, "index.html" \| "invoice.pdf")` |
| The upload | `AssetPublisher::publish` (both artifacts) and `publish_page` (HTML only) in `src/invoicing/gateway.rs` |
| Config resolution, all nine keys | `src/settings.rs` — `invoicing_config`, `invoicing_status` |
| The send-path constructors | `src/cli/invoice.rs` — `build_clients` (all nine required), `optional_publisher` / `optional_gateway` (whatever is set) |
| The step vocabulary a config failure belongs to | `src/invoicing/send.rs` — `SendStep::Config` |
| Best-effort teardown, the pattern to copy | `src/invoicing/void.rs` — `TeardownStep`, `VoidOutcome::warnings()` |
| Payment recording | `src/invoicing/invoices.rs` — `record_payment` (also called by `sync::sync_invoice`) |
| Sync | `src/invoicing/sync.rs` — `sync_invoice`, `run_sync`, `SyncReport` |
| Launch sync | `src/main.rs` — `sync_invoice_payments()` |
| API | `src/server/routes/invoices.rs` — `pay`, `sync`, `void`/`VoidResult`, `public_url(&invoice)` |

Two facts worth stating before the decisions, because the task text implies
otherwise:

- **The URL is not in the email.** `MailgunClient::send_invoice` posts
  `from/to/subject/html` plus the PDF attachment; the HTML is the invoice page
  itself and carries no link to its own address, and no Stripe object records
  it either. The address is *printed* by the CLI, *returned* by the API
  (`SendResult.publicUrl`, `InvoiceDetail.publicUrl`), and *shown* by the TUI
  and the SPA. Fixing `r2::public_url` fixes every one of them; there is no
  second place to chase.
- **`optional_publisher` is used by void today**, and void never reads the URL
  it gets back — the teardown only needs the upload to happen.

---

# TASK-67 — the address, and the setting behind it

## Decision 1: emit the full `index.html` URL

`r2::public_url` becomes:

```rust
/// The object every published page is written to. The address Nigel hands out
/// names this file rather than its directory, because a static host is not
/// required to have an opinion about directories.
pub const PAGE_OBJECT: &str = "index.html";

pub fn public_url(public_base_url: &str, token: &str) -> String {
    format!(
        "{}/{}/{PAGE_OBJECT}",
        public_base_url.trim_end_matches('/'),
        token
    )
}
```

`https://billing.example.com/i/aBc123.../index.html`.

**Why this and not the documented rewrite.** The rewrite works — a Cloudflare
transform rule or a Worker appending `index.html` makes the directory form
resolve — but it moves the correctness of a link Nigel emails into a
configuration surface Nigel cannot see, cannot test, and cannot mention in an
error message. The failure mode is the one we just had: everything reports
success, the operator gets a URL that looks right, and the client gets a 404
some days later. The file URL resolves on a bare R2 custom domain, on S3 static
hosting, on a Worker, behind a rewrite (the object is at the same key either
way), and on `python -m http.server` pointed at a synced copy. It is the only
form that is correct without asking anything of the host.

The cost is four characters of ugliness in an address that already carries a
16-character random token, is clicked rather than typed, and is never read
aloud.

**Rejected: a `public_url_style` setting** (`directory` | `file`). A setting
whose wrong value produces a 404 in a client's inbox is the bug this task
exists to remove, and defaulting it merely relocates the decision.

**The rewrite stays documented, as an option rather than a requirement.**
`docs/invoicing.md`'s hosting section currently claims the object "is served at
`https://billing.example.com/i/{token}/`". That sentence becomes: the object is
served at `…/i/{token}/index.html`, which is what Nigel links to; if you would
rather hand out the directory form, add an edge rewrite — and both addresses
then work.

**Blast radius.** One function. Its two unit tests change; `send.rs`'s tests
assert `starts_with("https://billing.example.com/i/")` and stay green;
`server::routes::invoices::public_url` and `InvoiceDetail.publicUrl` inherit the
new form; the SPA's `hostOf(detail.publicUrl)` still parses. The committed
invoicing fixtures are captured under `TempConfigDir` with no
`public_base_url`, so `publicUrl` is `null` in all of them and nothing needs
recapturing.

## Decision 2: validate the setting at send time, in two strengths

Two pure functions beside `public_url`, in `src/invoicing/r2.rs`:

```rust
/// A `public_base_url` that cannot produce a working link at all. An absolute
/// http(s) address with a host is the whole requirement — the path is the
/// operator's business, and the `/i` question is a warning, not this.
pub fn validate_public_base_url(value: &str) -> Result<()>;

/// Nigel writes every object under `i/`, so a base URL that does not end there
/// is usually pointing at the bucket root. Usually, not always: a rewrite can
/// map the prefix onto the domain root, which is why this is a sentence and not
/// a refusal.
pub fn public_base_url_warning(value: &str) -> Option<&'static str>;
```

`validate_public_base_url` refuses, naming the setting and showing the shape:

```
public_base_url "billing.example.com" is not an absolute http(s) address.
Set it to the address your bucket is served at, including the scheme —
for example https://billing.example.com/i.
```

Refused: an empty or whitespace-only value; anything not beginning
`http://`/`https://` (ASCII case-insensitive); a value with no host between the
scheme and the first `/`; any value containing whitespace. That is a
hand-rolled check rather than a parser — `url` is not a direct dependency and
pulling one in to answer "does this start with https:// and have a host" would
be the largest thing in this task.

`public_base_url_warning` answers `Some(...)` when the path, with trailing
slashes trimmed, does not end in `/i`:

```
public_base_url does not end in /i — Nigel writes objects under the i/ prefix,
so published links will 404 unless that prefix is what this address serves.
```

**The warning quotes no value; the error does.** `settings::invoicing_status`
carries key names only and has a test asserting no configured value ever
appears in it (`the_invoicing_status_never_carries_a_value`); the warning is
going to live there, so it stays valueless. The hard error is answered to the
operator who typed the command — on a localhost-only server, about a public
address that is not a secret — where quoting the offending value is the
difference between fixing settings.json and fixing an env var.

### Where each one is called

**The refusal goes in `cli::invoice::build_clients`**, immediately before the
`R2Publisher` is constructed. That is the single constructor both front ends
use (`cli::invoice::send` and `server::routes::invoices::send`), it already
owns the "missing invoicing config" refusals, and it runs before any client
exists — so nothing is published, nothing is emailed, and no Stripe link is
created. On the API this lands as the existing `NigelError::Invalid` → 400/409
mapping out of `crate::cli::invoice::build_clients(config)?`; the handler
should tag it `SendStep::Config` the way every other config failure is tagged,
so the dialog files it under the same step.

**`optional_publisher` stays lenient.** It is void's constructor and TASK-64's:
both only need the upload to happen, and neither reads the address. A void
refused because the base URL has no scheme would leave a live payment link up
to protect the formatting of a URL it never prints.

**The warning is computed once, in `settings::invoicing_status`**, as a new
field:

```rust
pub struct InvoicingStatus {
    pub send_configured: bool,
    pub sync_configured: bool,
    pub missing: Vec<&'static str>,
    /// A configured `public_base_url` that is probably pointing at the wrong
    /// prefix. Absent when it is unset (that is `missing`'s job) or fine.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_base_url_warning: Option<&'static str>,
}
```

- `nigel invoice send` prints it to stderr as `notice: <sentence>` before it
  does anything, alongside the existing notice style.
- `GET /api/status` carries it inside the `invoicing` block it already
  serializes — the "facts about this installation" surface, and the one the SPA
  already reads for `missing`.
- The SPA renders it in the send dialog. **That display lands in TASK-79**,
  which is rebuilding that dialog anyway; adding a notice line to the current
  dialog and then rewriting it one PR later is work done twice. Until then the
  field is carried and unused by the browser, which is harmless — TS ignores
  fields it does not name.

### Why not validate when the setting is written

There is no settings screen for the nine invoicing keys (`settings.json` and
`NIGEL_*` env vars are the whole interface), so "at write time" has no hook. At
send time is also where the task asks for it, and it is the only moment we know
the value is about to be used.

## What each surface says after TASK-67

| Surface | Before | After |
|---|---|---|
| `nigel invoice send 1248` | `Sent invoice #1248: https://billing.example.com/i/aBc.../` | `Sent invoice #1248: https://billing.example.com/i/aBc.../index.html` |
| `nigel invoice send` with `public_base_url=billing.example.com` | publishes, emails, prints a broken link | refuses at `config`, naming the setting; nothing published |
| `nigel invoice send` with `…/` (no `/i`) | silent | `notice:` before the send; the send proceeds |
| `GET /api/status` | `invoicing.missing` | plus `invoicing.publicBaseUrlWarning` when applicable |
| `POST …/send` misconfigured | 502 from R2, or a bad link | 400 at `step: "config"` naming `public_base_url` |
| `InvoiceDetail.publicUrl` | directory form | file form |

---

# TASK-64 — republish when a payment lands

## Decision 3: it is best-effort, and it copies void's vocabulary

`void.rs` already solved this exact shape: a database write that commits first,
followed by network work that must not be able to undo it, reported as data
with the sentences in one place. TASK-64 is the same problem with a different
verb, so it gets the same structure rather than a new one.

New module `src/invoicing/republish.rs`:

```rust
/// How a republish went. `NotApplicable` is the ordinary case: most payments
/// land on invoices that were never published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Republished {
    /// Never published, or void — there is no live page to correct.
    NotApplicable,
    /// There is a live page and no publisher configured to reach it.
    Skipped,
    /// The page was rewritten. `pdf` says whether the attachment beside it was
    /// rewritten too — a build without the `pdf` feature can only do the page.
    Done { pdf: bool },
    /// The upstream's own message, kept verbatim.
    Failed(String),
}

pub struct RepublishOutcome {
    pub number: i64,
    pub page: Republished,
}

impl RepublishOutcome {
    /// The sentences every front end prints, in one place. Empty when nothing
    /// needs saying, which is the ordinary case.
    pub fn warnings(&self) -> Vec<String>;
}

/// Re-render a published invoice and put it back where it was.
///
/// Infallible by construction: the payment is already recorded and committed,
/// and nothing out here may read as a failed payment. Every way this can go
/// wrong becomes a `Republished` variant and a sentence.
pub fn republish_invoice<P: AssetPublisher>(
    conn: &Connection,
    invoice: &Invoice,
    client: &Client,
    branding: &Branding<'_>,
    publisher: Option<&P>,
) -> RepublishOutcome;
```

The warnings, matching `VoidOutcome::warnings()`'s register:

- `Skipped` → `Warning: invoice #1248 was paid but the R2 publisher is not configured, so its published page still shows the old balance.`
- `Failed(msg)` → `Warning: could not republish invoice #1248's page (r2 403: …). It still shows the old balance.`
- `NotApplicable` / `Done` → nothing.

**Both artifacts, or just the page.** With the `pdf` feature the outcome of the
render seam is an HTML+PDF pair and `AssetPublisher::publish` writes both, so
the attachment a client saved and the page they bookmarked agree. Without the
feature the seam answers `pdf: None`, and rather than refusing, republish falls
back to `publish_page` — the page is corrected, the PDF beside it is left as
the document that was actually emailed, which is exactly the rule void already
follows. `Done { pdf: false }` records which happened; no warning, because
nothing is wrong.

**A void invoice is `NotApplicable`.** Void has already replaced the page with
its own notice, and `record_payment` refuses a void invoice anyway
(`ensure_not_void`), so this branch is defence rather than a live path.

## Decision 4: the Pay button moves below the seam, and a settled invoice does not get one

`cli::invoice::pay_button_for(invoice)` — void → `Omitted`, a link → `Link`,
otherwise → `Placeholder` — lives in the CLI layer and is used by `preview` and
by the API's preview routes. `send.rs` has its own inline two-arm match, and
republish would be a third copy.

Move it into `src/invoicing/render.rs`, beside the seam, and give it the third
rule the republished page needs:

```rust
/// Which pay element an invoice renders, wherever it is rendered.
///
/// Void and paid-in-full both omit it: an invoice that is settled or cancelled
/// must not offer a working payment link, and a republished page is exactly the
/// moment that becomes reachable.
pub fn pay_button_for(invoice: &Invoice) -> PayButton<'_>;
```

`send` then uses it too, which changes one live behaviour: re-sending an invoice
that is already paid in full publishes a page with no Pay button. That is
correct and was previously an accident of nobody trying it.

`cli::invoice::pay_button_for` becomes a re-export so the API preview routes and
`cli/invoice.rs` need no edits beyond the import.

## Decision 5: TASK-64 needs TASK-78 to have anything to say

The acceptance criterion is that the republished page "reflects paid amount,
balance, and status". **Today's page renders none of those** — the stock
template prints line items, a total, the Pay button, notes, terms and the
direct-deposit line; the PDF prints subtotal/tax (when tax is non-zero), total,
notes and terms. Re-rendering the current documents after a payment produces
bytes identical to what is already up there, apart from the Pay button
disappearing on the final payment.

TASK-78 is the task that gives both documents a money block. Its spec
(`2026-08-11-task-78-document-parity-design.md`) therefore owns the
`Paid` / `Balance` rows and the shared `MoneySummary`, and the render seam
loads `paid_amount` itself so **every** caller — preview, send, republish, the
API preview routes — shows the same figures with no signature change.

**So the recommended PR order is 67 → 78 → 64 → 79, not 67+64 → 78 → 79.** See
"PR sequencing" below.

## Decision 6: one hook per front end, two helpers

`src/invoicing/` may not read settings or load a template, so the branding
(template, company name, contact address) is resolved by the caller. To keep
six call sites from each growing their own version of that, the CLI layer gets
two helpers in `src/cli/invoice.rs`:

```rust
/// Republish one invoice's published page after a payment. Returns the
/// sentences to print; never fails. A broken custom template, an unreadable
/// data directory and an R2 outage are all warnings here — the payment is
/// recorded and nothing may take that back.
pub(crate) fn republish_after_payment(conn: &Connection, invoice_id: i64) -> Vec<String>;

/// The same, for every invoice a sync recorded a payment against.
pub(crate) fn republish_all(conn: &Connection, numbers: &[i64]) -> Vec<String>;
```

`republish_after_payment` resolves `invoicing_config()`, `optional_publisher`,
`load_template(&get_data_dir())`, `company_name(conn)` and `from_email` (the
same `contact_email_for_preview` fallback preview uses — a republish must not
depend on the address being set), then calls `republish_invoice`. A failure
resolving any of it is a warning sentence, not an error. The server already
calls `cli::invoice::optional_publisher` and `company_name` from its routes, so
a route reaching these is in keeping.

### The call sites

| Caller | Change |
|---|---|
| `cli::invoice::pay` | after `record_payment`, print each warning |
| `cli::invoice::sync` | after `sync_all_report`, `republish_all(&report.recorded_invoices)`; print |
| `main::sync_invoice_payments` | same, as `notice:` lines beside the existing ones |
| `cli/invoice_manager.rs` (TUI `p`) | warnings onto the result screen, beside void's |
| `server::routes::invoices::pay` | into the response (below) |
| `server::routes::invoices::sync` | into `SyncReport` (below) |

### `SyncReport` gains the invoices it touched

`run_sync` is where a payment is recorded during a sync, and it is inside
`src/invoicing/`, where no template can be loaded. Rather than push branding
down into sync, the report says which invoices moved and the caller republishes:

```rust
pub struct SyncReport {
    pub recorded: u32,
    pub invoices_checked: u32,
    pub failures: Vec<SyncFailure>,
    /// Invoice numbers a new payment was recorded against, in the order the run
    /// found them. What the front end republishes, and worth showing besides.
    pub recorded_invoices: Vec<i64>,
}
```

Numbers rather than ids, because it crosses the wire and a number is what a
person reads. `sync_invoice` already answers how many payments it recorded, so
the loop in `run_sync` pushes the number when that count is non-zero.

### The API shapes

`pay` currently answers a bare `InvoiceDetail`. It becomes `VoidResult`'s
shape, for the same reason void has it — a best-effort teardown that failed is
a 200 carrying a correct invoice plus something a human has to do:

```rust
#[derive(Serialize)] #[serde(rename_all = "camelCase")]
struct PayResult {
    #[serde(flatten)]
    invoice: InvoiceDetail,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    republish_warnings: Vec<String>,
}
```

Flattened, so a client reading `.status` or `.balance` off the pay response
keeps working and the SPA change is additive. `SyncReport` carries its
warnings the same way (`republishWarnings`), because a browser cannot read the
server's stderr — the rule `sync`'s per-invoice failures already follow.

**`POST /api/invoices/{number}/pay` now reaches the network.** It joins `send`
and `void` as a blocking request bounded by `invoicing::REQUEST_TIMEOUT` (two
uploads, ~60s worst case) and holds `db_gate` for that long. Worth documenting
in `docs/api.md`; not worth queueing, for the reasons `send`'s doc comment
already gives.

The SPA renders `republishWarnings` through the channel it already has for
void's `teardownWarnings` — warning notices above the invoice, dismissable
individually (`screens/invoices.ts`).

## Out of scope

- Republishing when an invoice is *edited*. Edit is draft-only, so a published
  invoice cannot be edited; nothing to do.
- Republishing after a client's name or address changes. The published page is
  a document as sent; `docs/invoicing.md` already says editing a client affects
  the next send.
- A webhook. Sync stays pull-based.
- Any change to `object_key` or the bucket layout.
- Backfilling: existing published pages are not rewritten by an upgrade. The
  next payment on each one corrects it.

## Open questions for Sam

1. **The file URL.** Recommended above: `…/{token}/index.html` everywhere, with
   the edge rewrite documented as an optional way to keep the pretty form. Say
   the word if you would rather keep `…/{token}/` and make the Cloudflare rule a
   required setup step — it is a doc change plus one warning, and the rest of
   this spec is unaffected.
2. **PR order.** Recommended: 67 alone, then 78, then 64, then 79 — because 64's
   "the page reflects the payment" is TASK-78's document work. Confirm, or say
   you want 64 to carry a minimal paid/balance block of its own that 78 then
   rewrites.
3. **`pay` becoming a network call.** It is what makes AC #1 true from the
   browser, but it turns an instant write into a request that can take a minute
   on a bad day. The alternative is republishing only from the CLI and the
   launch sync, and leaving the browser's page stale until the next sync. I
   recommend doing it; confirm you want the latency.
4. **Warning wording.** `Warning: invoice #1248 was paid but the R2 publisher is
   not configured, so its published page still shows the old balance.` — one
   sentence per outcome, matching void. Change the words now if you want
   different ones; they land in three front ends at once.
5. **`recorded_invoices` on `SyncReport`.** Added for the republish loop, but it
   is also the only way a browser could say *which* invoices a sync moved.
   Should the SPA show that, or is the count enough?
</content>
</invoke>
