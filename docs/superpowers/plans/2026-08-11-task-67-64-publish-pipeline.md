# Publish pipeline — TASK-67 and TASK-64 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Spec: `docs/superpowers/specs/2026-08-11-task-67-64-publish-pipeline-design.md`.
Read it first — every "why" below lives there.

**Goal:** the address Nigel hands a client resolves on a plain static host; a
`public_base_url` that cannot produce a working link is refused by name before
anything is published or emailed; and a payment landing on a published invoice
puts a corrected page and PDF back where the client is looking, best-effort.

**Architecture:** `invoicing::r2::public_url` names `index.html` instead of a
directory, and gains two pure validators beside it. The hard one is called from
`cli::invoice::build_clients` — the single constructor both send paths use — so
a refusal costs no Stripe link; the soft one is computed once in
`settings::invoicing_status` and carried on `/api/status`. Republishing is a new
`src/invoicing/republish.rs` built on `void.rs`'s shape: the write commits
first, the network work cannot undo it, and every outcome is a variant and a
sentence. Two `src/cli/invoice.rs` helpers resolve branding and publisher so the
six front-end call sites are one line each.

**Tech Stack:** Rust, rusqlite, reqwest/rusty-s3 (behind the `AssetPublisher`
trait — no test reaches the network), clap, axum (`serve` feature), assert_cmd /
predicates / tempfile; Lit + vitest for the SPA half of Part B.

## Sequencing — read before starting

This plan is **two parts that ship as two PRs, with TASK-78 between them**:

| Part | Tasks | PR | Depends on |
|---|---|---|---|
| A — TASK-67 | 1-6 | PR-2a | nothing |
| B — TASK-64 | 7-13 | PR-2c | Part A **and TASK-78 merged** |

Part B re-renders the published document so it "reflects paid amount, balance
and status". Today's documents render none of those; the Paid/Balance rows and
the shared `MoneySummary` are TASK-78's
(`docs/superpowers/specs/2026-08-11-task-78-document-parity-design.md`,
Decision 2). Running Part B first means republishing bytes identical to what is
already up there, and then rewriting the same code one PR later. **Do not start
Task 7 until TASK-78 is merged.** If the orchestrator overrides this, Part B
must carry a minimal paid/balance block of its own and TASK-78 must then
absorb it.

## Global Constraints

- After every task, all four green:
  - `cargo test -- --test-threads=1` (serial — the DB password is a process global)
  - `cargo test --no-default-features --features gusto -- --test-threads=1` and
    `cargo test --no-default-features -- --test-threads=1` — **every task must
    pass without the `pdf` feature**; republish falls back to `publish_page`
    there, and that path is only exercised in that build.
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt --check`
- Part B's web task additionally: `cd web && npm test && npm run lint && npm run typecheck && npm run build`.
- **TDD, always.** Write the failing test, watch it fail for the right reason,
  implement, watch it pass. A step that skips the failure is not done.
- `src/invoicing/` never reads `settings` and never reaches into `src/cli/`.
  Branding, publishers and template loading are resolved in `src/cli/invoice.rs`
  and passed down.
- No test may reach the network. Publishers and gateways arrive as traits and
  are faked, the way `send.rs`, `void.rs` and `server::routes::invoices` already
  do it.
- **A failed republish is never an error.** `republish_invoice` returns no
  `Result`; if you find yourself adding one, the design is wrong.
- The bucket layout (`object_key`) does not change.

---

# Part A — TASK-67 (PR-2a)

### Task 1: the published address names the file

**Files:** `src/invoicing/r2.rs`.

- [ ] **Step 1: Rewrite the failing tests** in `mod tests`:

```rust
#[test]
fn public_url_names_the_index_document_not_its_directory() {
    // A plain R2 custom domain serves objects by key and has no directory index.
    assert_eq!(
        public_url("https://billing.example.com/i", "abc"),
        "https://billing.example.com/i/abc/index.html"
    );
    assert_eq!(
        public_url("https://billing.example.com/i/", "abc"),
        "https://billing.example.com/i/abc/index.html"
    );
}

