# The house invoice layout — TASK-101 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Spec: `docs/superpowers/specs/2026-08-12-task-101-house-invoice-layout-design.md`.
Read it first — every "why" below lives there, including the three obstacles and
Sam's ruling on the pay link.

**Goal:** both client-facing documents carry the house layout — the real logo,
a ruled From block with the company's address and phone, a two-column band of
invoice metadata and Invoice For, a ruled item table with right-aligned figures,
a right-aligned Amount Due, Notes under a full-width rule and the operator's own
payment instructions — with the company address, phone, logo and payment
instructions configurable in one place, a missing value omitting its block, the
two documents agreeing on everything they both carry, and every template
exported before this change still loading.

**Three owner decisions the spec now records, reversing or extending what it
first recommended.** Read them before Task 1; they are why several steps below
differ from what a first reading of the spec would suggest:

1. **The real logo goes in both documents** (spec Decision 2). `printpdf` gains
   `embedded_images`; RGBA is flattened onto white before printpdf sees it; the
   fallback to a text wordmark is mandatory on every failure path and a logo
   problem may never fail a render or a send.
2. **The PDF carries no page URL** (spec Decision 5) — no Stripe link and no
   `{base}/{token}/index.html` line either. `page_url` never joins `Branding`.
3. **Payment instructions are configurable text on both documents** (spec
   Decision 11) — new scope, replacing the hardcoded "Direct deposit" paragraph.

**Architecture:** every shared decision goes into `src/invoicing/document.rs`,
the module PR #204 created for exactly this: it gains `CompanyBlock`/
`company_block`, `MetaRow`/`meta_rows`, `due_value`, `parse_logo` and
`payment_lines` beside the `MoneySummary`, `address_lines` and `email_line`
already there, and both renderers consume them, so the page and the PDF agree by
construction rather than by review. `render_html.rs` gains six optional
placeholders following #204's fragment pattern (`LOGO`, `META_ROWS`,
`TERMS_BLOCK`, `PAYMENT_BLOCK`, plus the `COMPANY_ADDRESS`/`COMPANY_PHONE`/
`PAYMENT_INSTRUCTIONS` text keys) and extends #204's `{{COMPANY_BLOCK}}` into
the full From block; `REQUIRED` and `REQUIRED_ALTERNATIVES` do not change.
`Branding` grows `company_address`, `company_phone`, `logo` and
`payment_instructions`, all resolved once by a new
`cli::invoice::company_profile` instead of by each of the construction sites.
`src/pdf.rs` grows five drawing primitives (`vline`, `text_right`, an explicit
two-column band, `logo`, `wordmark`) and draws the same blocks from the same
`document.rs` functions.

**Tech Stack:** Rust, rusqlite (the `metadata` key-value table), printpdf 0.7
behind the `pdf` feature **with `features = ["embedded_images"]`** and
`pdf::extract_text` as the assertion seam, the embedded HTML template, `base64`
(named directly; already in `Cargo.lock` via reqwest/rusty-s3), axum + the Lit
SPA for the settings surfaces.

**This lands after PR #204 and PR #206 merge** — ideally after #207 too. See the
spec's Decision 0: #204 rewrites `render_html.rs`, `pdf.rs` and the template and
creates `document.rs`; #206 rewrites `render.rs` and `cli/invoice.rs`. Branching
TASK-101 beside them is a three-way merge across every line of both renderers.
**Do not start Task 1 until `git log origin/main` shows both merged.**

## Global Constraints

- After every task, all of these green — CI's exact set (`.github/workflows/ci.yml`)
  plus the stricter clippy:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test -- --test-threads=1`
  - `cargo test --no-default-features -- --test-threads=1`
  - `cargo test --no-default-features --features serve -- --test-threads=1`
  - In `web/`, for any task touching the SPA: `npm run lint`, `npm run typecheck`,
    `npm test`, `npm run build`
  The PDF half of every change is `pdf`-gated and the HTML half must pass in
  both builds — that is what the no-default-features runs are for.
- **TDD, always.** Failing test first, watched failing for the right reason.
- **`REQUIRED` does not grow.** `{{NUMBER}}`, `{{CLIENT}}`, `{{ROWS}}`,
  `{{TOTAL}}` remain the whole requirement, and `REQUIRED_ALTERNATIVES` keeps its
  single `("TOTAL", "TOTALS")` entry.
- **No key that shipped before #204 changes meaning.** `{{DUE}}`, `{{DUE_DATE}}`,
  `{{TERMS}}`, `{{NOTES}}`, `{{COMPANY}}`, `{{CLIENT_ADDRESS}}`,
  `{{CLIENT_EMAIL}}`, `{{SUBTOTAL}}`, `{{TAX}}`, `{{TOTAL}}`, `{{PAY}}`,
  `{{PAY_URL}}`, `{{CONTACT}}` render exactly what they render today.
  `{{COMPANY_BLOCK}}` is extended, and may be, because #204 has not shipped.
- `expand` stays single-pass; every value stays escaped on the way in with `esc`;
  the template itself is still never sanitized.
- `src/invoicing/` reads no settings, opens no database for branding, and reaches
  into no `src/cli/`. `document.rs` stays pure — `parse_logo` takes a `&str`, not
  a path.
- **No live Stripe payment link reaches the PDF.** `pdf.rs`'s
  `no_live_payment_link_reaches_the_pdf` test stays green throughout, and the
  PDF prints no URL of any kind.
- **A logo problem never fails a render or a send.** Every `parse_logo`,
  decode, colour-type and dimension failure ends in the text wordmark and a
  correct invoice.
- **Nothing hands printpdf an RGBA image.** Asserted, not reviewed.
- The seam's shape does not change: `render_invoice(conn, invoice, client, pay,
  branding)` is what `preview`, `send`, `republish` and the API call.
- The page and the PDF must carry the same blocks. Every task that adds a block
  to one adds it to the other in the same task or the one immediately after, and
  Task 6 pins the pair.

---

### Task 1: `document.rs` — the company block, the metadata rows, the logo, the payment lines

**Files:** `src/invoicing/document.rs`; `Cargo.toml` (add `base64 = "0.22"`, and
`features = ["embedded_images"]` on `printpdf`).

**Interface produced** (consumed by Tasks 2, 3, 4, 5, 7, 8):

```rust
pub struct CompanyBlock<'a> { pub name: &'a str, pub address: Vec<&'a str>, pub phone: Option<&'a str> }
pub fn company_block<'a>(name: &'a str, address: &'a str, phone: &'a str) -> CompanyBlock<'a>;
impl CompanyBlock<'_> { pub fn is_empty(&self) -> bool; }

