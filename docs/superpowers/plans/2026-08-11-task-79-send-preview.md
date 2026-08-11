# Preview in the send flow — TASK-79 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Spec: `docs/superpowers/specs/2026-08-11-task-79-send-preview-design.md`.
Read it first — every "why" below lives there.

**Goal:** neither surface can send an invoice without showing the operator what
the client is about to receive. `nigel invoice send` renders the document,
writes it beside the invoice, states the recipient and the total, and asks —
with `--yes` to skip, exactly as `void` does. The web send dialog frames the
rendered page before the confirm button, with the PDF offered beside it, and
says up front when this build cannot send at all.

**Architecture:** no new render path. The CLI reuses the seam and the preview
writer (extracted out of `cli::invoice::preview` so both callers share the
paths, the permissions and the wording). The web extracts the iframe out of
`wc-invoice-preview` into a new `wc-document-frame`, and the send dialog takes
the page's HTML — fetched through a new `ApiClient.invoicePreviewHtml` — as
`srcdoc`, so a broken custom template arrives as a sentence rather than as a
JSON envelope rendered in a box.

**Tech Stack:** Rust, clap, rusqlite, assert_cmd/predicates/tempfile; Lit 3,
Web Awesome `wa-dialog`, vitest + jsdom + axe, the `@nigel/ui` preview harness.

**This lands as PR-2c'/PR-2d, last in stream 2.** It is independent of TASK-78
and TASK-64 in code, but it is the surface that shows their output, so
reviewing it last means reviewing the finished document.

## Global Constraints

