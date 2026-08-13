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

A second gap is configurability rather than layout. `templates/invoice.html`
hardcodes a bank-transfer paragraph — "Direct deposit / To pay by bank transfer,
reference invoice #N. Contact `{{CONTACT}}` for account details." — in which only
the address is variable. `src/pdf.rs` has no equivalent block, so the two
documents disagree; the block is unconditional, so an installation that takes no
bank transfers still advertises one; and editing the wording means owning a
custom template forever, which still cannot reach the PDF. Decision 11 makes it
configurable text carried by both documents or by neither.

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
| `src/invoicing/document.rs` | The module that decides, once, what both documents say — `MoneySummary`/`MoneyLine`, `address_lines`, `email_line`, `MAX_ADDRESS_LINES`, `ADDRESS_TRUNCATED` | Gains `CompanyBlock`, `MetaRow`/`meta_rows`, `due_value`, `parse_logo`, `payment_lines`. Same rule: the decision lives above both renderers |
| `document::address_lines` | Splits a billing address into drawable lines, blank ones dropped, clamped at 6 with `...` | Reused verbatim for the **company** address. One clamp, one truncation marker, both parties |
| `document::MoneySummary::lines()` | Which money rows exist and which are emphasised | Untouched. The Amount Due block renders `lines()`; only its CSS/geometry changes |
| Optional fragment placeholders | `COMPANY_BLOCK`, `CLIENT_ADDRESS_BLOCK`, `CLIENT_EMAIL_BLOCK`, `TOTALS` — added to `PLACEHOLDERS`, never to `REQUIRED` | The pattern for six more: `LOGO`, `META_ROWS`, `TERMS_BLOCK`, `PAYMENT_BLOCK`, plus the `COMPANY_ADDRESS`/`COMPANY_PHONE`/`PAYMENT_INSTRUCTIONS` text keys |
| `REQUIRED_ALTERNATIVES` | `("TOTAL", "TOTALS")` — a required key another key stands in for | Untouched, and the reason `REQUIRED` still does not need to grow |
| `Branding<'a>` | `{ template, company, contact_email }`, all `&'a str`, resolved by the CLI/server layer because `src/invoicing/` reads no settings and no database | Grows `company_address`, `company_phone`, `logo`, `payment_instructions`. Same convention: borrowed `&str`, empty means unset |
| `render_invoice_pdf(invoice, client, items, company, summary)` | The PDF, drawing the company name bold under the number, then the client block | Takes the whole company block, the parsed logo and the payment instructions instead of a bare `company: &str` |
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
Payment                                   ← Decision 11, both documents, omitted
…                                            entirely when nothing is configured
```

The logo block is the **real image** on both documents (Decision 2); the
wordmark is what stands in its place when there is no usable one.

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

### Decision 2 (obstacle A): the real logo goes in both documents

**Settled by Sam, reversing this document's earlier recommendation.** The
recommendation was page-only with a 22pt text wordmark in the PDF; the price of
the reversal was measured before it was taken, and Sam took it knowing the
number. An invoice is the most client-facing artifact this product produces, and
the attachment is the half that gets forwarded to an AP department and printed.
A letterhead that is a letterhead on the page and a line of Helvetica on the
attachment is not one design.

The findings below stand as the record of what the reversal costs and of the
defect it has to route around. They were reproduced against the version actually
in the lockfile:

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

**So: `{{LOGO}}` on the page, and the same image drawn top-left in the PDF.**
`printpdf` gains `features = ["embedded_images"]`. When no logo is configured —
or when the configured one cannot be embedded — the PDF draws the company name
in `HelveticaBold` at 22pt in the same top-left position and the page's From
block carries the name alone, so the two documents still agree.

Three things the reversal has to get right:

**RGBA never reaches printpdf.** Finding 5 is not an edge case to guard against;
it is the normal path, because an operator's logo is a transparent PNG
essentially every time. So the flattening is not a fallback either: every image
is composited onto **white** before it is handed over, and `/SMask null` is what
the writer then emits. White rather than a guess at the operator's brand
background — a PDF page is white, and compositing onto the surface the image
will sit on is the only choice that is not an invention. A test asserts that
what reaches printpdf is never `Rgba8`, so the defect stays unreachable by
construction rather than by review.

**The bound.** The logo is drawn at `MARGIN_LEFT`, at the top of the page,
scaled to fit a **60 mm × 16 mm** box with its aspect ratio preserved (fill the
box in whichever dimension binds first). The printable width is 177.8 mm and the
From block sits at the right of it; 60 mm leaves that block 117 mm, and the
reference's own logo occupies about a third of the width. 16 mm is the height of
a four-line From block at this document's 5 mm line spacing, so a logo that
fills the box ends level with the block beside it and can never collide with it.
The box is a bound, not a size: a taller-than-wide logo is 16 mm tall and
narrow, a wide wordmark is 60 mm wide and shorter.

**The fallback is mandatory and total.** Bad magic bytes, a payload over
`MAX_LOGO_BYTES`, a decode failure, an unsupported colour type, dimensions that
cannot be read: every one of them ends with the PDF drawing the text wordmark
and the page rendering no `<img>` at all, so the From block's name stands where
the image would have been. **A logo problem may never fail an invoice render or
a send.** The wrong shape here is an exception propagating out of
`render_invoice` and turning a bad PNG into an unsendable invoice; a logo is
decoration on a document about money.

Two places make that safe rather than silent. Both editing surfaces run
`parse_logo` **before** `set_metadata`, so a bad file is refused at the settings
screen with a sentence naming what was wrong — a stored value that cannot be
embedded is therefore not reachable through any supported path. The renderers'
fallback exists for the value that got in another way (a hand-edited `metadata`
row, a restored backup from a build with a wider allow-list), and for that value
the right answer is a correct invoice without a logo.

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
that Gmail readers see the `alt` text — the company name — in the email body,
while the **attachment on that same email carries the real logo**, which is the
half a client keeps. An operator who wants a
hosted image in the email body has the escape hatch that already exists:
`{{LOGO}}` is optional, and their own `<data_dir>/templates/invoice.html` can
carry an absolute `<img src="https://…">` instead.

Shape and validation, all pure and all in `document.rs`:

- Stored value: `data:image/png;base64,<payload>` or `data:image/jpeg;base64,…`.
- **PNG and JPEG only.** SVG is unrendered by most mail clients and cannot be
  handed to printpdf at all, so allowing it would buy a validation branch and a
  document that disagrees with itself.
- `parse_logo(&str) -> Result<Logo>` checks the prefix, the MIME against the
  allow-list, that the payload base64-decodes, that the decoded bytes carry the
  right magic (`\x89PNG\r\n\x1a\n` / `\xFF\xD8\xFF`), that its pixel dimensions
  can be read out of the header and are non-zero, and that the payload is at
  most **128 KiB** — every byte is base64-inflated by a third into every email
  body and every published object.
- It yields what **both** renderers need from one parse:

  ```rust
  pub struct Logo { pub mime: &'static str, pub base64: String,
                    pub bytes: Vec<u8>, pub width: u32, pub height: u32 }
  ```

  `base64` is the page's `<img src>` payload, verbatim as stored. `bytes` is
  what the PDF decodes and embeds. `width`/`height` are read from the PNG
  `IHDR` and the JPEG `SOFn` frame by a small pure reader, because `document.rs`
  must validate a logo identically in a build with **no `pdf` feature**, where
  the `image` crate does not exist. A `pdf`-gated test cross-checks the header
  reader against `image`'s own decode for both formats, so the two cannot drift;
  the transform the PDF draws with uses the decoded image's dimensions, which is
  what printpdf actually embeds.
- Input is a **file path** in the TUI and the data URI itself over the API (the
  SPA does the `FileReader` work); both go through `parse_logo` before
  `set_metadata`, so a bad file is refused at the settings screen and never at
  send time.
- This needs `base64` (0.22) in `Cargo.toml`. It is **already in `Cargo.lock`**,
  pulled transitively by `reqwest` and `rusty-s3`, so naming it directly adds no
  crate to the build. Confirm with `cargo tree` at implementation time and say
  so; if the version resolved changes, report it rather than pulling a second
  copy in quietly.

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
pub(crate) struct CompanyProfile {
    pub name: String, pub address: String, pub phone: String,
    pub logo: String, pub payment_instructions: String,
}
pub(crate) fn company_profile(conn: &Connection) -> CompanyProfile;   // five get_metadata reads

// src/invoicing/render_html.rs
#[derive(Default)]
pub struct Branding<'a> {
    pub template: &'a str,
    pub company: &'a str,
    pub company_address: &'a str,   // empty means unset, as `company` already does
    pub company_phone: &'a str,
    pub logo: &'a str,              // the data URI, empty means none
    pub payment_instructions: &'a str,   // Decision 11
    pub contact_email: &'a str,
}
```