pub struct MetaRow { pub label: &'static str, pub value: String, pub emphasis: bool }
pub fn meta_rows(invoice: &Invoice) -> Vec<MetaRow>;
pub fn due_value(invoice: &Invoice) -> Option<String>;
pub fn terms_block_text(invoice: &Invoice) -> Option<&str>;   // Some only when due_value did not fold them

/// Everything both renderers need from one parse: `base64` for the page's
/// `<img src>`, `bytes` for the PDF to decode and embed, dimensions for the
/// bound — read from the file header, because this module must validate a logo
/// identically in a build with no `pdf` feature and so no `image` crate.
pub struct Logo {
    pub mime: &'static str, pub base64: String,
    pub bytes: Vec<u8>, pub width: u32, pub height: u32,
}
pub fn parse_logo(data_uri: &str) -> Result<Logo>;
pub const MAX_LOGO_BYTES: usize = 128 * 1024;

/// The operator's payment instructions as lines, blank ones dropped. No clamp
/// and no truncation marker: this is the operator's own prose about their own
/// bank, and the PDF's `table_row_wrapped` paginates.
pub fn payment_lines(text: &str) -> Vec<&str>;
```

- [ ] **Step 0: price the two dependency changes and report both numbers.**
      `cargo tree -p base64` and `git grep -n 'name = "base64"' Cargo.lock` —
      it is already in the lockfile via reqwest/rusty-s3, so naming it directly
      must add no crate; say so explicitly. Then measure the **release binary**
      before and after `printpdf`'s `embedded_images`: `cargo build --release`,
      record `ls -l target/release/nigel`, add the feature, rebuild, record
      again. Report both byte counts and the crate delta from
      `cargo tree --no-default-features --features pdf`. The spec's estimate was
      ~984 KiB and 8 new crates; a materially different number is worth saying
      out loud, not quietly accepting.
- [ ] **Step 1: Write failing tests** in the existing `mod tests`:

```rust
#[test] fn the_company_block_splits_its_address_the_way_a_client_address_is_split() {
    let b = company_block("Bluepeak", "P.O. Box 1234\n\nSpringfield, CA 90001", " 619.555.0123 ");
    assert_eq!(b.address, vec!["P.O. Box 1234", "Springfield, CA 90001"]);
    assert_eq!(b.phone, Some("619.555.0123"));
}
#[test] fn an_unset_company_block_says_nothing_at_all() {
    let b = company_block("", "  \n ", "   ");
    assert!(b.address.is_empty() && b.phone.is_none() && b.is_empty());
}
#[test] fn a_long_company_address_is_clamped_the_same_way_a_client_address_is() {
    // MAX_ADDRESS_LINES rows, the last one ADDRESS_TRUNCATED — one rule, both parties
}
#[test] fn the_metadata_rows_always_lead_with_the_invoice_id() {
    let rows = meta_rows(&invoice_with(None, None));
    assert_eq!(labels(&rows), vec!["Invoice ID", "Issue Date"]);
    assert!(rows[0].emphasis, "the number is what a client quotes back");
}
#[test] fn a_due_date_brings_its_row_and_a_missing_one_brings_nothing() {}
#[test] fn single_line_terms_ride_beside_the_due_date() {
    assert_eq!(due_value(&invoice_with(Some("2026-09-05"), Some("Net 30"))).unwrap(),
               "2026-09-05 (Net 30)");
}
#[test] fn multi_line_terms_stay_a_block_rather_than_a_parenthetical() {
    let inv = invoice_with(Some("2026-09-05"), Some("Net 30\nLate fees apply after 60 days."));
    assert_eq!(due_value(&inv).unwrap(), "2026-09-05");
    assert!(terms_block_text(&inv).is_some(), "the paragraph has to land somewhere");
}
#[test] fn folded_terms_do_not_also_print_as_a_block() {
    assert!(terms_block_text(&invoice_with(Some("2026-09-05"), Some("Net 30"))).is_none());
}
#[test] fn terms_with_no_due_date_are_a_block() {
    assert!(terms_block_text(&invoice_with(None, Some("Net 30"))).is_some());
}
#[test] fn a_png_data_uri_parses() {
    // mime, the base64 back verbatim, the decoded bytes, and the pixel size
}
#[test] fn a_jpeg_data_uri_parses_with_its_dimensions() {}
#[test] fn a_declared_png_that_is_not_a_png_is_refused() {
    // right prefix, valid base64, wrong magic bytes
}
#[test] fn an_svg_or_a_gif_data_uri_is_refused_by_name() {}
#[test] fn a_logo_over_the_cap_is_refused_with_its_size_in_the_message() {}
#[test] fn a_payload_that_is_not_base64_is_refused() {}
#[test] fn a_truncated_png_whose_size_cannot_be_read_is_refused() {}
#[test] fn a_zero_by_zero_image_is_refused() {}
#[test] fn the_empty_string_is_no_logo_rather_than_an_error() {}
#[test] fn payment_instructions_split_into_the_lines_they_were_typed_as() {
    // blank lines dropped, each line trimmed, unset is empty
}
#[test] fn payment_instructions_are_never_clamped_the_way_an_address_is() {
    // twenty lines in, twenty lines out, and no ADDRESS_TRUNCATED marker
}
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib invoicing::document 2>&1 | tail -30`
- [ ] **Step 3: Implement** per the spec's Decisions 3, 4, 2 and 11.
      `company_block` reuses `address_lines` verbatim and mirrors `email_line`'s
      trim-or-`None` for the phone. `due_value` folds terms only when
      `terms.trim()` contains no `\n`. `parse_logo` checks prefix → MIME
      allow-list (`image/png`, `image/jpeg`) → base64 decode → magic bytes
      (`\x89PNG\r\n\x1a\n`, `\xFF\xD8\xFF`) → `MAX_LOGO_BYTES` → pixel
      dimensions from the PNG `IHDR` / the JPEG `SOFn` frame, non-zero; every
      refusal is a `NigelError::Invalid` naming what was wrong. `payment_lines`
      is `address_lines` without the clamp.
- [ ] **Step 4: Verify.** All five cargo commands.

---

### Task 2: `Branding` grows, and one resolver fills it

**Files:** `src/invoicing/render_html.rs` (the struct only); `src/cli/invoice.rs`;
`src/cli/invoice_manager.rs`; `src/server/routes/invoices.rs`.

**Interface produced** (consumed by Tasks 3, 5, 7):

```rust
// src/invoicing/render_html.rs
#[derive(Default)]
pub struct Branding<'a> {
    pub template: &'a str, pub company: &'a str,
    pub company_address: &'a str, pub company_phone: &'a str,
    pub logo: &'a str, pub payment_instructions: &'a str,
    pub contact_email: &'a str,
}