- Rust, after every Rust task:
  - `cargo test -- --test-threads=1`
  - `cargo test --no-default-features --features gusto -- --test-threads=1` and
    `cargo test --no-default-features -- --test-threads=1` — **AC #6 lives in
    the second build**: no PDF is written, the notice is printed, the send is
    refused before any network call, and the exit status is still meaningful.
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt --check`
- Web, after every web task: `cd web && npm test && npm run lint && npm run typecheck && npm run build`.
- **TDD, always.** Failing test first, watched failing for the right reason.
- **One render path.** Everything shown comes from
  `invoicing::render::render_invoice` (CLI) or `GET /api/invoices/{n}/preview`,
  which is that seam behind a route. Any second renderer is the bug this task
  exists to prevent.
- **Nothing new may reach the network at preview time.** No gateway is
  constructed on the preview path, and the CLI's confirmation happens before
  `build_clients` is called.
- **Component-first, per CLAUDE.md's mandatory checklist.** A new `wc-*`
  component is three files (`wc-foo.ts`, `wc-foo.preview.ts`, `wc-foo.test.ts`)
  plus a line in `web/packages/ui/src/components/index.ts`, with
  `describePreviewA11y(preview)` as the last statement of the test file and a
  preview state for every visible state.
- **No address outside `src/api`.** `web/apps/app/src/__tests__/api-seam.test.ts`
  fails the build on a quoted `/api/` literal or a bare `fetch(` anywhere else.
  The fake client in `web/apps/app/src/__mocks__/fake-api-client.ts` gains every
  method the real one does.

---

# Part A — the CLI

### Task 1: extract the preview writer

**Files:** `src/cli/invoice.rs`.

**Interface produced** (consumed by Task 2):

```rust
fn write_preview(rendered: &RenderedInvoice, number: i64, output_dir: Option<String>) -> Result<()>;
```

- [ ] **Step 1: Write the failing test** in `mod tests`:

```rust
#[test]
fn write_preview_puts_both_artifacts_where_preview_paths_says() {
    // render nothing real: a RenderedInvoice { html: "<p>x</p>", pdf: Some(b"%PDF".to_vec()) }
    // into a temp dir, assert both files exist with the stable undated names
    // and that a `pdf: None` render writes the html and no pdf
}
```

- [ ] **Step 2: Verify it fails.**
- [ ] **Step 3: Implement** by moving the second half of `preview()` into
      `write_preview` verbatim — the `create_dir_all`, the default-only
      `restrict_dir_permissions`, `preview_paths`, `restrict_file_permissions`,
      the `Wrote <path>` lines and the `PDF_DISABLED_MESSAGE` notice. `preview()`
      becomes resolve → notice → render → `write_preview`.
- [ ] **Step 4: Verify.** `tests/cli_dispatch.rs`'s seven existing
      `invoice_preview_*` tests must pass **untouched** — that invariance is the
      proof the extraction changed nothing.
- [ ] **Step 5: Verify.** All four commands.

---

### Task 2: `send` renders, shows, and asks

**Files:** `src/cli/invoice.rs` (`send`, plus two small pure helpers),
`src/cli/mod.rs`, `src/main.rs`.

**Interfaces** (the first two are pure and unit-tested):

```rust
fn send_summary(invoice: &Invoice, client: &Client) -> String;
fn send_consequences(invoice: &Invoice, client: &Client, publish_host: Option<&str>) -> String;
fn confirm_send(invoice: &Invoice, yes: bool) -> Result<bool>;
pub fn send(number: i64, today: &str, yes: bool) -> Result<()>;
```

- [ ] **Step 1: Write failing unit tests.**

```rust
#[test] fn the_summary_names_the_client_the_total_and_the_recipient() {
    // "Invoice #1248 — Acme Co, $1,850.00 USD, issued 2026-08-04." plus the address
}
#[test] fn a_client_with_no_email_is_still_summarised() {
    // the precheck refuses later; the summary must not panic or print "None"
}
#[test] fn a_resend_says_the_link_is_reused_and_the_page_republished() {
    // published_at set → the TUI's re-send wording
}
#[test] fn the_consequences_name_the_publish_host_when_there_is_one() {}
```

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement `send`** in the spec's order: `find_invoice`,
      `ensure_not_void`, `get_client`, `load_template` (unmoved — it is what
      makes AC #7 true), render through the seam with
      `render::pay_button_for(&invoice)` and `contact_email_for_preview`, then:

```rust
if !yes {
    println!("{}", send_summary(&invoice, &client));
    write_preview(&rendered, number, None)?;
    println!("{}", send_consequences(&invoice, &client, host.as_deref()));
    if !confirm_send(&invoice, yes)? {
        println!("Aborted.");
        return Ok(());
    }
}
let (stripe, r2, mail) = build_clients(invoicing_config())?;
```

      `confirm_send` is `confirm_void`'s twin: `--yes` returns true, a non-TTY
      without it is
      `Refusing to send invoice #1248 without confirmation. Pass --yes.`,
      otherwise `Send it? [y/N]`.
- [ ] **Step 4: Add the flag** in `src/cli/mod.rs`'s `InvoiceCommands::Send`,
      worded exactly as `Void`'s: *Send without confirmation (required when
      stdin is not a TTY)*.
- [ ] **Step 5: Dispatch** the new argument in `src/main.rs`.
- [ ] **Step 6: Verify.** All four commands, plus `cargo run -- invoice send --help`.

---

### Task 3: CLI end-to-end

**Files:** `tests/cli_dispatch.rs`.

`TestEnv` clears the nine `NIGEL_*` variables, so an unconfigured send fails at
`build_clients` — *after* the confirmation, which is what several of these
assert.

- [ ] **Step 1: Write the tests.**