#[test]
fn the_address_and_the_key_name_the_same_object() {
    let url = public_url("https://billing.example.com/i", "abc");
    assert!(url.ends_with(&object_key("abc", PAGE_OBJECT)));
}
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib invoicing::r2 2>&1 | tail -20`
- [ ] **Step 3: Implement.** Add `pub const PAGE_OBJECT: &str = "index.html";`
      and use it in both `public_url` and `AssetPublisher::publish` /
      `publish_page`'s `object_key` calls, so the address and the key cannot
      drift.
- [ ] **Step 4: Verify nothing above it moved.** `send.rs`'s
      `starts_with("https://billing.example.com/i/")` assertions and
      `server::routes::invoices`'s `a_public_url_is_built_from_the_configured_base`
      are the two places that look at the shape — the second one needs its
      expected string updated, the first must pass untouched.
- [ ] **Step 5: Verify.** All four commands.

---

### Task 2: the two validators

**Files:** `src/invoicing/r2.rs`.

**Interface produced** (consumed by Tasks 3 and 4):

```rust
pub fn validate_public_base_url(value: &str) -> Result<()>;
pub fn public_base_url_warning(value: &str) -> Option<&'static str>;
```

- [ ] **Step 1: Write failing tests.**

```rust
#[test]
fn a_base_url_without_a_scheme_is_refused_naming_the_setting() {
    let err = validate_public_base_url("billing.example.com").unwrap_err().to_string();
    assert!(err.contains("public_base_url"), "got: {err}");
    assert!(err.contains("billing.example.com"), "the offending value: {err}");
    assert!(err.contains("https://"), "the expected shape: {err}");
}

#[test]
fn the_shapes_that_can_produce_a_link_are_accepted() {
    for ok in [
        "https://billing.example.com/i",
        "https://billing.example.com/i/",
        "HTTP://billing.example.com/i",
        "http://localhost:8787/i",
    ] {
        assert!(validate_public_base_url(ok).is_ok(), "refused: {ok}");
    }
}

#[test]
fn the_shapes_that_cannot_are_refused() {
    for bad in ["", "   ", "billing.example.com", "//billing.example.com/i",
                "ftp://billing.example.com/i", "https://", "https:// billing.example.com"] {
        assert!(validate_public_base_url(bad).is_err(), "accepted: {bad}");
    }
}

#[test]
fn a_base_url_that_does_not_end_in_the_i_prefix_warns_without_quoting_it() {
    let warning = public_base_url_warning("https://billing.example.com").expect("warns");
    assert!(warning.contains("/i"), "got: {warning}");
    assert!(!warning.contains("billing.example.com"), "status carries no values: {warning}");
    assert_eq!(public_base_url_warning("https://billing.example.com/i"), None);
    assert_eq!(public_base_url_warning("https://billing.example.com/i/"), None);
    assert!(public_base_url_warning("https://billing.example.com/invoices").is_some());
}
```

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement**, hand-rolled — `url` is not a dependency and adding
      one to answer "scheme plus host" would be the largest thing in this task.
      Trim; refuse empty, refuse any whitespace, require a case-insensitive
      `http://`/`https://` prefix, require a non-empty host before the next `/`.
      The warning trims trailing slashes and checks the tail is `/i`.
- [ ] **Step 4: Verify.** All four commands.

---

### Task 3: the refusal, where both front ends pass

**Files:** `src/cli/invoice.rs` (`build_clients`), `src/settings.rs`
(`InvoicingStatus`), `src/server/routes/status.rs` (test only).

- [ ] **Step 1: Write failing tests** in `src/cli/invoice.rs`'s `mod tests`
      (helpers `test_config()` and the `missing_public_base_url_names_the_setting`
      test are already there):

```rust
#[test]
fn a_scheme_less_public_base_url_is_refused_before_any_client_is_built() {
    let mut cfg = fully_configured_test_config();
    cfg.public_base_url = Some("billing.example.com".into());
    let err = build_clients(cfg).unwrap_err().to_string();
    assert!(err.contains("public_base_url"), "got: {err}");
}

#[test]
fn a_configured_base_url_that_can_produce_a_link_still_builds() {
    assert!(build_clients(fully_configured_test_config()).is_ok());
}
```

and in `src/settings.rs`:

