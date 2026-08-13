# The invoice page and PDF, laid out like the house invoice

Task: TASK-101 (high). Lands **after** PR #204 (`feat/document-parity`) and
PR #206 (`feat/republish-on-payment`) merge — see Decision 0.

## Problem

Nigel's invoice is a heading, a paragraph and a table. The invoice this business
already sends is a letterhead: a logo top-left, a rule-set "From" block
top-right, a two-column band of invoice metadata and "Invoice For", a ruled
item table with column dividers and right-aligned figures, a right-aligned
Amount Due, and Notes under a full-width rule at the foot.

The gap is layout, not data. What the reference prints that Nigel's schema
already holds: invoice number, issue date, due date, terms, line items with
quantity and unit amount, notes, client name, client billing address, company
name. What it does not hold: a logo, a company address, a company phone, a
per-invoice subject, and a per-line item type.

Changing the page changes what a client receives in their inbox: `send.rs:249`
hands `rendered.html` — the same string uploaded to R2 — straight to
`Mailer::send_invoice`. **There is no email template.** There are two documents,
not three, and the page is one and a half of them.

## Where the code is today

### This design sits on top of PR #204, which is open and unmerged

#204 rewrites both renderers. Everything below extends its seams; nothing here
re-decides anything it decided.

| #204 seam | What it is | How TASK-101 extends it |
|---|---|---|
| `src/invoicing/document.rs` | The module that decides, once, what both documents say — `MoneySummary`/`MoneyLine`, `address_lines`, `email_line`, `MAX_ADDRESS_LINES`, `ADDRESS_TRUNCATED` | Gains `CompanyBlock`, `MetaRow`/`meta_rows`, `due_value`, `parse_logo`. Same rule: the decision lives above both renderers |
| `document::address_lines` | Splits a billing address into drawable lines, blank ones dropped, clamped at 6 with `...` | Reused verbatim for the **company** address. One clamp, one truncation marker, both parties |
| `document::MoneySummary::lines()` | Which money rows exist and which are emphasised | Untouched. The Amount Due block renders `lines()`; only its CSS/geometry changes |
| Optional fragment placeholders | `COMPANY_BLOCK`, `CLIENT_ADDRESS_BLOCK`, `CLIENT_EMAIL_BLOCK`, `TOTALS` — added to `PLACEHOLDERS`, never to `REQUIRED` | The pattern for four more: `LOGO`, `META_ROWS`, `TERMS_BLOCK`, plus `COMPANY_ADDRESS`/`COMPANY_PHONE` text keys |
| `REQUIRED_ALTERNATIVES` | `("TOTAL", "TOTALS")` — a required key another key stands in for | Untouched, and the reason `REQUIRED` still does not need to grow |
| `Branding<'a>` | `{ template, company, contact_email }`, all `&'a str`, resolved by the CLI/server layer because `src/invoicing/` reads no settings and no database | Grows `company_address`, `company_phone`, `logo`, `page_url`. Same convention: borrowed `&str`, empty means unset |
| `render_invoice_pdf(invoice, client, items, company, summary)` | The PDF, drawing the company name bold under the number, then the client block | Takes the whole company block and the logo/page-url decisions instead of a bare `company: &str` |
| `pdf.rs`'s `no_live_payment_link_reaches_the_pdf` test | Pins Sam's ruling on #204 | Stays, and Decision 5 makes it permanent |