```rust
#[test] fn invoice_send_without_yes_refuses_on_a_non_tty_and_sends_nothing() {
    // stderr names --yes; the invoice is still a draft; no network (TEST_TIMEOUT)
}
#[test] fn invoice_send_renders_a_preview_before_it_asks() {
    // with stdin closed the command refuses, and previews/invoice-1248.html
    // must NOT have been written — the refusal comes first
}
#[test] fn invoice_send_with_a_broken_template_fails_before_asking_anything() {
    // write an invalid <data_dir>/templates/invoice.html → failure naming the path
}
#[test] fn invoice_send_with_yes_and_no_config_fails_at_the_config_step() {
    // proves --yes reaches build_clients and writes no preview artifacts
}
#[cfg(not(feature = "pdf"))]
#[test] fn invoice_send_without_the_pdf_feature_says_why_before_it_fails() {
    // the PDF_DISABLED sentence, and the send refusal, not a panic
}
```

      A y/N prompt cannot be answered from `assert_cmd` without a pty, so the
      interactive path is covered by the unit tests on `send_summary` /
      `send_consequences` / `confirm_send` and by the manual pass — the same
      split `void`'s tests already use.

- [ ] **Step 2: Verify they fail, then pass.**
- [ ] **Step 3: Verify.** Both feature builds of `--test cli_dispatch`.

---

# Part B — the web

### Task 4: `wc-document-frame`

**Files:** create `web/packages/ui/src/components/wc-document-frame.ts`,
`.preview.ts`, `.test.ts`; modify `web/packages/ui/src/components/index.ts`.

- [ ] **Step 1: Write the failing tests.**

```ts
it('frames srcdoc without granting it the app origin', () => {
  // sandbox === PREVIEW_SANDBOX, and the constant contains neither
  // 'allow-same-origin' nor 'allow-scripts'
});
it('frames a src when given one instead', () => {});
it('shows a spinner until the frame loads', () => {});
it('labels the frame for a screen reader', () => {});
```

      plus `describePreviewA11y(preview)` last, and a `.preview.ts` declaring
      `loading`, `loaded-srcdoc` and `loaded-src` (the `data:text/html` page
      `wc-invoice-preview.preview.ts` already uses is the harness-safe source).

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement** per the spec's Decision 1: `srcdoc`, `src`, `label`,
      `height` (→ `--nc-document-frame-height`, default `32rem`), the
      `loaded` state and the spinner overlay. `PREVIEW_SANDBOX` moves here and
      is re-exported from `wc-invoice-preview` so nothing importing it breaks.
- [ ] **Step 4: Verify.** The web four.

---

### Task 5: `wc-invoice-preview` delegates the frame

**Files:** `web/packages/ui/src/components/wc-invoice-preview.ts`.

- [ ] **Step 1: Run its existing suite** and note the six passing specs and four
      preview states. They are the test for this task: **none of them may
      change.**
- [ ] **Step 2: Implement.** `renderFrame()` returns
      `<wc-document-frame .src=${this.src} label="Invoice preview">`; the
      lazy-creation rule (no frame until the disclosure is open), the reset of
      `loaded` on close, the links and the missing-config notice all stay where
      they are.
- [ ] **Step 3: Verify.** The web four, and the six specs untouched. If the
      sandbox spec needed editing, the constant moved wrong.

---

### Task 6: the preview HTML crosses the api seam

**Files:** `web/apps/app/src/api/client.ts`, `.../types.ts` (if a type is
needed), `web/apps/app/src/__mocks__/fake-api-client.ts`, and a test in
`web/apps/app/src/api/`.

- [ ] **Step 1: Write the failing test.**

```ts
it('fetches the rendered invoice page as text', async () => {
  // 200 text/html → the string; 500 with an error envelope → an ApiError
  // carrying the server's message
});
```

- [ ] **Step 2: Verify it fails.**
- [ ] **Step 3: Implement** `invoicePreviewHtml(number: number): Promise<string>`
      on the `ApiClient` interface and on `FetchApiClient`, against the same
      address `invoicePreviewUrl(number, 'html')` spells — one place, no second
      literal. Non-2xx raises the usual `ApiError` so the dialog can print the
      server's sentence. Add it to the fake, answering a small stub document and
      honouring a `previewHtmlError` field the way `sendInvoiceError` works.