`Default` is derived because the struct is built in twenty-odd test literals
and a field added to all of them by hand is a field that ends up meaning
different things in different tests. Production sites still name every field.

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
`MENU_COMPANY_ADDRESS`, `MENU_COMPANY_PHONE`, `MENU_COMPANY_LOGO` and
`MENU_PAYMENT_INSTRUCTIONS` before `MENU_PASSWORD`; `MENU_LAST` moves.
`Screen::EditingName` carries no discriminator today, so it becomes
`Screen::Editing(usize)` keyed by the `MENU_*` constant, and
`handle_edit_name_key`'s hard-coded `set_metadata(conn, "company_name", …)`
(`:310`) becomes key-parameterised. The address field is a single-line buffer
whose typed `\n` cannot be entered — the same limitation `ClientForm`'s
"Address" field already has (`cli/client_manager.rs:79`), so **it takes `\n` as
the two-character escape `\n`** and stores real newlines. That is a new
convention; it is the smallest one that lets a multi-line value be typed into a
TUI form that has no multi-line widget, and it is applied to the two multi-line
fields — the company address and the payment instructions — and to nothing else.
The logo field takes a **path**, reads the file, and runs `parse_logo`; a
failure is the status line's, not the save's.

**Web.** `PUT /api/settings/company-name` becomes
**`GET`/`PUT /api/settings/company`** carrying
`{name, address, phone, logo, paymentInstructions}`,
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