// src/cli/invoice.rs
pub(crate) struct CompanyProfile {
    pub name: String, pub address: String, pub phone: String,
    pub logo: String, pub payment_instructions: String,
}
pub(crate) fn company_profile(conn: &Connection) -> CompanyProfile;
```

**No `page_url` and no `page_url_for`** — spec Decision 5. `r2::public_url`
keeps its single caller.

- [ ] **Step 1: Write failing tests.**

```rust
// cli/invoice.rs
#[test] fn the_company_profile_reads_all_five_metadata_keys() {}
#[test] fn an_unset_key_is_an_empty_string_not_a_missing_field() {}
// render_html.rs / render.rs — migrate every existing Branding literal with NO
// assertion change. That invariance is the point. Test literals take
// `..Branding::default()`; production sites name every field.
```

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement.** `company_profile` does five `db::get_metadata`
      reads (`company_name`, `company_address`, `company_phone`, `company_logo`,
      `payment_instructions`). Replace the `company_name(conn)` call at every
      production `Branding` site — `cli/invoice.rs` (`republish_after_payment`,
      `preview`, and both literals in `send`) and `server/routes/invoices.rs`
      (`republish`, `send_with`, `render`) — with one `company_profile` and
      borrow from it. **Leave `cli::invoice::company_name` alone** —
      `cli/export.rs`, `cli/report/text.rs`, `cli/status.rs` and
      `server/routes/status.rs` still want a bare name.
- [ ] **Step 4: Verify.** All five cargo commands, and
      `git grep -n 'company_name(conn\|company_name(&conn' src/` shows only the
      report/status callers left.

---

### Task 3: the HTML renderer's new fragments

**Files:** `src/invoicing/render_html.rs`.

**Interface produced:** `render_invoice_html` keeps #204's signature; four keys
join `PLACEHOLDERS`.

- [ ] **Step 1: Write failing tests** in the existing `mod tests`:

```rust
#[test] fn the_company_block_is_the_whole_from_block() {
    // name in <strong>, one line per address line, a "ph." phone line
}
#[test] fn the_from_block_omits_the_lines_it_does_not_have() {
    // name only: no empty address rows, no bare "ph.", no orphan rule element
}
#[test] fn an_entirely_unset_company_renders_no_from_block_at_all() {}
#[test] fn the_logo_is_an_img_with_the_company_name_as_its_alt_text() {
    // src is the stored data URI verbatim; alt is esc(company)
}
#[test] fn no_logo_renders_no_img() {}
#[test] fn the_meta_rows_are_table_rows_in_the_shared_order() {
    // labels and order == document::meta_rows(); the Invoice ID row is emphasised
}
#[test] fn a_due_date_with_terms_reads_as_one_value() {
    // "2026-09-05 (Net 30)" in one cell, not two rows
}
#[test] fn the_terms_block_is_empty_when_the_terms_rode_beside_the_date() {}
#[test] fn every_placeholder_in_the_vocabulary_still_expands() { /* #204's test, wider */ }
#[test] fn no_shipped_placeholder_changed_meaning() {
    // {{DUE}} is still "<br>Due: 2026-09-05" with no parenthetical;
    // {{TERMS}} is still the block whenever terms are set;
    // {{COMPANY}} is still the bare escaped name.
}
#[test] fn a_company_address_containing_markup_is_text() {}
#[test] fn the_payment_block_is_the_configured_text_one_line_per_line() {}
#[test] fn no_payment_instructions_render_no_payment_block() {}
#[test] fn payment_instructions_containing_markup_are_text() {}
#[test] fn the_contact_placeholder_still_expands_to_the_contact_address() {
    // {{CONTACT}} keeps its exact shipped meaning; only the stock page stops
    // using it
}
```

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement.** Add `LOGO`, `META_ROWS`, `TERMS_BLOCK`,
      `PAYMENT_BLOCK`, `COMPANY_ADDRESS`, `COMPANY_PHONE` and
      `PAYMENT_INSTRUCTIONS` to `PLACEHOLDERS` — **not** to `REQUIRED`. Build
      the fragments beside #204's `block()` closure, from
      `document::company_block`, `document::meta_rows`,
      `document::terms_block_text` and `document::payment_lines`.
      `{{COMPANY_BLOCK}}` becomes the full From block. `{{META_ROWS}}` emits
      `<tr><th>Issue Date</th><td>2026-08-06</td></tr>`, the emphasised row's
      `<td>` carrying `class="strong"`. `{{LOGO}}` renders only when
      `document::parse_logo` accepted the stored value — a refusal is no `<img>`
      at all, never a broken one, and never an error. `{{CONTACT}}` is
      untouched.
- [ ] **Step 4: Verify.** All five cargo commands.

---

### Task 4: the stock page

**Files:** `src/invoicing/templates/invoice.html`.

- [ ] **Step 1: Write the failing tests** in `render_html.rs`'s `mod tests`,
      against `DEFAULT_TEMPLATE`:

```rust
#[test] fn the_stock_page_carries_all_seven_house_blocks() {
    // logo/wordmark, From, invoice metadata, Invoice For, item table,
    // Amount Due, Notes — in that document order
}
#[test] fn the_stock_page_item_table_says_quantity_and_unit_price() {
    // and no longer says "Qty" or "Unit"
}
#[test] fn the_stock_page_of_a_sparse_invoice_prints_no_empty_labels() {
    // no company, address, phone, logo, payment instructions, client address,
    // email, due date, terms or notes: assert no "<p></p>", no "<h3></h3>",
    // no "From", no "ph.", no "Due Date", no "Payment", no "<br><br>"
}
#[test] fn the_stock_page_no_longer_hardcodes_a_bank_transfer_paragraph() {
    // with payment_instructions unset: no "Direct deposit", no
    // "bank transfer", no "account details" anywhere in the rendered page
}
#[test] fn the_stock_page_prints_the_configured_payment_instructions() {}
#[test] fn the_stock_page_and_the_money_lines_agree() { /* #204's test, kept */ }
#[test] fn the_default_template_still_validates() { /* #204's test, kept */ }
```

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement** the spec's Decision 1 layout:

```html
<header class="letterhead">{{LOGO}}
  <div class="party from"><span class="party-label">From</span>
    <div class="party-body">{{COMPANY_BLOCK}}</div></div></header>