- [ ] **Step 4: Verify.** The web four — `api-seam.test.ts` included, which is
      what proves the address stayed inside `src/api`.

---

### Task 7: the dialog shows the document

**Files:** `web/packages/ui/src/components/wc-send-dialog.ts`, `.preview.ts`,
`.test.ts`.

- [ ] **Step 1: Write the failing tests.**

```ts
it('states the recipient and the total where the send is confirmed', () => {
  // [data-recipient-line] carries the client name, the address and wc-money
});
it('frames the rendered invoice in the confirm phase', () => {
  // wc-document-frame present with the given html
});
it('drops the frame once the send is in flight', () => {
  // phase="sending" → no wc-document-frame, the trace is what is on screen
});
it('renders a preview failure as a notice, not as a frame', () => {
  // previewError set → [data-preview-error] carries the server's sentence,
  // and the confirm button is still reachable (a preview failure is not a block)
});
it('says the PDF is unavailable in a build without it', () => {
  // pdfAvailable=false → the sentence, and no download link
});
it('blocks the send when this build cannot attach a PDF', () => {
  // blocked set by the screen; [data-confirm] disabled, the frame still rendered
});
```

      plus new preview states — `confirm-with-preview`, `preview-loading`,
      `preview-failed`, `blocked-no-pdf` — each covered by
      `describePreviewA11y`. The existing twelve specs and the `wa-hide` pair
      must pass unchanged.

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement** the confirm body in the spec's Decision 3 order,
      with the new properties (`previewHtml`, `previewError`, `previewLoading`,
      `pdfHref`, `pdfAvailable`) all `attribute: false` where they are rich.
      Widen the dialog and bound the frame per Decision 4, in this component's
      own stylesheet, with the `48rem` breakpoint.
- [ ] **Step 4: Verify.** The web four. Eyeball every state at
      http://localhost:9090 (`npm run preview` in `web/`).

---

### Task 8: the screen wires it up

**Files:** `web/apps/app/src/screens/invoices.ts`,
`web/apps/app/src/screens/invoice-data.ts`,
`web/apps/app/src/screens/invoices.test.ts`.

- [ ] **Step 1: Write the failing tests.**

```ts
it('fetches the preview when the send dialog opens, and not before', () => {});
it('sends only after the confirmation dialog resolves', () => { /* existing */ });
it('shows the email subject the server will use', () => {
  // companyName set → "Invoice #1251 from Bluepeak"; unset → "Invoice #1251"
});
it('blocks the send when the build has no pdf export', () => {});
it('surfaces a public_base_url warning without blocking the send', () => {});
```

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement.**
  - `openSend` clears the previous preview and kicks off
    `client.invoicePreviewHtml(detail.number)`, holding
    `previewHtml`/`previewError`/`previewLoading` as `@state`.
  - `emailSubject(number, companyName)` lands in `invoice-data.ts`, mirroring
    `send.rs`'s rule, with a comment naming that function — a knowingly
    duplicated two-line rule (spec Decision 5).
  - `sendBlockReason` gains the no-`pdfExport` case; the
    `invoicing.publicBaseUrlWarning` from TASK-67 renders as a warning line in
    the dialog, not as a block.
  - `pdfHref` comes from `invoicePreviewUrl(number, 'pdf')`, `pdfAvailable`
    from `appStore.status.pdfExport`.
- [ ] **Step 4: Verify.** The web four.

---

### Task 9: documentation and the manual pass

- [ ] **Step 1: `docs/invoicing.md` — "Sending".** The CLI now renders the
      invoice, writes the preview files, states the recipient and the total, and
      asks; `--yes` skips the prompt and the files (and is required when stdin
      is not a TTY), matching `void`. Note that the render happens either way,
      so a broken custom template is caught before any gateway is called.
