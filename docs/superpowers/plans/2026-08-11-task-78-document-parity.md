# Document field parity — TASK-78 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Spec: `docs/superpowers/specs/2026-08-11-task-78-document-parity-design.md`.
Read it first — every "why" below lives there.

**Goal:** the published page and the emailed PDF carry the same facts about the
same invoice — company, client address, client email, and one money block whose
lines are decided in one place — with a null value omitting its block rather
than printing an empty label, and every custom template exported before this
change still loading and rendering unchanged.

**Architecture:** a new `src/invoicing/document.rs` owns `MoneySummary` /
`MoneyLine` — the figures and the rules about which lines appear — and both
renderers consume it, which is what makes the two documents agree by
construction rather than by review. `render_html.rs` gains four *fragment*
placeholders (`COMPANY_BLOCK`, `CLIENT_ADDRESS_BLOCK`, `CLIENT_EMAIL_BLOCK`,
`TOTALS`) because a single-pass expander with no conditionals can only omit a
block by being handed an empty one; the seven previously-unused text keys keep
their meanings. `render_invoice` loads `paid_amount` alongside the line items it
already loads, so preview, send, the API preview routes and TASK-64's republish
all pick up the new figures with no signature change above the seam.
`render_invoice_pdf` gains the client address, the client email, the shared
money lines, and the pay URL.

**Tech Stack:** Rust, rusqlite, printpdf (feature-gated, with `pdf::extract_text`
as the assertion seam), the embedded HTML template.

**This lands as PR-2b, before TASK-64.** TASK-64's republish is only meaningful
once the documents show what has been paid; the Paid/Balance rows are here, in
the task that owns document design and carries the side-by-side review.

## Global Constraints

- After every task, all four green:
  - `cargo test -- --test-threads=1`
  - `cargo test --no-default-features --features gusto -- --test-threads=1` and
    `cargo test --no-default-features -- --test-threads=1` — the PDF half of
    every change is `pdf`-gated, and the HTML half must be exercised in both.
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt --check`
- **TDD, always.** Failing test first, watched failing for the right reason.
- **`REQUIRED` does not grow.** `{{NUMBER}}`, `{{CLIENT}}`, `{{ROWS}}`,
  `{{TOTAL}}` remain the whole requirement, so no template written before this
  change can stop validating.
- No placeholder changes meaning. `COMPANY`, `CLIENT_ADDRESS`, `CLIENT_EMAIL`,
  `DUE_DATE`, `SUBTOTAL`, `TAX`, `PAY_URL` keep rendering exactly what they
  render today.
- `expand` stays single-pass and every value stays escaped on the way in. The
  template itself is still never sanitized.
- `src/invoicing/` reads no settings and reaches into no `src/cli/`.
- The seam's shape does not change: `render_invoice(conn, invoice, client, pay,
  branding)` is what `preview`, `send` and the API call, and it stays that way.

---

### Task 1: `src/invoicing/document.rs` — one money vocabulary

**Files:** create `src/invoicing/document.rs`; modify `src/invoicing/mod.rs`
(`pub mod document;`, alphabetically before `gateway`).

**Interface produced** (consumed by Tasks 2, 3 and 5): `MoneyLine`,
`MoneySummary`, `MoneySummary::of`, `MoneySummary::lines`, and
`address_lines(&str) -> Vec<&str>`.

- [ ] **Step 1: Write failing tests** in a new `mod tests`:

```rust
#[test] fn an_untaxed_unpaid_invoice_prints_one_line() {
    let lines = MoneySummary::of(&invoice(100.0, 0.0), 0.0).lines();
    assert_eq!(labels(&lines), vec!["Total"]);
    assert!(lines[0].emphasis);
}

#[test] fn tax_brings_the_subtotal_with_it() {
    let lines = MoneySummary::of(&invoice(108.25, 8.25), 0.0).lines();
    assert_eq!(labels(&lines), vec!["Subtotal", "Tax", "Total"]);
}

#[test] fn a_payment_brings_paid_and_balance() {
    let lines = MoneySummary::of(&invoice(100.0, 0.0), 40.0).lines();
    assert_eq!(labels(&lines), vec!["Total", "Paid", "Balance due"]);
    assert_eq!(lines[2].amount, 60.0);
    assert!(lines[2].emphasis, "the balance is what a client looks for");
}

#[test] fn a_settled_invoice_shows_a_zero_balance_rather_than_hiding_it() {
    let lines = MoneySummary::of(&invoice(100.0, 0.0), 100.0).lines();
    assert_eq!(labels(&lines), vec!["Total", "Paid", "Balance due"]);
    assert_eq!(lines[2].amount, 0.0);
}