```rust
#[test]
fn the_status_warns_about_a_base_url_that_misses_the_i_prefix() {
    let mut cfg = fully_configured();
    cfg.public_base_url = Some("https://billing.example.com".into());
    let status = invoicing_status(&cfg);
    assert!(status.send_configured, "a warning is not a refusal");
    assert!(status.public_base_url_warning.is_some());
}

#[test]
fn an_unset_base_url_is_missing_rather_than_warned_about() {
    let mut cfg = fully_configured();
    cfg.public_base_url = None;
    let status = invoicing_status(&cfg);
    assert!(status.public_base_url_warning.is_none());
    assert!(status.missing.contains(&"public_base_url"));
}
```

  Extend the existing `the_invoicing_status_never_carries_a_value` test to a
  config whose `public_base_url` warns, so the no-values invariant covers the
  new field.

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement.** In `build_clients`, validate the value after
      `require(cfg.public_base_url, "public_base_url")?` and before the
      `R2Publisher` literal. Add
      `#[serde(skip_serializing_if = "Option::is_none")] pub public_base_url_warning: Option<&'static str>`
      to `InvoicingStatus` and fill it in `invoicing_status`.
      **Leave `optional_publisher` alone** — void and republish only need the
      upload, and refusing there would leave a live payment link up to protect
      the formatting of a URL neither prints.
- [ ] **Step 4: Update the status route test** in
      `src/server/routes/status.rs` that counts the `invoicing` keys, if it
      asserts on the object's shape.
- [ ] **Step 5: Verify.** All four commands.

---

### Task 4: the CLI notice, and the API's step tag

**Files:** `src/cli/invoice.rs` (`send`), `src/server/routes/invoices.rs`
(`send`).

- [ ] **Step 1: Write the failing route test** in
      `src/server/routes/invoices.rs`'s `mod tests`, beside
      `send_with_no_invoicing_config_is_a_409_naming_the_missing_keys`:

```rust
#[tokio::test]
async fn a_send_with_an_unusable_public_base_url_fails_at_config() {
    // All nine keys set, one of them unusable: the refusal is the config step's,
    // not a 502 from R2 after the upload was attempted.
    // → status is a 4xx, the message names public_base_url, details.step == "config"
}
```

- [ ] **Step 2: Verify it fails.**
- [ ] **Step 3: Implement.** In the route, `build_clients`'s error is already
      forwarded; tag it `SendStep::Config` the way the other config failures are
      (`.at_step(SendStep::Config)` / the same `details` shape
      `not_configured` produces). In `cli::invoice::send`, print the warning
      before doing anything:

```rust
if let Some(url) = invoicing_config().public_base_url.as_deref() {
    if let Some(warning) = crate::invoicing::r2::public_base_url_warning(url) {
        eprintln!("notice: {warning}");
    }
}
```

      (Read it off `settings::invoicing_status` if that reads better — one
      source either way.)
- [ ] **Step 4: Verify.** All four commands.

---

### Task 5: end-to-end

**Files:** `tests/cli_dispatch.rs`.

`TestEnv` clears all nine `NIGEL_*` variables per command, so a test that wants
a configured-but-wrong installation sets them explicitly on that command.
Anything that reaches the network hits `TEST_TIMEOUT`.

- [ ] **Step 1: Write the tests.**

```rust
#[test] fn invoice_send_refuses_a_public_base_url_with_no_scheme() {
    // nine NIGEL_* vars set, public_base_url = "billing.example.com"
    // → failure, stderr names public_base_url, and the invoice is still a draft
    //   (no Stripe key was ever used: the refusal is before build_clients returns)
}
#[test] fn invoice_preview_is_unaffected_by_a_broken_public_base_url() {
    // preview needs no config at all and must not have grown a dependency on one
}
```

- [ ] **Step 2: Verify they fail, then implement nothing** — Tasks 3-4 already
      did. If they pass immediately, confirm by reverting Task 3's one-line
      validation call and watching them fail.
- [ ] **Step 3: Verify.** Both feature builds of `--test cli_dispatch`.

---

### Task 6: Part A documentation

Per CLAUDE.md's Documentation Policy the work is not complete until these land.

- [ ] **Step 1: `docs/invoicing.md` — "Hosting: billing.example.com → R2".** Rewrite
      the two paragraphs that claim the object "is served at
      `https://billing.example.com/i/{token}/`": the object is served at
      `…/i/{token}/index.html`, which is the address Nigel prints, emails and
      reports, because a static host is not required to have an opinion about
      directories. An edge rewrite (Cloudflare rule or Worker appending
      `index.html`) is an *option* if you prefer the directory form — both
      addresses then resolve to the same object.