That leaves what, if anything, stands in that slot. **Settled by Sam:
nothing. No Stripe link and no page URL either.** This document earlier
recommended printing `{base}/{token}/index.html` under the Amount Due figure;
that recommendation is withdrawn.

- The email carries the live link, and the email is where the invoice arrives.
  The attachment is read beside it, not instead of it.
- A tokenized page URL is about sixty characters of opaque text that no reader
  can retype and no PDF reader can be relied on to linkify. Printed unclickable
  under the figure that matters, it is noise on the one block a client actually
  reads.
- It also removes a caveat rather than documenting one: `invoice preview` on a
  draft would otherwise print an address that does not resolve yet.

So `page_url` never joins `Branding` and never reaches `render_invoice_pdf`.
Nothing else wanted it, so `cli::invoice::page_url_for` is not written either —
`r2::public_url` keeps its single caller.

What a client who has only the attachment does is what they did before this
change: the invoice carries the sender's name, address, phone and — new in
Decision 11 — the operator's own payment instructions, which is the block that
answers "where do I send the money" for anyone not clicking a link at all.

`pdf.rs`'s `no_live_payment_link_reaches_the_pdf` test stays green, and gains a
stronger sibling: the PDF prints **no URL of any kind**.

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
| `text_right(s, right_edge, size, bold)` | The label column and the Amount Due block, without inventing a `Col` for each |
| An explicit save/restore of `self.y` around a two-column band | Draw the left column, reset `y` to the band's top, draw the right column, then set `y` to the lower of the two |
| `logo(&Logo)` and `wordmark(name)` | Decision 2's PDF half: the image inside its 60 × 16 mm box, or the company name at 22pt bold at `MARGIN_LEFT` when there is no usable image. `logo` returns whether it drew, and the caller falls back — a refusal here is never an error |