`pay_button_for` (moved into `render.rs` by **#206**) omits the Pay button for a
void or settled invoice, and `republish.rs` rewrites the published page whenever
a payment lands. That is load-bearing for Decision 5.

### Everything else this touches

| Thing | Location |
|---|---|
| Placeholder vocabulary, validation, expansion | `src/invoicing/render_html.rs` — `PLACEHOLDERS`, `REQUIRED`, `REQUIRED_ALTERNATIVES`, `validate_template`, `expand`, `esc` |
| The stock page | `src/invoicing/templates/invoice.html` (17 lines) |
| The PDF writer | `src/pdf.rs` — `PdfWriter` (`text`, `hline`, `table_header`, `table_row`, `table_row_wrapped`, `section_label`, `separator`), `Col`/`Align`, `approx_text_width` |
| The one render seam | `src/invoicing/render.rs` — `render_invoice(conn, invoice, client, pay, branding)` |
| `company_name` storage | `metadata` table; `db::get_metadata`/`db::set_metadata` (`src/db.rs:771`, `:778`); the one helper is `cli::invoice::company_name` (`src/cli/invoice.rs:101`) |
| The legacy migration out of settings.json | `settings::migrate_company_name` (`src/settings.rs:218`), called once from `cli/dashboard.rs:1188` |
| `Branding` construction — five production sites | `cli/invoice.rs:490` (`preview`), `cli/invoice.rs:544` (`send`), `cli/invoice_manager.rs:1129`, `server/routes/invoices.rs:688` (`send_with`), `server/routes/invoices.rs:806` (`render`, the preview routes) |
| TUI settings | `src/cli/settings_manager.rs` — `MENU_BUSINESS_NAME`/`MENU_PASSWORD`/`MENU_UPDATE_CHECK`/`MENU_LAST` (`:29-32`), `Screen::{Main, EditingName, Password}` (`:22-26`), `draw_main` (`:114`), `handle_main_key` (`:251`), `handle_edit_name_key` (`:302`, the `set_metadata` call at `:310`) |
| Web settings | `src/server/routes/settings.rs:39` route, `CompanyNameRequest`/`CompanyNameResponse` (`:99-109`), `put_company_name` (`:111`); SPA `web/apps/app/src/screens/settings.ts:279-301`, `api/client.ts:313`/`:444`, `api/types.ts:350-357` |
| The multi-line address precedent in a TUI form | `src/cli/client_manager.rs:79` — `ClientForm` labels `["Name", "Email", "Address", "Notes"]`, one single-line buffer per field |
| Published address | `src/invoicing/r2.rs` — `public_url(base, token)` → `{base}/{token}/index.html` |

## Design

### Decision 0: this lands after #204 and #206 merge, not beside them

Confirmed, and not a preference. #204 rewrites `render_html.rs` (+463 lines),
`pdf.rs` (+223) and `templates/invoice.html` and creates `document.rs`; #206
rewrites `render.rs` and `cli/invoice.rs` (+219) and adds `republish.rs`.
TASK-101 rewrites the same four files again, and its central claim — that the
two documents agree because one module decides — is only true if it is built on
`document.rs` rather than racing it. A parallel branch would be a three-way
merge across every line of both renderers, resolved by whoever went second.

#207 (`feat/send-preview`) is stacked on #206 and touches `cli/invoice.rs`,
`server/routes/invoices.rs` and the SPA. It does not touch the renderers, so it
is not a hard prerequisite — but its `wc-document-frame` is what Sam will
review the new layout in, so landing it first is worth it.

**Order: #204 → #206 → #207 → TASK-101.**

### Decision 1: the house layout, block by block, in both documents

Neither document copies the reference. Both carry the same seven blocks in the
same order and the same hierarchy, which is what the brief asks for.

```
┌─ logo / wordmark ─────────────────┐   From │ Company Name          (bold)
│                                   │        │ address line 1
│                                   │        │ address line 2
│                                   │        │ ph. 619.555.0123
└───────────────────────────────────┘

Invoice ID    1248        (bold)  Invoice For │ Client Name       (bold)
Issue Date    2026-08-06                      │ address line 1
Due Date      2026-09-05 (Net 30)             │ address line 2
                                              │ ap@client.test

┌──────────────┬──────────┬────────────┬────────┐
│ Description  │ Quantity │ Unit Price │ Amount │   ← ruled header,
├──────────────┼──────────┼────────────┼────────┤     column dividers,
│ …            │    10.50 │    $150.00 │ $1,575 │     figures right-aligned
└──────────────┴──────────┴────────────┴────────┘

                                       Amount Due   (from MoneySummary::lines)
                                        $1,575.00
                              [Pay online]            ← page only, Decision 5

──────────────────────────────────────────────────  ← full-width rule
Notes
…
```

Two labels change on **both** documents, which also fixes a parity bug #204 left
behind: the page's item table says `Description / Qty / Unit / Amount` and the
PDF's says `Description / Qty / Rate / Amount`. Both become
**`Description / Quantity / Unit Price / Amount`** — the reference's wording,
and now one wording. The three figure columns are right-aligned on the page as
they already are in the PDF (`Align::Right` in `render_invoice_pdf`'s `cols`);
the stock stylesheet's blanket `td,th{text-align:left}` is what makes the page
disagree today.

The Amount Due block is still `document::MoneySummary::lines()` — #204's
decision, unchanged. It moves visually from a plain `<tfoot>` to a right-aligned
block whose emphasised row is set larger, and the PDF draws the same rows
right-aligned against the Amount column's right edge instead of at
`MARGIN_LEFT`. No line appears or disappears.

### Decision 2 (obstacle A): the logo is on the page; the PDF draws a wordmark

**Recommendation: keep the recorded decision, with the cost now measured rather
than asserted.**

I investigated rather than inheriting. Findings, all reproduced against the
version actually in the lockfile:

1. **Version.** `printpdf 0.7.0` (`Cargo.lock:1834-1836`), optional behind
   Nigel's `pdf` feature (`Cargo.toml:10`, `:32`).
2. **The nine-crate claim is right in shape.** `embedded_images = ["image"]`,
   and printpdf's manifest pins `image 0.24.3` with
   `features = ["gif", "jpeg", "png", "pnm", "tiff", "bmp"]` — hard-enabled, so
   PNG alone is genuinely not on offer. `cargo tree` on printpdf 0.7 with and
   without the feature adds **11 packages** to the subtree, of which **8 are new
   to Nigel's `Cargo.lock`**: `bytemuck`, `color_quant`, `fdeflate`, `gif`,
   `image`, `jpeg-decoder`, `png`, `tiff`. (`bitflags`, `byteorder`,
   `num-traits` are already there.)
3. **Binary cost: ~984 KiB.** An isolated release binary linking printpdf,
   built twice, grew from 1,934,544 to 2,942,504 bytes — 1,007,960 bytes. On a
   binary the updater downloads (`cli/update.rs`) and CI notarizes per platform.
4. **The soft-mask defect is real, and I reproduced it.**
   `impl From<ImageXObject> for lopdf::Stream`
   (`printpdf-0.7.0/src/xobject.rs:263-278`) builds the mask sub-image with
   `height: img.width`. A 400×60 RGBA PNG emits
   `/Width 400 /Height 60 … /SMask << /Width 400 /Height 400 … /Length 45 >>` —
   a mask declaring 160,000 samples with 24,000 bytes of alpha behind it. A wide
   wordmark embeds wrong, exactly as CLAUDE.md records.
5. **New: the defect is confined to alpha.** `preprocess_image_with_alpha`
   (`xobject.rs:637-651`) returns `Some(SMask)` only for `ColorType::Rgba8`;
   every other colour type yields `None` and the writer emits `/SMask null`.
   The same 400×60 PNG saved **without** alpha embeds correctly. "Restrict PDF
   logos to opaque images" is therefore technically sound, and Nigel could go
   further and composite RGBA onto white itself before handing printpdf an
   `Rgb8` image — about ten lines, and the defect is unreachable.

I still recommend against putting a logo in the PDF now:

- ~1 MB and 8 crates is a permanent cost paid by every user on every platform,
  for a block the reference fills with a lowercase wordmark.
- The failure mode of the "opaque only" variant is a **silently wrong document
  in a client's inbox**, and an operator's logo is a transparent PNG essentially
  every time. Flattening it for them means Nigel picking a background colour on
  behalf of someone's brand asset.
- The PDF's alternative is close. The reference's logo *is* a wordmark; the
  company name in `HelveticaBold` at 22pt in the same top-left position is the
  same block, in the same place, saying the same thing.
- Reversal stays cheap, and this design leaves the seam pointing at it:
  `Branding::logo` already carries the bytes to both renderers, and
  `document::parse_logo` already validates and decodes them. The PDF half would
  be one `cfg`'d branch drawing an image where it now draws text.

**So: `{{LOGO}}` on the page, `company` as a 22pt bold wordmark in the PDF.**
When no logo is configured, the page draws the same wordmark (`{{LOGO}}` empty,
`{{COMPANY_BLOCK}}`'s name doing the work) and the two documents look alike.

#### How a logo is configured

**One metadata key, `company_logo`, holding a `data:` URI, beside
`company_name`.** Not a filesystem path, and not an uploaded R2 object.

The three surfaces have to converge, and they only do at one form:

| Surface | Filesystem? | Needs |
|---|---|---|
| `nigel invoice preview` / `send` | yes | must work offline — preview is defined to make no network call |
| `nigel serve` preview routes (`server/routes/invoices.rs:806`) | yes (same machine) | must render in the SPA's `wc-document-frame` iframe |
| The published page (`i/{token}/index.html`) | **no** — a static R2 object | must be self-contained or fetch over https |
| The email body — *the same string* | n/a | whatever mail clients will render |

`AssetPublisher` uploads `index.html` and `invoice.pdf` and nothing else. A
third object means a third upload, a third `SendStep`, a third failure mode in
the send trace, and a republish/void story for it. A path in metadata dangles
when the file moves and is not carried by a database backup. A data URI in
metadata is reached by one `get_metadata` call from all three surfaces, is
carried by backup/restore, is encrypted with an encrypted database, is
per-database (so the `personal` profile has no letterhead), and needs no change
to the publisher at all.

**Honest cost: Gmail does not render `data:` URIs in `<img src>`.** Since the
email body *is* the page, there is one HTML and therefore one logo form; a
hosted `https://` image would render in Gmail but would break `invoice preview`
(which would reference an object that does not exist), add the upload step
above, and put a third artifact in the publish story. The chosen degradation is
that Gmail readers see the `alt` text — the company name — which is precisely
the PDF's treatment, so the two documents still agree. An operator who wants a
hosted image in the email body has the escape hatch that already exists:
`{{LOGO}}` is optional, and their own `<data_dir>/templates/invoice.html` can
carry an absolute `<img src="https://…">` instead.

Shape and validation, all pure and all in `document.rs`:

- Stored value: `data:image/png;base64,<payload>` or `data:image/jpeg;base64,…`.
- **PNG and JPEG only.** SVG is unrendered by most mail clients and could never
  follow into the PDF if Decision 2 is ever reversed, so allowing it would buy a
  validation branch and nothing else.
- `parse_logo(&str) -> Result<Logo>` checks the prefix, the MIME against the
  allow-list, that the payload base64-decodes, that the decoded bytes carry the
  right magic (`\x89PNG\r\n\x1a\n` / `\xFF\xD8\xFF`), and that they are at most
  **128 KiB** — every byte is base64-inflated by a third into every email body
  and every published object.
- Input is a **file path** in the TUI and the data URI itself over the API (the
  SPA does the `FileReader` work); both go through `parse_logo` before
  `set_metadata`, so a bad file is refused at the settings screen and never at
  send time.
- This needs `base64` (0.22, no transitive dependencies) in `Cargo.toml`. Verify
  with `cargo tree` at implementation time; if it is not dependency-free,
  say so rather than pulling a tree in quietly.

### Decision 3 (obstacle B): address and phone are metadata keys, resolved once

**`company_address` and `company_phone` join `company_name` in the `metadata`
table.** Not a tenth invoicing key in `settings.json`, for four reasons:

1. `company_name` was deliberately *moved out* of `settings.json` into metadata
   — `settings::migrate_company_name` (`src/settings.rs:218`) exists only to
   finish that move. A sibling belongs where the sibling lives.
2. Every existing invoicing key in `Settings` (`stripe_secret_key` …
   `public_base_url`) is a credential or an endpoint, resolved through
   `invoicing_config_from` with `NIGEL_*` environment overrides. A postal
   address is neither a secret nor an override.
3. `settings.json` is per-machine; metadata is per-database. A `business` and a
   `personal` database on one machine have different letterhead, or none.
4. Metadata travels with a backup and is encrypted with an encrypted database.

`company_address` is multi-line, exactly like `clients.billing_address`, and is
split by the **same** `document::address_lines` — one clamp at
`MAX_ADDRESS_LINES`, one `ADDRESS_TRUNCATED` marker, both parties.

#### They reach the renderers through `Branding`, resolved in one place

`Branding` is the carrier and stays it. Today five production sites each call
`company_name(conn)` and build the struct literal by hand; adding three fields
to five hand-built literals is how they drift. So:

```rust
// src/cli/invoice.rs — owned, because Branding borrows.
pub(crate) struct CompanyProfile { pub name: String, pub address: String, pub phone: String, pub logo: String }
pub(crate) fn company_profile(conn: &Connection) -> CompanyProfile;   // four get_metadata reads

// src/invoicing/render_html.rs
pub struct Branding<'a> {
    pub template: &'a str,
    pub company: &'a str,
    pub company_address: &'a str,   // empty means unset, as `company` already does
    pub company_phone: &'a str,
    pub logo: &'a str,              // the data URI, empty means none
    pub page_url: &'a str,          // Decision 5; empty when public_base_url is unset
    pub contact_email: &'a str,
}
```

`cli::invoice::company_name` stays, unchanged, for the nine report exporters
(`cli/export.rs`), the text reports (`cli/report/text.rs`) and `/api/status` —
none of which want a letterhead.

Both renderers draw the From block from one shared decision, the way they
already draw the money block from `MoneySummary::lines()`:

```rust
// src/invoicing/document.rs
pub struct CompanyBlock<'a> { pub name: &'a str, pub address: Vec<&'a str>, pub phone: Option<&'a str> }
pub fn company_block<'a>(name: &'a str, address: &'a str, phone: &'a str) -> CompanyBlock<'a>;
```

`address` through `address_lines`, `phone` through the same trim-or-`None` rule
as `email_line`. `src/pdf.rs` already imports from `document.rs`, so the PDF
takes `&CompanyBlock` in place of `company: &str` and does not learn about
`Branding`.

#### Editing surfaces

**TUI (`cli/settings_manager.rs`).** `MENU_BUSINESS_NAME` gains
`MENU_COMPANY_ADDRESS`, `MENU_COMPANY_PHONE` and `MENU_COMPANY_LOGO` before
`MENU_PASSWORD`; `MENU_LAST` moves. `Screen::EditingName` carries no
discriminator today, so it becomes `Screen::Editing(usize)` keyed by the
`MENU_*` constant, and `handle_edit_name_key`'s hard-coded
`set_metadata(conn, "company_name", …)` (`:310`) becomes key-parameterised. The
address field is a single-line buffer whose typed `\n` cannot be entered — the
same limitation `ClientForm`'s "Address" field already has
(`cli/client_manager.rs:79`), so **it takes `\n` as the two-character escape
`\n`** and stores real newlines. That is a new convention; it is the smallest
one that lets a two-line address be typed into a TUI form that has no multi-line
widget, and it is applied to this field only. The logo field takes a **path**,
reads the file, and runs `parse_logo`; a failure is the status line's, not the
save's.

**Web.** `PUT /api/settings/company-name` becomes
**`GET`/`PUT /api/settings/company`** carrying `{name, address, phone, logo}`,
and the single-field route is removed. Two writers for one letterhead is how
`company_name` and `company_address` end up disagreeing about whether they were
saved. The API serves only its own embedded SPA over `127.0.0.1`, so there is no
external consumer to break; the callers are `api/client.ts:444`,
`api/types.ts:350-357`, `screens/settings.ts:167-179` and the locked-route probe
at `server/testutil.rs:364`. `StatusResponse.companyName` (`status.rs:60`) does
**not** grow — the sidebar and the document title want a name, not a letterhead.

**CLI.** Nothing. There is no `nigel settings` subcommand today and this task
does not invent one; the TUI settings screen and the web screen are the two
editors, as they are for `company_name`.

### Decision 4: the metadata column, and terms beside the due date

The reference's left column is label/value rows, one of which disappears when
the invoice has no due date. `{{DUE}}` is shaped for a sentence
(`<br>Due: 2026-09-03`) and cannot be a table row, so the metadata column gets
the `{{TOTALS}}` treatment — a fragment of `<tr>` rows built from a decision in
`document.rs`, so the PDF draws the identical rows:

```rust
// src/invoicing/document.rs
pub struct MetaRow { pub label: &'static str, pub value: String, pub emphasis: bool }
pub fn meta_rows(invoice: &Invoice) -> Vec<MetaRow>;
pub fn due_value(invoice: &Invoice) -> Option<String>;
```

| Row | Appears when | Value | Emphasis |
|---|---|---|---|
| Invoice ID | always | `1248` | yes |
| Issue Date | always | `2026-08-06` | no |
| Due Date | `due_date` is set | `due_value(invoice)` | no |

`due_value` is where AC #4 lives, and it is one rule in one function:

- no due date → `None`;
- due date, no terms → `2026-09-05`;
- due date, terms on a single line after trimming → `2026-09-05 (Net 30)`;
- due date, **multi-line** terms → `2026-09-05`, and the terms stay a block. A
  paragraph does not belong in parentheses after a date, and the alternative — a
  character-count threshold — is a hidden rule nobody can predict.

`{{TERMS_BLOCK}}` is the new optional fragment that carries the terms **only
when `due_value` did not fold them in**, so nothing is printed twice. The
existing `{{TERMS}}` keeps its exact current meaning (the Terms block whenever
terms are set) for any template already using it; the stock page switches to
`{{TERMS_BLOCK}}`. Likewise `{{DUE}}` and `{{DUE_DATE}}` are **not touched** —
the stock page stops using them in favour of `{{META_ROWS}}`, and an older
template using them renders exactly what it always did.

### Decision 5 (obstacle C): the PDF carries no live payment link — settled

**Settled by Sam, not open.** His words: *"No need for a live link in the
PDF. The live link in the email is sufficient."*

The rule now has an owner as well as a rationale. The rationale, for the record:
an emailed attachment cannot be recalled or republished, so a live Stripe charge
link inside one would outlive the settlement it was created for. The page cannot
have that problem — `pay_button_for` (#206, `render.rs:16`) omits the button for
a void or settled invoice, `republish.rs` rewrites the page when a payment
lands, and `void_invoice_with_teardown` replaces it outright and deactivates the
link. Nothing rewrites an attachment sitting in someone's mail.

Worth stating plainly: **nothing deactivates a Stripe payment link on
settlement.** Only void calls `deactivate_payment_link`. So the URL that #206
stops *showing* is still live in Stripe — which is exactly why it must not be
printed on a document that cannot be corrected.

**The reference's "Pay online" line is deliberately not reproduced in Nigel's
PDF.**

That leaves what, if anything, stands in that slot. **Recommendation: print the
invoice page URL** — `View or pay online: https://billing.example.com/i/<token>/index.html`
— right-aligned under the Amount Due figure, omitted entirely when
`public_base_url` is unset.

- It is republishable by construction. A settled invoice's page shows a zero
  balance and no button; a voided one shows `voided_page_html`. The attachment
  points at whatever is true today, which is the property the rule protects.
- A PDF is the artifact forwarded to an AP department, and "where do I pay this"
  is the one question it currently cannot answer.
- It costs one `Branding` field. The URL is `r2::public_url(base, &invoice.token)`
  — pure, deterministic from the token, and computable before publish, so
  `send` and `preview` produce the same document. Resolving it is the CLI/server
  layer's job, like every other `Branding` field.

Two caveats to write down rather than discover: it is **printed text, not a
clickable annotation** (printpdf's `annotations` feature, which would add
`pdf-writer`, is off — most readers auto-linkify a bare URL, and enabling it is
a candidate follow-up, not this task); and `invoice preview` on a draft that is
never sent prints a URL that does not resolve yet. The alternative — the PDF
saying nothing at all about paying — is defensible and one line cheaper, but it
makes the attachment a dead end.

`pdf.rs`'s `no_live_payment_link_reaches_the_pdf` test stays, and gains a
sibling asserting the printed URL is the page's and never
`stripe_payment_link_url`.

### Decision 6: Subject and Item Type are out of scope

**Both. Recommended explicitly, and neither is a near miss.**

**Subject** ("Bluepeak services: July 1 – July 31, 2026 / PO # 10000001") needs an
`invoices.subject` column, which is: a migration (`MIGRATIONS` +
`LATEST_VERSION` in `migrations.rs`), `create_invoice`/`update_invoice`/
`InvoiceUpdate`, a `--subject` flag on `nigel invoice new`/`edit`, a row in the
TUI draft form's repeatable-field machinery (`cli/invoice_manager.rs`),
`POST`/`PATCH /api/invoices`, `types.ts`, the SPA invoice form, and then the two
renderers and the placeholder vocabulary. Five surfaces and a schema version for
a cosmetic line — against a brief that says "similar, not exact". And the two
jobs the reference's Subject does are already served: the billing period reads
naturally as a line-item description, and the client's PO number reads naturally
in `notes`, which both documents already print.

**Item Type** ("Service") needs `invoice_line_items.item_type` — the same
migration cost, plus `NewLineItem`, which is a `Deserialize` wire input, plus
`validate_items`, plus the repeatable `--item` grammar. For one column whose
only observed value is the word "Service". Dropping it leaves four of the
reference's five columns, and the four that carry the figures.

If Sam wants either, they are proper backlog tasks with their own migrations,
not riders on a layout change.

### Decision 7: dates stay zero-padded ISO

**Recommendation: keep `YYYY-MM-DD`. Do not add a display format.**

The reference prints `08/06/2026`. Nigel stores and prints zero-padded ISO
everywhere, and #203 made that an invariant rather than a habit: `validate_date`
returns the **normalized** zero-padded string, all five date writers go through
it, and `is_overdue` compares ISO strings directly — an unpadded or reordered
date would never read as past due.

A display-only format would not break that invariant, and the honest cost is
small but real: a new `fmt::` function, two call sites (`meta_rows` is the only
place either document formats a date, so it would be *one*), and tests. What it
actually costs is coherence. The client-facing document would be the only
surface in the product printing dates differently from the CLI, the TUI, the web
UI, every report and every export — so an operator reading `2026-09-05` on the
aging report and `09/05/2026` on the invoice has two vocabularies for one date.
And `MM/DD/YYYY` is unambiguous only inside the US; ISO is unambiguous
everywhere and is what an AP system parses.

If Sam overrules this, the change is genuinely one function called from
`document::meta_rows`, and it should be a metadata key rather than a constant so
it is a choice and not a hard-coded assumption about locale.

### Decision 8: what the PDF writer needs to grow

`PdfWriter` is a single-column top-down writer: `self.y` only advances, `text`
takes an arbitrary `x`, and the only rule primitive is `hline`. The house layout
needs four small additions, all inside `src/pdf.rs`:

| Addition | Why |
|---|---|
| `vline(x, y_from, y_to)` | The vertical rules beside "From" and "Invoice For", and the item table's column dividers |
| `text_right(s, right_edge, size, bold)` | The label column, the Amount Due block and the page URL, without inventing a `Col` for each |
| An explicit save/restore of `self.y` around a two-column band | Draw the left column, reset `y` to the band's top, draw the right column, then set `y` to the lower of the two |
| `wordmark(name)` | The company name at 22pt bold at `MARGIN_LEFT`, Decision 2's PDF half |

`ensure_space`/`new_page` keep their meaning: the two-column bands are drawn
before any table row, so nothing paginates mid-band. The reference's
"Page 1 of 1" footer is **out of scope** — it needs a total page count, which
means a two-pass render, for a line no one reads on a one-page invoice.

### Decision 9: `REQUIRED` does not grow, and no shipped key changes meaning

Four keys join `PLACEHOLDERS`, none joins `REQUIRED`, and `REQUIRED_ALTERNATIVES`
is untouched:

| New key | Kind | Value | Empty when |
|---|---|---|---|
| `{{LOGO}}` | fragment | `<img class="logo" src="data:image/png;base64,…" alt="Acme LLC">` | no `company_logo` |
| `{{META_ROWS}}` | fragment | the invoice-metadata `<tr>` rows | never — the Invoice ID row is always there |
| `{{TERMS_BLOCK}}` | fragment | `<h3>Terms</h3><p>…</p>` | terms unset, or folded into the Due Date row |
| `{{COMPANY_ADDRESS}}` / `{{COMPANY_PHONE}}` | text | the escaped raw values | unset |

`{{COMPANY_BLOCK}}` is **extended** rather than added: it becomes the full From
block (name bold, address lines, phone) instead of `<p class="company">Name</p>`.
That is safe because it is a #204 key that has not shipped in a release — the
"template exported before this change" case means a template written against the
**pre-#204** vocabulary, and every key in that vocabulary (`{{DUE}}`,
`{{TERMS}}`, `{{COMPANY}}`, `{{TOTAL}}`, `{{PAY}}`, …) renders exactly what it
renders today. The regression test #204 introduces for that legacy template stays
and must stay green.

### Decision 10: the side-by-side review is a task, not a hope

AC #9 asks for it. The plan's penultimate task is a **HALT** on four fixtures
rendered through `nigel invoice preview`, chosen so that every omission rule in
this spec is visible in at least one of them:

| Fixture | Exercises |
|---|---|
| House | logo, company address + phone, two-line client address, email, due date with `Net 30` terms, three items, notes, a live Stripe link |
| Sparse | no logo, no company address or phone, no client address or email, no due date, no terms, no notes, one item, draft |
| Long terms | multi-line terms — the Due Date row bare, the Terms block present, nothing duplicated |
| Part-paid | House plus half the total recorded — Paid/Balance rows, no Pay button once settled |

Step 2 of that task is checking the sparse pair before Sam sees it: no empty
label, no orphan rule, no `<br>` with nothing after it, no `From` heading over
nothing, in either document.

## Out of scope

- **A per-invoice Subject and a per-line Item Type.** Decision 6.
- **A logo in the PDF.** Decision 2 — with the seam left pointing at it and the
  cost measured, so reversing it is a decision rather than a rediscovery.
- **A live Stripe link in the PDF.** Decision 5, settled by Sam.
- **Clickable link annotations in the PDF.** Would enable printpdf's
  `annotations` feature (`pdf-writer`). Candidate follow-up.
- **`Page N of M`.** Decision 8.
- **Currency glyph unification.** The page prints `USD 250.00` and the PDF
  `$250.00`; that is TASK-87's, deferred by #204 on purpose, and reopening it
  here would restyle every invoice ever sent as a side effect of a layout change.
- **The email template.** There is none. `send.rs:249` emails
  `rendered.html`, so everything in this spec reaches the inbox by construction —
  which is also why the Gmail data-URI degradation in Decision 2 is a documented
  consequence rather than an oversight.
- **Bill.com-style extra payment-routing lines** in the From block. The
  reference has two; they are a third-party network ID with no home in Nigel's
  schema and no second user. If they are wanted, they are a `company_notes`
  metadata key in a follow-up, not a column.
- **Restyling the SPA's invoice screens.** `wc-document-frame` (#207) renders
  whatever the seam produces.

## Open questions for Sam

1. **The page URL in the PDF.** Recommended in Decision 5: printed, right-aligned
   under Amount Due, omitted when `public_base_url` is unset. The alternative is
   a PDF that says nothing about paying at all. Yes/no.
2. **The logo, given the measured cost.** Decision 2 recommends page-only with a
   22pt wordmark in the PDF. The reversal is now precisely priced — 8 crates and
   ~984 KiB — and the alpha defect is avoidable by flattening onto white. Say if
   ~1 MB is worth a logo on the attachment.
3. **The TUI's `\n` escape for the company address.** Decision 3. It is the only
   way to type a two-line address into a form with no multi-line widget, and it
   is a convention this app does not otherwise have. The alternative is a
   single-line company address (`P.O. Box 1234, Springfield, CA 90001`), which is
   less like the reference and needs no new convention.
4. **Retiring `PUT /api/settings/company-name` for `PUT /api/settings/company`.**
   Decision 3. Free at this scale, but it is an API shape change.
5. **Dates.** Decision 7 recommends keeping ISO everywhere. Confirm, or say
   `MM/DD/YYYY` on the client-facing documents only and accept two vocabularies.
6. **Confirm Subject and Item Type stay out** (Decision 6), or file them as their
   own tasks with their own migrations.
</content>
</invoke>