- [ ] **Step 2: `docs/invoicing.md` — "Configuration".** Note that
      `public_base_url` is validated at send time: an address without an
      http(s) scheme is refused by name before anything is published, and one
      whose path does not end in `/i` produces a notice, because Nigel writes
      every object under that prefix.
- [ ] **Step 3: `docs/invoicing.md` — "Sending"**, step 3 and the sample output
      line: the printed URL now ends in `/index.html`.
- [ ] **Step 4: `docs/api.md`** — `GET /api/status`'s `invoicing` block gains
      `publicBaseUrlWarning`; the send route's config refusals gain the
      unusable-base-URL case.
- [ ] **Step 5: `CLAUDE.md`** — amend the Invoicing bullet: `r2.rs` publishes
      under `i/{token}/` and hands out the `index.html` address rather than the
      directory, and `public_base_url` is validated at send time (hard refusal
      on a missing scheme, a warning on a path that does not end in `/i`).
- [ ] **Step 6: Verify.** `git diff --stat` shows all four docs touched.
- [ ] **Open PR-2a.**

---

# Part B — TASK-64 (PR-2c, after TASK-78)

### Task 7: `src/invoicing/republish.rs`

**Files:** create `src/invoicing/republish.rs`; modify `src/invoicing/mod.rs`
(`pub mod republish;`, alphabetically between `r2` and `render`).

**Interface produced** (consumed by Tasks 9-11): the `Republished`,
`RepublishOutcome` and `republish_invoice` items from the spec's Decision 3.

- [ ] **Step 1: Write failing tests** in a new `mod tests`. Copy `test_conn()`,
      `seed()` and the fake publishers from `void.rs`'s test module — a
      `CapturePub` that records the bytes, a `FailPub` that answers
      `r2 403: …`. **These tests are not gated on `pdf`**: the fallback path is
      only reachable in the other build.

```rust
#[test] fn an_unpublished_invoice_is_not_applicable_and_uploads_nothing() {}
#[test] fn a_published_invoice_is_re_rendered_and_re_uploaded() {
    // record a payment, republish, assert the captured html carries the new
    // balance and that publish() (not publish_page) was the call
}
#[test] fn a_void_invoice_is_not_applicable() {}
#[test] fn no_publisher_is_skipped_and_warns_that_the_page_is_stale() {}
#[test] fn a_failed_upload_keeps_the_upstreams_own_words() {}
#[test] fn republishing_writes_nothing_to_the_invoice() {}
#[cfg(not(feature = "pdf"))]
#[test] fn without_the_pdf_feature_only_the_page_is_replaced_and_nothing_warns() {
    // publish_page was called, publish was not, Done { pdf: false }, warnings empty
}
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib invoicing::republish 2>&1 | tail -20`
- [ ] **Step 3: Implement.** `republish_invoice` returns no `Result`: every
      failure — including a render failure — becomes `Republished::Failed`.
      Dispatch on `(invoice.published_at, is_void(invoice), publisher)` first so
      the ordinary case does no rendering at all.
- [ ] **Step 4: Verify.** All four commands.

---

### Task 8: the pay button moves below the seam

**Files:** `src/invoicing/render.rs`, `src/invoicing/send.rs`,
`src/cli/invoice.rs`.

- [ ] **Step 1: Write failing tests** in `render.rs`:

```rust
#[test] fn a_settled_invoice_never_renders_a_pay_button() {
    // record the full balance, then assert matches!(pay_button_for(&invoice), PayButton::Omitted)
}
#[test] fn a_void_invoice_never_renders_a_pay_button_even_with_a_live_link() {}
#[test] fn a_partly_paid_invoice_keeps_its_link() {}
#[test] fn a_draft_with_no_link_gets_the_placeholder() {}
```

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement.** Move `pay_button_for` out of `src/cli/invoice.rs`
      into `render.rs`, add the paid-in-full arm (`invoice.status ==
      InvoiceStatus::Paid.as_str()`), re-export it from `cli::invoice` so the
      API preview routes and `preview()` need only an import change, and replace
      `send.rs`'s inline two-arm match with a call. Move the existing
      `cli::invoice` unit tests for it across.
- [ ] **Step 4: Verify.** `send.rs`'s suite must pass untouched.
- [ ] **Step 5: Verify.** All four commands.

---

### Task 9: the CLI-layer helpers

**Files:** `src/cli/invoice.rs`.