#[test] fn an_address_is_split_into_the_lines_it_was_typed_as() {
    assert_eq!(address_lines("123 Main St\nSpringfield, IL"),
               vec!["123 Main St", "Springfield, IL"]);
    assert!(address_lines("  \n \n").is_empty(), "blank lines say nothing");
    assert!(address_lines("").is_empty());
}
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib invoicing::document 2>&1 | tail -20`
- [ ] **Step 3: Implement** per the spec's Decision 2 table. `balance` is
      `total - paid`; the half-cent slack `refresh_status` uses is not needed
      here (nothing branches on equality) but a `-0.00` must never print, so
      clamp a balance within half a cent of zero to `0.0`.
- [ ] **Step 4: Verify.** All four commands.

---

### Task 2: the HTML renderer's new fragments

**Files:** `src/invoicing/render_html.rs`.

**Interface produced** (consumed by Tasks 3 and 5):

```rust
pub fn render_invoice_html(
    branding: &Branding<'_>, invoice: &Invoice, client: &Client,
    items: &[InvoiceLineItem], money: &MoneySummary, pay: PayButton<'_>,
) -> String;
```

- [ ] **Step 1: Write failing tests** in the existing `mod tests` (the
      `sample()`, `brand()`, `brand_with()` helpers are already there). First
      migrate every existing test to the new signature with **no change to any
      assertion** — that invariance is the point. Then add:

```rust
#[test] fn the_company_block_carries_the_name_and_disappears_without_one() {}
#[test] fn the_client_address_block_is_br_joined_and_escaped() {
    // "123 <Main> St\nSpringfield" → "<br>123 &lt;Main&gt; St<br>Springfield"
}
#[test] fn an_absent_address_or_email_renders_nothing_at_all() {
    // no stray <br>, no empty <p>, no label
}
#[test] fn the_totals_fragment_is_table_rows_in_the_shared_order() {
    // labels and order match MoneySummary::lines(), emphasis carries class="total"
}
#[test] fn every_placeholder_in_the_vocabulary_still_expands() {
    // the existing test, now over 22 keys
}
#[test] fn the_bare_text_keys_did_not_change_meaning() {
    // {{SUBTOTAL}}/{{TAX}}/{{COMPANY}}/{{CLIENT_ADDRESS}}/{{CLIENT_EMAIL}}
    // render exactly what they rendered before
}
```

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement.** Add the four keys to `PLACEHOLDERS` (not to
      `REQUIRED`), build each fragment beside the existing `block()` closure,
      and render `{{TOTALS}}` from `money.lines()` as
      `<tr><td colspan="3">Label</td><td>USD 250.00</td></tr>`, the emphasised
      rows carrying `class="total"`. Amount formatting stays the page's
      existing `{{CURRENCY}} {:.2}` convention (spec Decision 3).
- [ ] **Step 4: Verify.** All four commands.

---

### Task 3: the stock template

**Files:** `src/invoicing/templates/invoice.html`.

- [ ] **Step 1: Write the failing tests** in `render_html.rs`'s `mod tests`,
      against `DEFAULT_TEMPLATE`:

```rust
#[test] fn the_stock_page_shows_the_company_the_address_and_the_email() {}
#[test] fn the_stock_page_of_a_sparse_invoice_prints_no_empty_labels() {
    // no company, no address, no email, no tax, no due, no notes, no terms:
    // assert no "<p></p>", no "<h3></h3>", no "Due:", no double "<br><br>"
}
#[test] fn the_stock_page_and_the_money_lines_agree() {
    // every label from MoneySummary::lines() appears once, in order
}
#[test] fn the_default_template_still_validates() { /* existing test */ }
```

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement** the template from the spec's Decision 4 — company
      block above the heading, address and email inside the "Billed to"
      paragraph, `{{TOTALS}}` in a `<tfoot>` of the line-item table, `{{PAY}}`
      unmoved. Add only the CSS rules the new blocks need.
- [ ] **Step 4: Verify.** All four commands.

---

### Task 4: the PDF

**Files:** `src/pdf.rs`.

**Interface produced:**

```rust
pub fn render_invoice_pdf(
    invoice: &Invoice, client: &Client, items: &[InvoiceLineItem],
    company: &str, money: &MoneySummary, pay: PayButton<'_>,
) -> Result<Vec<u8>>;
```

- [ ] **Step 1: Write failing tests** in `mod invoice_pdf_tests` (gated on
      `pdf`), using `extract_text`:

```rust
#[test] fn the_client_block_carries_the_address_and_the_email() {
    // order: Invoice #1248 < company < "Billed to: Acme" < address < email < Issued
}
#[test] fn an_absent_address_or_email_draws_no_line() {
    // the sparse client renders neither, and "Issued:" still follows the name
}
#[test] fn the_money_block_is_the_shared_one() {
    // labels and order == MoneySummary::lines()
}
#[test] fn a_paid_invoice_shows_paid_and_the_balance() {}
#[test] fn a_live_payment_link_is_printed_as_a_url() {
    // PayButton::Link → "Pay online: https://pay.stripe.test/x"
}
#[test] fn a_placeholder_or_omitted_button_prints_nothing() {}
```

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement.** Address lines through `document::address_lines`,
      each drawn at `SUBTITLE_SIZE` under `Billed to:`; the money rows from
      `money.lines()` replacing the inline `tax != 0.0` block (delete it — the
      rule now lives in one place); the pay line after the money block. Amounts
      keep `fmt::money` (spec Decision 3).
- [ ] **Step 4: Verify.** All four commands. Without the `pdf` feature this file
      is not compiled — confirm the other build still passes.

---

### Task 5: the seam supplies the figures

**Files:** `src/invoicing/render.rs`.

- [ ] **Step 1: Write failing tests** in `render.rs`'s `mod tests`:

```rust
#[test] fn the_seam_reads_the_payments_and_the_page_shows_them() {
    // record 40.00 against a 100.00 invoice, render, assert the html carries
    // "Paid" and "60.00"
}
#[cfg(feature = "pdf")]
#[test] fn the_pdf_and_the_page_carry_the_same_money_labels() {
    // extract_text(pdf) and html contain the same set of labels
}
#[test] fn rendering_still_writes_nothing_to_the_invoice() { /* existing test */ }
```

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement.** `render_invoice` calls
      `invoices::paid_amount(conn, invoice.id)?`, builds
      `MoneySummary::of(invoice, paid)`, and passes a reference to both
      renderers. **No caller above the seam changes** — that is the property
      being preserved; if `cli/invoice.rs`, `send.rs` or
      `server/routes/invoices.rs` needed an edit, the seam grew a parameter it
      should not have.
- [ ] **Step 4: Verify.** All four commands, and `git diff --stat` shows no
      change to `src/cli/invoice.rs`, `src/invoicing/send.rs` or
      `src/server/routes/invoices.rs`.

---

### Task 6: templates from before this change still work

**Files:** `src/invoicing/render_html.rs` (test module only).

- [ ] **Step 1: Write the test.** Paste the *pre-change* stock template into the
      test module as a `const LEGACY_TEMPLATE: &str` — the 17-line page with
      `Billed to: {{CLIENT}}`, `Total: {{CURRENCY}} {{TOTAL}}` and no
      `{{TOTALS}}` — and assert:

```rust
#[test]
fn a_template_exported_before_the_field_parity_change_still_loads_and_renders() {
    let dir = tempfile::tempdir().unwrap();
    write_override(dir.path(), LEGACY_TEMPLATE);
    let loaded = load_template(dir.path()).expect("an older export must keep working");
    let html = render_invoice_html(&brand_with(&loaded, "b@e.test"), &inv, &client,
                                   &items, &money, PayButton::Omitted);
    assert!(html.contains("Billed to: Acme"));
    assert!(html.contains("250.00"));
    assert!(!html.contains("{{"), "no unexpanded placeholder: {html}");
}
```

- [ ] **Step 2: Verify it passes**, and that it would fail if a new key were
      added to `REQUIRED` — try it locally, then put `REQUIRED` back.
- [ ] **Step 3: Verify.** All four commands.

---

### Task 7: the side-by-side review — **HALT**

AC #7: a rendered example of both documents, reviewed side by side, before this
is called done. Nothing after this task starts until Sam has looked.

- [ ] **Step 1: Build the three fixtures** in a scratch data directory (no
      invoicing config needed — `preview` requires none):

```bash
export NIGEL_DATA_DIR_SCRATCH=$(mktemp -d)      # or a --data-dir init
cargo run -- init --data-dir "$NIGEL_DATA_DIR_SCRATCH"
# rich: company set via the settings screen or metadata, two-line address,
#       email, tax, due date, notes, terms, three items, a payment link
# sparse: no company, no address, no email, no tax, no due, no notes/terms,
#       one item, draft
# part-paid: the rich one with half the total recorded
cargo run -- invoice preview 1248 --output-dir /tmp/task-78
cargo run -- invoice preview 1249 --output-dir /tmp/task-78
cargo run -- invoice preview 1250 --output-dir /tmp/task-78
```

- [ ] **Step 2: Check the sparse pair yourself first** — no empty label, no
      empty `<p>`, no stray `<br>`, no heading with nothing under it, in either
      document. A failure here is a bug, not a review note.
- [ ] **Step 3: Hand over the six paths** and stop. Name the open questions the
      spec raises that the review should settle: the currency glyph
      (`USD 250.00` on the page vs `$250.00` in the PDF), the pay URL in the
      PDF, and the company block's position.
- [ ] **Step 4: Apply what comes back**, re-render, and confirm.

---

### Task 8: documentation

- [ ] **Step 1: `docs/invoicing.md` — "Placeholders" table.** Four new rows
      (`COMPANY_BLOCK`, `CLIENT_ADDRESS_BLOCK`, `CLIENT_EMAIL_BLOCK`, `TOTALS`),
      each marked *fragment*, each with its "empty when" condition. Add a
      sentence under the table: the text keys are the escaped values an author
      can place themselves, the fragment keys are the blocks that vanish when
      there is nothing to say, and `{{TOTAL}}` remains required and available
      even though the stock page now prints it inside `{{TOTALS}}`.
- [ ] **Step 2: `docs/invoicing.md` — "Sending" / "Customizing the invoice
      PDF".** Describe what each document now carries, and state the rule that
      decides the money lines once, so the two cannot disagree. Note that a
      template exported from an older Nigel keeps working and gains nothing
      until it is edited.
- [ ] **Step 3: `CLAUDE.md`** — amend the Invoicing bullet: `document.rs`
      (`MoneySummary`/`MoneyLine` — the one place that decides which money lines
      a document prints, consumed by both renderers), the four new fragment
      placeholders, `render_invoice` loading `paid_amount` beside the line
      items, and the PDF's new client block and pay line. Add to Key Design
      Constraints: *the page and the PDF print the same money lines because both
      ask `MoneySummary::lines()`; a null value omits its block, never printing
      an empty label; and `REQUIRED` never grows, so a custom template exported
      before a vocabulary change keeps rendering.*
- [ ] **Step 4: `README.md`** — only if it describes what an invoice looks like.
- [ ] **Step 5: Verify.** `git diff --stat`, then all four commands one last
      time.
- [ ] **Open PR-2b**, with the three rendered pairs attached or linked.

---

## Final verification

- [ ] `cargo test -- --test-threads=1`
- [ ] `cargo test --no-default-features --features gusto -- --test-threads=1`
- [ ] `cargo test --no-default-features -- --test-threads=1`
- [ ] `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`
- [ ] `git diff src/invoicing/render.rs` shows one added `paid_amount` call and
      the two calls that pass it on — nothing else.
- [ ] `git grep -n 'REQUIRED' src/invoicing/render_html.rs` shows the same four
      keys it always had.
- [ ] Sam has seen the three rendered pairs (Task 7).

## Acceptance criteria mapping

| AC | Verified by |
|---|---|
| #1 the page renders the company block, the client address and the client email when present | Task 2, Task 3 (`the_stock_page_shows_the_company_the_address_and_the_email`) |
| #2 the page and the PDF agree on subtotal, tax and total | Task 1 (one `lines()`), Task 2, Task 4 (`the_money_block_is_the_shared_one`), Task 5 (`the_pdf_and_the_page_carry_the_same_money_labels`) |
| #3 the PDF renders the company name, the client address and the client email | Task 4 (`the_client_block_carries_the_address_and_the_email`; the company half already shipped in TASK-68.3 and is covered by `the_company_name_heads_the_document`) |
| #4 a null or empty value omits its block | Task 2 (`an_absent_address_or_email_renders_nothing_at_all`), Task 3 (`the_stock_page_of_a_sparse_invoice_prints_no_empty_labels`), Task 4 (`an_absent_address_or_email_draws_no_line`), Task 7 Step 2 |
| #5 a custom template exported before this change still loads and renders | Task 6, and the `REQUIRED`-does-not-grow constraint |
| #6 the pay link placement is settled deliberately | Task 3 (`{{PAY}}` stays put, `{{PAY_URL}}` stays the author's hook), Task 4 (`a_live_payment_link_is_printed_as_a_url`) |
| #7 a rendered example of both documents reviewed side by side | Task 7 (HALT) |
</content>