<section class="band">
  <table class="meta">{{META_ROWS}}</table>
  <div class="party"><span class="party-label">Invoice For</span>
    <div class="party-body"><strong>{{CLIENT}}</strong>{{CLIENT_ADDRESS_BLOCK}}{{CLIENT_EMAIL_BLOCK}}</div></div>
</section>
<table class="items">
  <thead><tr><th>Description</th><th>Quantity</th><th>Unit Price</th><th>Amount</th></tr></thead>
  <tbody>{{ROWS}}</tbody><tfoot>{{TOTALS}}</tfoot></table>
<p class="pay-line">{{PAY}}</p>
<hr class="foot-rule">
{{NOTES}}
{{TERMS_BLOCK}}
{{PAYMENT_BLOCK}}
```

      The `.party` rule is a left border on `.party-body`; when
      `{{COMPANY_BLOCK}}` is empty the whole `.party` must collapse — use
      `:empty` selectors or emit the wrapper from the fragment, and let the
      sparse test decide which. Right-align the three figure columns.
      **Delete the hardcoded "Direct deposit" paragraph** (spec Decision 11):
      `{{PAYMENT_BLOCK}}` replaces it, and it prints nothing at all when the
      operator has configured nothing. `{{CONTACT}}` stays in the vocabulary
      for templates that use it, and leaves the stock page.
- [ ] **Step 4: Verify.** All five cargo commands.

---

### Task 5: the PDF

**Files:** `src/pdf.rs`.

**Interface produced:**

```rust
pub fn render_invoice_pdf(
    invoice: &Invoice, client: &Client, company: &CompanyBlock<'_>,
    logo: Option<&Logo>, items: &[InvoiceLineItem], money: &MoneySummary,
    payment_instructions: &str,
) -> Result<Vec<u8>>;
```

No `page_url` parameter — spec Decision 5.

- [ ] **Step 1: Write failing tests** in `mod invoice_pdf_tests` (gated on
      `pdf`), using `extract_text`:

```rust
#[test] fn a_configured_logo_is_embedded_as_an_image() {
    // the rendered bytes carry an image XObject, and the wordmark is not drawn
}
#[test] fn nothing_handed_to_printpdf_is_ever_rgba() {
    // a transparent PNG in: the prepared image's colour type is Rgb8, the
    // stream carries /SMask null, and printpdf's width-sized soft mask is
    // therefore unreachable
}
#[test] fn a_transparent_logo_is_flattened_onto_white() {}
#[test] fn the_logo_is_bounded_and_keeps_its_aspect_ratio() {
    // a 1200x120 wordmark and a 120x1200 tower both fit 60mm x 16mm, each
    // filling exactly one dimension
}
#[test] fn an_unusable_logo_falls_back_to_the_wordmark_rather_than_failing() {
    // bad magic bytes, over MAX_LOGO_BYTES, undecodable payload, unsupported
    // colour type — each renders a PDF, and each draws the company name
}
#[test] fn the_wordmark_heads_the_document_when_there_is_no_logo() {
    // the company name is drawn first, before "From"
}
#[test] fn the_from_block_carries_the_address_and_the_phone() {
    // order: company < address lines < "ph. …"
}
#[test] fn an_unset_company_draws_no_from_block() {
    // no "From", no "ph.", and the metadata band still starts where it should
}
#[test] fn the_metadata_column_is_the_shared_one() {
    // labels, order and values == document::meta_rows()
}
#[test] fn the_invoice_for_block_matches_the_page() {
    // name, address lines, email — document::address_lines/email_line
}
#[test] fn the_item_table_says_quantity_and_unit_price() {}
#[test] fn the_money_block_is_still_the_shared_one() { /* #204's test, kept */ }
#[test] fn no_live_payment_link_reaches_the_pdf() { /* #204's test — MUST stay green */ }
#[test] fn the_pdf_prints_no_url_at_all() {
    // with a live stripe_payment_link_url set and the invoice published:
    // no "http", no token, no "index.html" anywhere in the text
}
#[test] fn the_payment_instructions_are_printed_under_the_foot_rule() {}
#[test] fn no_payment_instructions_draw_no_payment_heading() {}
#[test] fn a_page_that_paginates_does_not_split_a_two_column_band() {}
```

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement.** Add `vline`, `text_right`, `logo`, `wordmark` and
      an explicit `y` save/restore for the two-column bands (spec Decision 8),
      then draw the blocks. The item table gets `vline` column dividers; the
      money rows are drawn right-aligned against the Amount column's right edge
      rather than at `MARGIN_LEFT`. Amounts keep `fmt::money` — currency
      unification is TASK-87's, not this task's. **Nothing is printed under the
      Amount Due figure.** The payment instructions go under the foot rule
      beside Notes, through `document::payment_lines`.

      The logo path: `image::load_from_memory` → **flatten onto white** →
      `Image::from_dynamic_image` → `add_to_layer` with an `ImageTransform`
      whose scale fits the 60 × 16 mm box at 300 dpi. Every step returns
      `Option`, never `Result`, and `None` means the wordmark: a decode failure
      is a document without a logo, not a failed invoice. The flattening is
      unconditional for RGBA and a no-op otherwise, and one function is the only
      thing that constructs what printpdf receives, so the RGBA assertion has a
      single seam to test.
- [ ] **Step 4: Verify.** All five cargo commands. Without the `pdf` feature this
      file is not compiled — confirm both no-default-features runs pass.

---

### Task 6: the seam threads it through, and the two documents are pinned together

**Files:** `src/invoicing/render.rs`.

- [ ] **Step 1: Write failing tests** in `render.rs`'s `mod tests`:

```rust
#[cfg(feature = "pdf")]
#[test] fn the_pdf_and_the_page_carry_the_same_company_block() {
    // name, both address lines and the phone appear in both
}
#[cfg(feature = "pdf")]
#[test] fn the_pdf_and_the_page_carry_the_same_metadata_rows() {
    // every document::meta_rows() value appears in both, including "(Net 30)"
}
#[cfg(feature = "pdf")]
#[test] fn a_sparse_invoice_omits_the_same_blocks_in_both_documents() {}
#[cfg(feature = "pdf")]
#[test] fn the_same_logo_reaches_both_documents() {
    // html carries the data URI; the pdf bytes carry an image XObject
}
#[cfg(feature = "pdf")]
#[test] fn an_unusable_logo_degrades_on_both_documents_and_fails_neither() {
    // stored value is a bad data URI: the page has no <img>, the pdf draws the
    // wordmark, and render_invoice returns Ok
}
#[cfg(feature = "pdf")]
#[test] fn the_payment_instructions_reach_both_documents_or_neither() {}
#[test] fn rendering_still_writes_nothing_to_the_invoice() { /* kept */ }
```

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement.** `render_invoice` builds
      `document::company_block(branding.company, branding.company_address,
      branding.company_phone)` once and hands it to both renderers, and parses
      `branding.logo` **once** — `parse_logo(...).ok()` — so the page and the
      PDF cannot disagree about whether there is a logo, and a refusal costs one
      parse rather than two. **No caller above the seam changes** — if
      `send.rs`, `republish.rs` or `server/routes/invoices.rs` needed an edit,
      the seam grew a parameter it should not have.
- [ ] **Step 4: Verify.** All five cargo commands, and `git diff --stat` shows no
      change to `src/invoicing/send.rs`, `src/invoicing/republish.rs` or
      `src/invoicing/void.rs`.

---

### Task 7: storing and editing the letterhead — the TUI

**Files:** `src/cli/settings_manager.rs`.

- [ ] **Step 1: Write failing tests** in the existing `mod tests`, mirroring the
      behaviours already pinned there (trim, empty-clears, buffer prepopulates,
      status TTL of 3):

```rust
#[test] fn the_settings_screen_lists_address_phone_logo_and_payment_instructions_under_the_name() {}
#[test] fn editing_the_payment_instructions_saves_to_its_own_key() {}
#[test] fn a_backslash_n_in_the_payment_instructions_stores_a_real_newline() {}
#[test] fn editing_the_address_saves_to_the_company_address_key() {}
#[test] fn a_backslash_n_in_the_address_field_stores_a_real_newline() {
    // typed: "P.O. Box 1234\\nSpringfield, CA 90001"
    // stored: "P.O. Box 1234\nSpringfield, CA 90001"
}
#[test] fn reopening_the_address_field_shows_the_escape_again_not_a_raw_newline() {}
#[test] fn an_empty_address_clears_the_key() {}
#[test] fn the_logo_field_takes_a_path_and_stores_a_data_uri() {}
#[test] fn a_logo_that_is_not_a_png_or_jpeg_is_refused_on_the_status_line() {
    // and the stored key is untouched
}
#[test] fn a_missing_logo_file_is_refused_by_name() {}
#[test] fn the_menu_still_clamps_to_the_new_last_row() {}
```

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement.** `MENU_COMPANY_ADDRESS`/`MENU_COMPANY_PHONE`/
      `MENU_COMPANY_LOGO`/`MENU_PAYMENT_INSTRUCTIONS` between
      `MENU_BUSINESS_NAME` and `MENU_PASSWORD`; `MENU_LAST` moves.
      `Screen::EditingName` becomes `Screen::Editing(usize)` keyed by the
      `MENU_*` constant, and `handle_edit_name_key` becomes key-parameterised —
      one `set_metadata` call, one key chosen by the selection, replacing the
      hard-coded `"company_name"` at `:310`. `draw_main`'s special-cased name
      row generalises to a helper that any of the five text rows uses. The
      address and payment-instruction fields escape `\n` on display and unescape
      on save (spec Decision 3); the logo field reads the path, base64-encodes,
      and runs `document::parse_logo` before saving — a refusal is a status-line
      message and no write.
- [ ] **Step 4: Verify.** All five cargo commands.

---

### Task 8: storing and editing the letterhead — the API and the SPA

**Files:** `src/server/routes/settings.rs`, `src/server/testutil.rs`;
`web/apps/app/src/api/{client.ts,types.ts}`,
`web/apps/app/src/__mocks__/fake-api-client.ts`,
`web/apps/app/src/screens/settings.ts` (+ its test).

- [ ] **Step 1: Write failing tests.**

```rust
// server/routes/settings.rs
#[test] fn get_company_answers_all_five_fields() {}
#[test] fn put_company_writes_all_five_and_trims_them() {}
#[test] fn put_company_with_an_empty_field_clears_that_key() {}
#[test] fn put_company_with_a_bad_logo_is_a_400_and_writes_nothing() {
    // document::parse_logo's message, and name/address/phone unchanged
}
#[test] fn the_company_route_is_behind_the_locked_guard() {}   // testutil.rs:364 probe
#[test] fn the_old_company_name_route_is_gone() {}             // 404
```

```ts
// web: settings.test.ts
it('seeds all five fields once and does not clobber typing');
it('saves the whole letterhead in one request');
it('surfaces the server refusal for a bad logo without clearing the form');
it('reads a chosen file as a data URI before sending');
```

**The Lit lesson from this epic, which this task must not repeat:** a
`.value=${…}` binding is not type-checked, so a wrong-shaped literal reaches
the browser as an unhandled rejection while every test reports passing. Use
`satisfies` on any preview or fixture literal this task adds, and run
`npm run build`, `npm test`, `npm run lint` **and** `npm run typecheck` before
claiming the SPA half works.

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement.** Replace `PUT /api/settings/company-name` with
      `GET`/`PUT /api/settings/company` taking
      `{ name, address, phone, logo, paymentInstructions }` (camelCase on the
      wire, per the task-31.2 convention); the PUT validates `logo` through
      `document::parse_logo` **before** any `set_metadata`, and writes the five
      keys in one `unchecked_transaction` so a rejected value leaves nothing
      half-applied. Update the locked-route probe at `server/testutil.rs:364`.
      In the SPA, widen the settings panel to five controls with one Save — the
      address and the payment instructions are `wa-textarea`, since neither is a
      single line — replace `setCompanyName` with `setCompany` in `ApiClient`,
      `FetchApiClient` and the fake, and do the `FileReader` → data URI
      conversion in the screen. `StatusResponse.companyName` does **not** change
      — the sidebar and document title still want a bare name.
- [ ] **Step 4: Verify.** All five cargo commands plus the four `web/` commands.

---

### Task 9: templates from before #204 still work

**Files:** `src/invoicing/render_html.rs` (test module only).

- [ ] **Step 1: Extend #204's regression test.** #204 already checks in the
      pre-#204 stock template as a literal and asserts it loads and renders.
      Keep that test exactly as it is and add:

```rust
#[test]
fn the_pre_204_template_gains_nothing_and_loses_nothing_from_task_101() {
    let html = render_invoice_html(&brand_with(LEGACY_TEMPLATE, …), …);
    assert!(html.contains("Billed to: Acme"));
    assert!(html.contains("Due: 2026-09-05"), "the old {{DUE}} shape, unchanged");
    assert!(!html.contains("(Net 30)"), "{{DUE}} did not gain the parenthetical");
    assert!(html.contains("<h3>Terms</h3>"), "the old {{TERMS}} block, unchanged");
    assert!(!html.contains("{{"), "no unexpanded placeholder: {html}");
}
#[test]
fn required_is_still_exactly_four_keys() {
    assert_eq!(REQUIRED, &["NUMBER", "CLIENT", "ROWS", "TOTAL"]);
    assert_eq!(REQUIRED_ALTERNATIVES, &[("TOTAL", "TOTALS")]);
}
```

- [ ] **Step 2: Verify.** Confirm the first test would fail if `{{DUE}}` were
      changed to fold terms, and the second if any key joined `REQUIRED` — try
      each locally, watch it fail, then put it back.
- [ ] **Step 3: Verify.** All five cargo commands.

---

### Task 10: the side-by-side review — **HALT**

AC #9: a rendered example of both documents, reviewed side by side, before this
is called done. Nothing after this task starts until Sam has looked.

- [ ] **Step 1: Build the four fixtures** in a scratch data directory. `preview`
      needs no invoicing config and makes no network call. Set a real
      **transparent, wider-than-tall PNG** logo through the settings screen —
      the shape that reproduces printpdf's soft-mask defect — and multi-line
      payment instructions.

```bash
SCRATCH=$(mktemp -d)
cargo run -- init --data-dir "$SCRATCH"
# house      : transparent PNG logo, company address + phone, payment
#              instructions, two-line client address, email, due date +
#              "Net 30", three items, notes, a Stripe link
# sparse     : none of the above, one item, draft
# long-terms : due date + multi-line terms
# part-paid  : the house one with half the total recorded
cargo run -- invoice preview 1248 --output-dir /tmp/task-101   # …1249, 1250, 1251
```

- [ ] **Step 2: Check the sparse pair yourself first.** No empty label, no
      orphan rule, no `From` heading over nothing, no bare `ph.`, no `Payment`
      heading over nothing, no `<br>` with nothing after it, no empty `<p>` or
      `<h3>`, in either document. A failure here is a bug, not a review note.
- [ ] **Step 3: Check the two documents against each other** for the house
      fixture — same company block, same metadata values, same client block,
      same money lines, same column headings, same payment instructions, and the
      **same logo, drawn undistorted and not overlapping the From block**. Any
      difference is a bug.
- [ ] **Step 4: Hand over the eight paths** and **stop**. Do not proceed past
      this task. The PR body carries the reproduction steps and a field-by-field
      description of both documents; the side-by-side judgement is Sam's, not
      the implementer's. Name what is still open: the TUI's `\n` escape, the
      retired `company-name` route, and ISO dates.
- [ ] **Step 5: Apply what comes back**, re-render, and confirm.

---

### Task 11: documentation

- [ ] **Step 1: `docs/invoicing.md` — the "Placeholders" table.** Rows for
      `{{LOGO}}`, `{{META_ROWS}}`, `{{TERMS_BLOCK}}`, `{{PAYMENT_BLOCK}}` (each
      *fragment*, each with its "empty when"), and `{{COMPANY_ADDRESS}}`/
      `{{COMPANY_PHONE}}`/`{{PAYMENT_INSTRUCTIONS}}` (*text*). Amend
      `{{COMPANY_BLOCK}}`'s row: it is the whole From block now. Note that
      `{{DUE}}`, `{{DUE_DATE}}`, `{{TERMS}}` and `{{CONTACT}}` are unchanged and
      remain available, and that the stock page uses `{{META_ROWS}}`,
      `{{TERMS_BLOCK}}` and `{{PAYMENT_BLOCK}}` instead.
- [ ] **Step 2: `docs/invoicing.md` — a "Letterhead" section.** The five
      metadata keys, where they are edited (TUI settings screen, web settings
      screen), the logo's accepted types and size cap, **and the Gmail caveat**:
      the logo is inlined as a data URI, Gmail does not render those, so a Gmail
      reader sees the company name as alt text in the email body while the
      attachment beside it carries the real image — and an operator who needs a
      hosted image in the body can put an absolute `<img src="https://…">` in
      their own `templates/invoice.html`.
- [ ] **Step 3: `docs/invoicing.md` — the payment instructions and the PDF.**
      Say plainly that the stock page **no longer hardcodes a bank-transfer
      paragraph**: an installation that wants one sets `payment_instructions`,
      and one that takes no bank transfers now prints nothing. Then the PDF
      section: what it carries, that it embeds the **real logo** (with the
      measured cost of `embedded_images` and the flattening that makes
      printpdf's alpha soft-mask defect unreachable), that a logo it cannot use
      falls back to a text wordmark and never fails a send, and that it carries
      **no payment link and no URL of any kind**.
- [ ] **Step 4: `docs/api.md`** — `GET`/`PUT /api/settings/company` replaces
      `PUT /api/settings/company-name`.
- [ ] **Step 5: `CLAUDE.md`.**
      - Invoicing bullet: `document.rs` gains `CompanyBlock`/`company_block`,
        `MetaRow`/`meta_rows`, `due_value`/`terms_block_text`, `parse_logo` and
        `payment_lines`; `Branding` carries the letterhead; the five metadata
        keys; `cli::invoice::company_profile` as the one resolver.
      - Settings Manager bullet: the four new rows and the `\n` escape.
      - Settings API bullet: the replaced route.
      - Key Design Constraints: replace the existing logo sentence — **both**
        documents carry the real logo; `printpdf`'s `embedded_images` is on and
        its price is recorded as a measured number; RGBA is flattened onto white
        before printpdf sees it, so the width-sized soft mask is unreachable;
        and any unusable logo degrades to a text wordmark rather than failing a
        render or a send.
      - Key Design Constraints: record Sam's ruling — the PDF carries no
        live payment link **and no page URL**, because an emailed attachment
        cannot be recalled, nothing deactivates a Stripe link on settlement, and
        a tokenized URL printed as unclickable text is noise.
      - Key Design Constraints: payment instructions are one configurable block
        rendered on both documents or on neither, and `{{CONTACT}}` keeps its
        meaning while leaving the stock documents.
      - Key Design Constraints: the page and the PDF draw the From block, the
        metadata rows and the money rows from the same `document.rs` functions,
        so they cannot disagree; a missing value omits its block; `REQUIRED`
        never grows.
- [ ] **Step 6: `README.md`** — only if it describes what an invoice looks like.
- [ ] **Step 7: Verify.** `git diff --stat`, then all five cargo commands and the
      four `web/` commands one last time.
- [ ] **Open the PR** against main, titled
      "Invoicing: the house invoice layout on the page and the PDF (TASK-101)",
      with the reproduction steps and a field-by-field description of both
      documents in the body. **Do not merge** — AC #9 is Sam's.

---

## Final verification

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test -- --test-threads=1`
- [ ] `cargo test --no-default-features -- --test-threads=1`
- [ ] `cargo test --no-default-features --features serve -- --test-threads=1`
- [ ] In `web/`: `npm run lint`, `npm run typecheck`, `npm test`, `npm run build`
- [ ] `git grep -n 'REQUIRED' src/invoicing/render_html.rs` shows the same four
      keys it always had, and `REQUIRED_ALTERNATIVES` its one pair.