**Interface produced:** `republish_after_payment(conn, invoice_id) -> Vec<String>`
and `republish_all(conn, numbers) -> Vec<String>`.

- [ ] **Step 1: Write failing tests** in `cli::invoice`'s `mod tests` (which
      already has `test_conn`, `seed_invoice`, `test_config`, and runs under a
      `TempConfigDir` so no real settings are read):

```rust
#[test] fn republishing_with_nothing_configured_warns_and_records_nothing() {
    // published invoice + paid + no r2 keys → one warning naming the invoice
}
#[test] fn republishing_an_unpublished_invoice_says_nothing() {}
#[test] fn a_broken_custom_template_is_a_warning_not_a_failure() {
    // write an invalid <data_dir>/templates/invoice.html, then republish:
    // the returned Vec carries the template's own sentence and nothing panics
}
```

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement.** Resolve `invoicing_config()`, `optional_publisher`,
      `load_template(&get_data_dir())`, `company_name(conn)` and
      `contact_email_for_preview`; look the invoice and client up; call
      `republish_invoice`. Any resolution failure becomes a warning string —
      the payment is committed and nothing here may read as its failure.
      `republish_all` maps numbers to ids and concatenates.
- [ ] **Step 4: Verify.** All four commands.

---

### Task 10: `SyncReport` says which invoices moved

**Files:** `src/invoicing/sync.rs`.

- [ ] **Step 1: Write failing tests** in `sync.rs`'s `mod tests`:

```rust
#[test] fn the_report_names_the_invoices_a_payment_landed_on() {}
#[test] fn an_invoice_with_no_new_payment_is_not_named() {
    // re-running a sync records nothing and the list is empty (session dedup)
}
```

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement.** Add `pub recorded_invoices: Vec<i64>` (invoice
      **numbers**, serialized camelCase like the rest of the struct) and push in
      `run_sync` when `sync_invoice` answers a non-zero count.
- [ ] **Step 4: Verify.** All four commands.

---

### Task 11: wire the six call sites

**Files:** `src/cli/invoice.rs` (`pay`, `sync`), `src/main.rs`
(`sync_invoice_payments`), `src/cli/invoice_manager.rs`,
`src/server/routes/invoices.rs` (`pay`, `sync`).

- [ ] **Step 1: Write the failing route tests** in
      `server::routes::invoices`'s `mod tests`, using the fake publisher the
      void tests already use:

```rust
#[tokio::test] async fn paying_a_published_invoice_republishes_its_page() {}
#[tokio::test] async fn a_failed_republish_is_still_a_200_carrying_the_payment() {
    // body has the refreshed invoice AND republishWarnings
}
#[tokio::test] async fn paying_an_unpublished_invoice_carries_no_warnings_field() {}
```

      A `pay_with(conn, …, publisher: Option<&P>)` seam mirroring `void_with` is
      what makes those tests possible — add it rather than reaching the real
      publisher from the handler.

- [ ] **Step 2: Verify they fail.**
- [ ] **Step 3: Implement.**
  - `cli::invoice::pay`: after `record_payment`, print each warning.
  - `cli::invoice::sync`: `republish_all(&conn, &report.recorded_invoices)`,
    printed beside the existing per-invoice failure notices.
  - `main::sync_invoice_payments`: the same, as `notice:` lines.
  - `cli/invoice_manager.rs`: warnings onto the result screen, exactly where
    void's land.
  - The `pay` route: `PayResult { #[serde(flatten)] invoice,
    #[serde(skip_serializing_if = "Vec::is_empty")] republish_warnings }`.
  - The `sync` route: `SyncReport` gains `republish_warnings` the same way; the
    handler fills it from `republish_all`.
- [ ] **Step 4: Verify.** All four commands, plus the TUI's own suite.

---

### Task 12: the SPA renders the warnings

**Files:** `web/apps/app/src/api/types.ts`,
`web/apps/app/src/screens/invoices.ts`,
`web/apps/app/src/__mocks__/fake-api-client.ts`, and the screen's test.

- [ ] **Step 1: Write the failing test** in
      `web/apps/app/src/screens/invoices.test.ts`, modelled on the void-warning
      spec:

```ts
it('shows a republish warning after a payment without hiding the payment', async () => {
  // fake pay() answers { …detail, republishWarnings: ['Warning: …'] }
  // → the balance updated AND one [data-action-warning] notice
});
```