`ensure_space`/`new_page` keep their meaning: the two-column bands are drawn
before any table row, so nothing paginates mid-band. The reference's
"Page 1 of 1" footer is **out of scope** — it needs a total page count, which
means a two-pass render, for a line no one reads on a one-page invoice.

### Decision 9: `REQUIRED` does not grow, and no shipped key changes meaning

Six keys join `PLACEHOLDERS`, none joins `REQUIRED`, and `REQUIRED_ALTERNATIVES`
is untouched:

| New key | Kind | Value | Empty when |
|---|---|---|---|
| `{{LOGO}}` | fragment | `<img class="logo" src="data:image/png;base64,…" alt="Acme LLC">` | no `company_logo`, or one that does not parse |
| `{{META_ROWS}}` | fragment | the invoice-metadata `<tr>` rows | never — the Invoice ID row is always there |
| `{{TERMS_BLOCK}}` | fragment | `<h3>Terms</h3><p>…</p>` | terms unset, or folded into the Due Date row |
| `{{PAYMENT_BLOCK}}` | fragment | `<h3>Payment</h3><p>…</p>`, one line per typed line | `payment_instructions` unset (Decision 11) |
| `{{COMPANY_ADDRESS}}` / `{{COMPANY_PHONE}}` | text | the escaped raw values | unset |
| `{{PAYMENT_INSTRUCTIONS}}` | text | the escaped raw value | unset |

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
| House | a real transparent-PNG logo, company address + phone, multi-line payment instructions, two-line client address, email, due date with `Net 30` terms, three items, notes, a live Stripe link |
| Sparse | no logo, no company address or phone, no payment instructions, no client address or email, no due date, no terms, no notes, one item, draft |
| Long terms | multi-line terms — the Due Date row bare, the Terms block present, nothing duplicated |
| Part-paid | House plus half the total recorded — Paid/Balance rows, no Pay button once settled |

Step 2 of that task is checking the sparse pair before Sam sees it: no empty
label, no orphan rule, no `<br>` with nothing after it, no `From` heading over
nothing, no `Payment` heading over nothing, in either document.

The House fixture's logo is deliberately a **wide transparent PNG** — the shape
that reproduces printpdf's soft-mask defect — so the flattening in Decision 2 is
being looked at by a human, not only asserted in a test.

### Decision 11: payment instructions are configuration, on both documents

New scope, added to the task after this document was first written. Today the
stock page ends with:

```html
<h3>Direct deposit</h3>
<p>To pay by bank transfer, reference invoice <strong>#{{NUMBER}}</strong>.
   Contact {{CONTACT}} for account details.</p>
```

Three things are wrong with it. It is **hardcoded English** in a template whose
whole point is that the wording is the operator's. It is **unconditional**, so an
installation that has never taken a bank transfer advertises one on every
invoice it sends. And the **PDF has no equivalent block at all**, so the two
documents disagree about how to pay — which is exactly the class of divergence
`document.rs` exists to end.

**So: a fifth company-profile key, `payment_instructions`, holding multi-line
free text, rendered on both documents or on neither, and omitted entirely when
unset.**

- It sits beside `company_address`/`company_phone` in `metadata` for the same
  four reasons Decision 3 gives: per-database, backed up, encrypted with the
  database, edited where `company_name` is edited.
- The key is `payment_instructions` rather than `company_payment_instructions`.
  The other four say who the sender *is*; this one tells the reader what to
  *do*, and naming it after the company would misfile it.
- Splitting is `document::payment_lines(&str)` — trimmed, blank lines dropped —
  the same shape as `address_lines` but with **no clamp and no truncation
  marker**. An address is a postal fact with a natural length; instructions are
  the operator's own prose about their own bank, and cutting them at six lines
  with `...` would be Nigel editing a sentence about where money goes. The PDF
  wraps them through `table_row_wrapped`, which paginates, so there is nothing
  to protect against.