- [ ] `git grep -n 'stripe_payment_link_url' src/pdf.rs` finds nothing outside a
      test asserting its absence, and no URL of any kind is printed.
- [ ] `cargo tree -p base64` and the lockfile confirm no crate was added for it.
- [ ] The release-binary delta from `embedded_images` is measured and reported.
- [ ] `git diff --stat src/invoicing/send.rs src/invoicing/republish.rs src/invoicing/void.rs`
      is empty.
- [ ] Sam has seen the four rendered pairs (Task 10).

## Acceptance criteria mapping

| AC | Verified by |
|---|---|
| #1 both documents render the house layout | Task 4 (`the_stock_page_carries_all_seven_house_blocks`), Task 5 (`a_configured_logo_is_embedded_as_an_image`, `the_from_block_carries_the_address_and_the_phone`, `the_metadata_column_is_the_shared_one`), Task 6, Task 10 |
| #2 company address and phone configurable, resolved from one place, on both documents | Task 1 (`company_block`), Task 2 (`company_profile` — one resolver, every site), Tasks 7 and 8 (the editors), Task 6 (`the_pdf_and_the_page_carry_the_same_company_block`) |
| #3 a logo is configurable and appears on the page; the PDF's treatment is decided and documented | Task 1 (`parse_logo`), Task 3 (`the_logo_is_an_img_with_the_company_name_as_its_alt_text`), Task 5 (`a_configured_logo_is_embedded_as_an_image`, `nothing_handed_to_printpdf_is_ever_rgba`, `an_unusable_logo_falls_back_to_the_wordmark_rather_than_failing`), Task 6 (`the_same_logo_reaches_both_documents`), Task 11 Steps 3 and 5 (spec Decision 2 — **reversed on the record, with the cost measured**) |
| #4 due date renders with its terms when set, without them when not | Task 1 (`single_line_terms_ride_beside_the_due_date`, `multi_line_terms_stay_a_block_rather_than_a_parenthetical`), Task 3, Task 6 (`the_pdf_and_the_page_carry_the_same_metadata_rows`) |
| #5 a missing value omits its block | Task 1 (`an_unset_company_block_says_nothing_at_all`), Task 3 (`the_from_block_omits_the_lines_it_does_not_have`), Task 4 (`the_stock_page_of_a_sparse_invoice_prints_no_empty_labels`), Task 5 (`an_unset_company_draws_no_from_block`), Task 6, Task 10 Step 2 |
| #6 the page and the PDF agree on every figure and every block they both carry | Task 6, all four parity tests; Task 10 Step 3 |
| #7 a custom template exported before this change still loads, and `REQUIRED` does not grow | Task 9, and the global constraint |
| #8 the pay-link-in-PDF rule is upheld or reversed on the record, with reasoning | **Upheld, and widened** — spec Decision 5 (Sam's ruling): no live link *and* no page URL. Task 5 (`no_live_payment_link_reaches_the_pdf`, `the_pdf_prints_no_url_at_all`), Task 11 Steps 3 and 5 |
| #9 a rendered example of both documents reviewed side by side | Task 10 (HALT) |
| #10 payment instructions are configurable text, not a sentence hardcoded in the stock template | Task 1 (`payment_lines`), Task 3 (`the_payment_block_is_the_configured_text_one_line_per_line`), Task 4 (`the_stock_page_no_longer_hardcodes_a_bank_transfer_paragraph`), Tasks 7 and 8 (the editors) |
| #11 payment instructions render on both the page and the PDF, or on neither | Task 5 (`the_payment_instructions_are_printed_under_the_foot_rule`), Task 6 (`the_payment_instructions_reach_both_documents_or_neither`) |
| #12 an installation that takes no bank transfers can omit the block entirely | Task 3 (`no_payment_instructions_render_no_payment_block`), Task 4 (`the_stock_page_of_a_sparse_invoice_prints_no_empty_labels`), Task 5 (`no_payment_instructions_draw_no_payment_heading`), Task 10 Step 2 |