- [ ] **Step 2: `docs/invoicing.md` — "From the web UI".** The dialog frames the
      rendered page before the confirm button, offers the PDF beside it, and
      says when a build cannot send. The frame is the same document the send
      publishes, rendered through the same seam, with no gateway in reach.
- [ ] **Step 3: `docs/api.md`** — `invoicePreviewHtml` uses the existing preview
      route; no new endpoint. Note that the route is now also the send dialog's
      source, which is why its error envelope has to stay legible.
- [ ] **Step 4: `CLAUDE.md`** — Commands block:
      `nigel invoice send 1248 --yes`. Architecture: the send confirmation in
      the CLI entry, `wc-document-frame` in the `@nigel/ui` list, and the send
      dialog's preview in the SPA invoicing entry. Key Design Constraints:
      *send is confirmed on every surface, and every confirmation shows the
      document the client will get, rendered through `render_invoice` — the CLI
      writes the preview artifacts and asks, the browser frames the page the
      preview route serves, and neither path constructs a gateway.*
- [ ] **Step 5: `README.md`** — the `--yes` flag in the invoicing command list.
- [ ] **Step 6: Manual pass.**
  - `nigel invoice send 1248` on a TTY: read the summary, open the written
    HTML, answer `n`, confirm nothing happened.
  - `nigel invoice send 1248 --yes` with no config: fails at config, no files
    written.
  - Break `<data_dir>/templates/invoice.html`: both the CLI and the dialog name
    the path before anything is sent.
  - `nigel serve`: open the send dialog, see the document; make the preview
    route fail (broken template) and see a notice rather than JSON in a box;
    narrow the window to a phone width and confirm one scrollbar, not two.
  - A build without the `pdf` feature: the dialog frames the page and blocks the
    send with the reason.
- [ ] **Open the PR.**

---

## Final verification

- [ ] `cargo test -- --test-threads=1`
- [ ] `cargo test --no-default-features --features gusto -- --test-threads=1`
- [ ] `cargo test --no-default-features -- --test-threads=1`
- [ ] `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`
- [ ] `cd web && npm test && npm run lint && npm run typecheck && npm run build`
- [ ] `git diff src/invoicing/` is empty except for whatever TASK-78/64 already
      landed — this task adds no rendering code.
- [ ] `git diff web/packages/ui/src/components/wc-invoice-preview.test.ts` is
      empty (Task 5's invariance).

## Acceptance criteria mapping

| AC | Verified by |
|---|---|
| #1 the web send flow shows the rendered invoice before confirm | Tasks 4, 6, 7 (`frames the rendered invoice in the confirm phase`), Task 8 |
| #2 `nigel invoice send` requires confirmation, `--yes` to skip, matching void | Task 2 (`confirm_send`), Task 3 (`invoice_send_without_yes_refuses_on_a_non_tty_and_sends_nothing`) |
| #3 the preview comes from the same `render_invoice` seam | Task 2 (the CLI calls the seam directly), Task 6 (the route is the seam behind HTTP), and the "one render path" constraint |
| #4 previewing makes no network call and creates no Stripe link | Task 2 (the confirmation happens before `build_clients`), the preview route taking no gateway, Task 3 (`TEST_TIMEOUT` catches a call) |
| #5 the recipient and the total are stated where the operator confirms | Task 2 (`the_summary_names_the_client_the_total_and_the_recipient`), Task 7 (`states the recipient and the total…`) |
| #6 a build without `pdf` previews the HTML and says why there is no PDF | Task 3 (`invoice_send_without_the_pdf_feature_says_why_before_it_fails`), Task 7 (`says the PDF is unavailable…`, `blocks the send when this build cannot attach a PDF`) |
| #7 a broken custom template is caught at preview, before any gateway is called | Task 2 (`load_template` before `build_clients` — already the ordering, now visible), Task 3 (`invoice_send_with_a_broken_template_fails_before_asking_anything`), Task 7 (`renders a preview failure as a notice`) |
</content>