- Heading on both documents: **Payment**, under the same full-width foot rule
  the Notes block sits under.
- No interpolation. The old sentence embedded `#{{NUMBER}}`; free text that
  quietly rewrote parts of itself would be a template language inside a
  template value, and the invoice number is already the largest thing on both
  documents.

#### What happens to `{{CONTACT}}`

**Preserved in the vocabulary, retired from the stock documents.** Not folded
into the new text, and not removed.

- Preserved because `{{CONTACT}}` shipped, and the rule this whole design runs
  on is that no shipped key changes meaning. It stays in `PLACEHOLDERS` and
  keeps expanding to exactly what it expands to today — `contact_email` falling
  back to `from_email`, escaped — so an operator's own template that prints it
  is untouched.
- Not folded in, because the two values are different in kind and in home.
  `contact_email` is a *send identity*: one email address, in `settings.json`,
  per machine, overridable by `NIGEL_CONTACT_EMAIL`, sitting beside
  `from_email` and the Mailgun keys. `payment_instructions` is *letterhead*:
  multi-line prose, in the database, per set of books. Making the second read
  the first would tie a paragraph about a bank to the address that Mailgun
  replies go to.
- Retired from the stock page, which now ends with `{{PAYMENT_BLOCK}}` instead
  of the hardcoded paragraph. An operator who wants their contact address in
  the instructions types it there — it is free text, and typing an address is
  cheaper than a rule that inserts one.

One consequence to state rather than let someone discover: `nigel invoice
preview` prints a notice when neither `contact_email` nor `from_email` is set,
because the stock page used to print that address. It now fires **only when the
template actually contains `{{CONTACT}}`**, which the stock page no longer does.
A notice about a placeholder the document does not carry is noise.

The other consequence: an installation that upgrades and sets nothing loses the
bank-transfer paragraph from its page. That is the point of AC #12 — the block
was never true for everyone — and it is what `docs/invoicing.md` has to say in
so many words.

## Out of scope

- **A per-invoice Subject and a per-line Item Type.** Decision 6.
- **A live Stripe link in the PDF.** Decision 5, settled by Sam.
- **The invoice page URL in the PDF.** Decision 5, settled by Sam. The PDF
  prints no URL at all.
- **Clickable link annotations in the PDF.** Would enable printpdf's
  `annotations` feature (`pdf-writer`). Nothing in either document is a link
  any more, so there is nothing left to annotate.
- **SVG logos.** Decision 2 — unrendered by most mail clients and not something
  printpdf can embed, so it would produce two documents that disagree.
- **Interpolating the invoice number into the payment instructions.**
  Decision 11.
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

## Questions Sam has settled

1. **The page URL in the PDF — no.** The PDF carries no Stripe link *and* no
   page URL. The email's live link is sufficient, and sixty characters of
   tokenized URL printed as unclickable text is noise. Decision 5.
2. **The logo, given the measured cost — reversed, both documents get the real
   image.** Decision 2. `embedded_images` is enabled, RGBA is flattened onto
   white before printpdf sees it, and the fallback to a text wordmark is
   mandatory on every failure path.
3. **Payment instructions — configurable, on both documents or neither.** New
   scope; Decision 11. `{{CONTACT}}` keeps its meaning and leaves the stock
   documents.

## Still open for the side-by-side review (Task 10)

1. **The TUI's `\n` escape for the company address and the payment
   instructions.** Decision 3. It is the only way to type a two-line value into
   a form with no multi-line widget, and it is a convention this app does not
   otherwise have. The alternative is single-line values, which are less like
   the reference and need no new convention.
2. **Retiring `PUT /api/settings/company-name` for `PUT /api/settings/company`.**
   Decision 3. Free at this scale, but it is an API shape change.
3. **Dates.** Decision 7 recommends keeping ISO everywhere. Confirm, or say
   `MM/DD/YYYY` on the client-facing documents only and accept two vocabularies.
4. **Confirm Subject and Item Type stay out** (Decision 6), or file them as their
   own tasks with their own migrations.