- [ ] **Step 2: Verify it fails.**
- [ ] **Step 3: Implement.** `PayResult`/`SyncReport` types gain
      `republishWarnings?: string[]`; the screen's existing `voidWarnings`
      channel becomes the general `actionWarnings` one (same `wc-notice-bar
      variant="warning"`, same per-index dismissal) and the payment flow pushes
      into it. The sentences come from Rust verbatim — they are not re-derived
      client-side, exactly as void's are not.
- [ ] **Step 4: Verify.** `cd web && npm test && npm run lint && npm run typecheck && npm run build`.

---

### Task 13: Part B documentation and the manual pass

- [ ] **Step 1: `docs/invoicing.md` — "Recording payments".** A payment against
      a *published* invoice re-renders the page and the PDF and puts them back,
      best-effort: nothing configured means a warning, an R2 failure means a
      warning naming it, and either way the payment is recorded. Note that a
      build without the `pdf` feature replaces the page only, leaving the
      attachment the client was actually sent.
- [ ] **Step 2: `docs/invoicing.md` — "Sync on launch".** The launch sync
      republishes too, so a payment found at launch corrects the page.
- [ ] **Step 3: `docs/api.md`** — `POST /api/invoices/{number}/pay` may now make
      network calls and answers `republishWarnings`; `POST /api/invoices/sync`
      answers `recordedInvoices` and `republishWarnings`.
- [ ] **Step 4: `CLAUDE.md`** — Invoicing bullet gains `republish.rs`, described
      the way `void.rs` is: best-effort, committed write first, one place for
      the sentences, `publish_page` fallback without the `pdf` feature, and the
      paid-in-full pay-button rule now living below the seam.
- [ ] **Step 5: `README.md`** — one line if the payment flow is described there.
- [ ] **Step 6: Manual pass** against a scratch data directory with test-mode
      Stripe keys and a scratch bucket:
  - send an invoice, open the printed URL (it must resolve with no edge rule);
  - record a partial payment, reload the page — Paid and Balance move;
  - record the rest, reload — no Pay button;
  - unset `r2_bucket`, record a payment on another published invoice — the
    payment lands and one warning prints;
  - set `public_base_url` to `billing.example.com` and try to send — refused by
    name, nothing published.
- [ ] **Open PR-2c.**

---

## Final verification

- [ ] `cargo test -- --test-threads=1`
- [ ] `cargo test --no-default-features --features gusto -- --test-threads=1`
- [ ] `cargo test --no-default-features -- --test-threads=1`
- [ ] `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`
- [ ] `cd web && npm test && npm run lint && npm run typecheck && npm run build`
- [ ] `git grep -n '{token}/\"' src docs` finds no surviving directory-form
      address.
- [ ] `git diff src/invoicing/send.rs` (Part B) changes only the pay-button
      call — nothing in the gateway call, the publish, the email or
      `mark_published`.

## Acceptance criteria mapping

| AC | Verified by |
|---|---|
| 67 #1 the printed/emailed link resolves on a plain R2 custom domain | Task 1 (`public_url_names_the_index_document_not_its_directory`, `the_address_and_the_key_name_the_same_object`), Task 6 Step 1 for the rewrite note |
| 67 #2 a scheme-less `public_base_url` is rejected by name before anything is published or emailed | Task 2, Task 3 (`a_scheme_less_public_base_url_is_refused_before_any_client_is_built`), Task 4 (`…fails_at_config`), Task 5 |
| 67 #3 a path not ending in `/i` warns | Task 2 (`a_base_url_that_does_not_end_in_the_i_prefix_warns_without_quoting_it`), Task 3 (status), Task 4 (CLI notice) |
| 64 #1 a manual payment re-renders and re-uploads HTML and PDF | Task 7 (`a_published_invoice_is_re_rendered_and_re_uploaded`), Task 11 (`paying_a_published_invoice_republishes_its_page`) |
| 64 #2 sync-recorded payments trigger the same republish | Task 10, Task 11 (`cli::invoice::sync`, launch sync, sync route) |
| 64 #3 a failed republish leaves the payment recorded and reports a notice | Task 7 (`republish_invoice` returns no `Result`; `a_failed_upload_keeps_the_upstreams_own_words`), Task 11 (`a_failed_republish_is_still_a_200_carrying_the_payment`), Task 12 |
</content>
