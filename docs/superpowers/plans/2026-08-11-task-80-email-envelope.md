# Outgoing email envelope Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An invoice email carries a display name, a from address validated
against `mailgun_domain`, and an optional reply-to; the published page's
direct-deposit contact line is configured separately from the Mailgun From —
per `docs/superpowers/specs/2026-08-11-task-80-email-envelope-design.md`.

**Architecture:** Three new optional settings (`from_name`, `reply_to_email`,
`contact_email`) resolve through `settings::invoicing_config()` beside the
existing nine. `src/invoicing/mailgun.rs` gains a value type, `EmailEnvelope`,
plus three pure functions — `format_address` (RFC 5322 `name-addr`),
`validate_from_address` (domain match), `validate_header_value` (no CR/LF) —
and `MailgunClient.from: String` becomes `MailgunClient.envelope: EmailEnvelope`.
`cli::invoice::build_clients` grows a `company: &str` argument so an unset
`from_name` falls back to the database's `company_name`, and stops being the
source of `Branding.contact_email`, which now comes from `contact_address(cfg)`.
The HTTP send route resolves the company name before building clients and maps
the new `Invalid` from `build_clients` to a `409 send_misconfigured`.

**Tech Stack:** Rust, rusqlite, reqwest (blocking, multipart), serde, axum
(behind the `serve` feature), assert_cmd/predicates/tempfile.

## Global Constraints

- After every task: `cargo test -- --test-threads=1`,
  `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` clean.
- **Every task must also pass without the `pdf` feature** —
  `cargo test --no-default-features --features gusto -- --test-threads=1` — and
  with no default features at all:
  `cargo test --no-default-features -- --test-threads=1`.
- `src/invoicing/` never reads `settings` and never reaches into `src/cli/`.
  `EmailEnvelope` is a value type the CLI layer fills in.
- **No response, error or log carries a configured value** — key names only.
  Every new refusal names settings keys and nothing else.
- `Branding` keeps its three fields; `render_html.rs`,
  `templates/invoice.html` and `render.rs` are **not touched** by this task
  (Stream 2's TASK-78 owns those files).
- No test may reach the network. Mailgun coverage is `message_fields` and the
  pure helpers; the orchestration keeps using the existing fakes.

---

### Task 1: Three settings, resolved and never required

**Files:** modify `src/settings.rs`.

**Interface produced** (consumed by Tasks 2–4):

```rust
pub struct Settings { /* … */ pub from_name: Option<String>,
                              pub reply_to_email: Option<String>,
                              pub contact_email: Option<String>, }

pub struct InvoicingConfig { /* … */ pub from_name: Option<String>,
                                     pub reply_to_email: Option<String>,
                                     pub contact_email: Option<String>, }
```

- [ ] **Step 1: Write failing tests** in `settings.rs`'s `mod tests`. Extend
  `fully_configured()` with the three values (`"Bluepeak"`,
  `"sam@example.com"`, `"accounts@example.com"`) and add them to the leak
  list in `the_invoicing_status_never_carries_a_value`. Then:

```rust
#[test]
fn the_optional_envelope_keys_never_appear_in_the_missing_list() {
    let mut cfg = fully_configured();
    cfg.from_name = None;
    cfg.reply_to_email = None;
    cfg.contact_email = None;
    let status = invoicing_status(&cfg);
    assert!(status.send_configured, "an optional key cannot block a send");
    assert!(status.missing.is_empty(), "got: {:?}", status.missing);
}

#[test]
fn the_new_envelope_keys_resolve_from_the_environment_first() {
    let file_settings = Settings {
        from_name: Some("File Co".into()),
        reply_to_email: Some("file@example.test".into()),
        contact_email: Some("file-contact@example.test".into()),
        ..Settings::default()
    };
    let cfg = invoicing_config_with(&file_settings, |name| match name {
        "NIGEL_FROM_NAME" => Some("Env Co".into()),
        "NIGEL_REPLY_TO_EMAIL" => Some("env@example.test".into()),
        "NIGEL_CONTACT_EMAIL" => Some("env-contact@example.test".into()),
        _ => None,
    });
    assert_eq!(cfg.from_name.as_deref(), Some("Env Co"));
    assert_eq!(cfg.reply_to_email.as_deref(), Some("env@example.test"));
    assert_eq!(cfg.contact_email.as_deref(), Some("env-contact@example.test"));

    let from_file = invoicing_config_with(&file_settings, |_| None);
    assert_eq!(from_file.from_name.as_deref(), Some("File Co"));
    assert_eq!(from_file.contact_email.as_deref(), Some("file-contact@example.test"));
}

#[test]
fn a_settings_file_written_before_these_keys_existed_still_loads() {
    let json = r#"{"data_dir": "/tmp/t", "from_email": "billing@mg.example.com"}"#;
    let s: Settings = serde_json::from_str(json).unwrap();
    assert_eq!(s.from_email.as_deref(), Some("billing@mg.example.com"));
    assert!(s.from_name.is_none() && s.reply_to_email.is_none() && s.contact_email.is_none());
}
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib settings 2>&1 | tail -20`
  — compile errors, the fields do not exist.

- [ ] **Step 3: Implement.** Add the three `#[serde(default)] Option<String>`
  fields to `Settings` (after `from_email`, keeping the file's key order
  readable), to the hand-written `impl Default for Settings`, to
  `InvoicingConfig`, and to `invoicing_config_with`:

```rust
from_name: env_or("NIGEL_FROM_NAME", &s.from_name),
reply_to_email: env_or("NIGEL_REPLY_TO_EMAIL", &s.reply_to_email),
contact_email: env_or("NIGEL_CONTACT_EMAIL", &s.contact_email),
```

  **`invoicing_status` is not touched.** Its `[(&str, &Option<String>); 9]` keeps
  nine entries and its documented order; the new keys are optional and a missing
  optional key is not missing configuration.

- [ ] **Step 4: Fix the other `InvoicingConfig` literals** so the tree compiles —
  `cli/invoice.rs`'s `test_config()` and any `..test_config()` spread, and the
  server's test helpers. Use `grep -rn "InvoicingConfig {" src/` to find them all.

- [ ] **Step 5: Verify.** All three feature builds, clippy, fmt.

---

### Task 2: The envelope and the header rules

**Files:** modify `src/invoicing/mailgun.rs`.

**Interface produced** (consumed by Tasks 3 and 4):

```rust
pub struct EmailEnvelope {
    pub from_address: String,
    pub from_name: Option<String>,
    pub reply_to: Option<String>,
}
pub fn format_address(display_name: Option<&str>, address: &str) -> String;
pub fn validate_from_address(from: &str, domain: &str) -> Result<()>;
pub fn validate_header_value(value: &str, what: &str) -> Result<()>;
pub fn message_fields(envelope: &EmailEnvelope, to: &str, subject: &str, html: &str)
    -> Vec<(String, String)>;
pub struct MailgunClient { pub api_key: String, pub domain: String, pub envelope: EmailEnvelope }
```

- [ ] **Step 1: Write failing tests** in `mailgun.rs`'s `mod tests`. Migrate
  `message_fields_include_from_to_subject_html` to the envelope first, with its
  assertions unchanged apart from the `from` value, then add:

```rust
fn envelope(name: Option<&str>, reply_to: Option<&str>) -> EmailEnvelope {
    EmailEnvelope {
        from_address: "billing@mg.example.com".into(),
        from_name: name.map(str::to_string),
        reply_to: reply_to.map(str::to_string),
    }
}

#[test]
fn a_plain_display_name_is_not_quoted() {
    assert_eq!(format_address(Some("Bluepeak"), "b@e.test"), "Bluepeak <b@e.test>");
}

#[test]
fn no_display_name_is_a_bare_address() {
    assert_eq!(format_address(None, "b@e.test"), "b@e.test");
    assert_eq!(format_address(Some("   "), "b@e.test"), "b@e.test");
}

#[test]
fn a_comma_or_a_quote_is_encoded_not_concatenated() {
    // AC #4.
    assert_eq!(format_address(Some("Carter, Sam"), "b@e.test"),
               "\"Carter, Sam\" <b@e.test>");
    assert_eq!(format_address(Some("Sam \"Nigel\" Carter"), "b@e.test"),
               "\"Sam \\\"Nigel\\\" Carter\" <b@e.test>");
    assert_eq!(format_address(Some("Bluepeak \\ Co"), "b@e.test"),
               "\"Bluepeak \\\\ Co\" <b@e.test>");
    for special in ["a;b", "a:b", "a<b", "a>b", "a@b", "a(b", "a)b", "a[b", "a]b"] {
        let out = format_address(Some(special), "b@e.test");
        assert!(out.starts_with('"'), "{special} must be quoted, got {out}");
    }
}

#[test]
fn a_utf8_display_name_passes_through_unencoded() {
    // Deliberate: Mailgun's API is UTF-8 and RFC 2047 would cost a base64 dep.
    assert_eq!(format_address(Some("Bluepeak — Books"), "b@e.test"),
               "Bluepeak — Books <b@e.test>");
}

#[test]
fn a_header_value_may_not_carry_a_newline() {
    let err = validate_header_value("Bluepeak\r\nBcc: someone@else.test", "from_name")
        .unwrap_err().to_string();
    assert!(err.contains("from_name"), "got: {err}");
    assert!(!err.contains("someone@else.test"), "the refusal must not echo the value");
    assert!(validate_header_value("Bluepeak", "from_name").is_ok());
}

#[test]
fn the_from_address_must_be_on_the_mailgun_domain() {
    assert!(validate_from_address("billing@mg.example.com", "mg.example.com").is_ok());
    assert!(validate_from_address("BILLING@MG.EXAMPLE.COM", "mg.example.com").is_ok());
    for bad in ["billing@example.com", "billing@sub.mg.example.com", "billing"] {
        let err = validate_from_address(bad, "mg.example.com").map(|_| ()).unwrap_err().to_string();
        assert!(err.contains("from_email") && err.contains("mailgun_domain"), "got: {err}");
        assert!(!err.contains(bad), "the refusal must name keys, not values: {err}");
    }
}

#[test]
fn a_reply_to_becomes_the_mailgun_header_field_and_is_absent_otherwise() {
    let with = message_fields(&envelope(Some("Bluepeak"), Some("sam@example.com")),
                              "a@b.test", "Invoice #1248", "<p>hi</p>");
    assert!(with.contains(&("from".into(), "Bluepeak <billing@mg.example.com>".into())));
    assert!(with.contains(&("h:Reply-To".into(), "sam@example.com".into())));

    let without = message_fields(&envelope(None, None), "a@b.test", "s", "<p>hi</p>");
    assert!(without.iter().all(|(k, _)| k != "h:Reply-To"),
            "an unset reply-to emits no field at all");
    assert!(without.contains(&("from".into(), "billing@mg.example.com".into())));
}
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib mailgun 2>&1 | tail -20`

- [ ] **Step 3: Implement.** `format_address` trims the name, returns the bare
  address when it is empty, and otherwise quotes when any character is outside
  RFC 5322 `atext` plus space — escaping `\` and `"` inside the quotes.
  `validate_from_address` splits once on the last `@`, lowercases both sides with
  `eq_ignore_ascii_case`, and returns

```rust
NigelError::Invalid(
    "from_email is not on mailgun_domain — Mailgun will refuse it. \
     The from address's domain must match mailgun_domain exactly.".into())
```

  `validate_header_value` refuses `\r`, `\n` and any other ASCII control
  character with `format!("{what} may not contain a line break")`.

- [ ] **Step 4: Rework `MailgunClient`.** `from: String` becomes
  `envelope: EmailEnvelope`; the `Mailer` impl passes `&self.envelope` to
  `message_fields`. Nothing else in the impl changes.

- [ ] **Step 5: Verify.** All three feature builds, clippy, fmt.

---

### Task 3: Resolution in the CLI layer

**Files:** modify `src/cli/invoice.rs`, `src/cli/invoice_manager.rs`.

**Interface produced** (consumed by Task 4):

```rust
pub(crate) fn build_clients(cfg: InvoicingConfig, company: &str)
    -> Result<(StripeClient, R2Publisher, MailgunClient)>;
pub(crate) fn contact_address(cfg: &InvoicingConfig) -> Option<String>;
pub(crate) fn contact_email_for_preview(cfg: &InvoicingConfig) -> (String, bool);
```

- [ ] **Step 1: Write failing tests** in `cli/invoice.rs`'s `mod tests`
  (`test_config()` and the fully-populated helpers already exist):

```rust
#[test]
fn the_display_name_falls_back_to_the_company_name() {
    let cfg = InvoicingConfig { from_name: None, ..configured() };
    let (_s, _r, mail) = build_clients(cfg, "Bluepeak").expect("built");
    assert_eq!(mail.envelope.from_name.as_deref(), Some("Bluepeak"));

    let cfg = InvoicingConfig { from_name: Some("Bluepeak Books".into()), ..configured() };
    let (_s, _r, mail) = build_clients(cfg, "Bluepeak").expect("built");
    assert_eq!(mail.envelope.from_name.as_deref(), Some("Bluepeak Books"),
               "from_name wins over the business name");
}

#[test]
fn no_company_and_no_from_name_means_no_display_name() {
    let cfg = InvoicingConfig { from_name: None, ..configured() };
    let (_s, _r, mail) = build_clients(cfg, "").expect("built");
    assert!(mail.envelope.from_name.is_none());
}

#[test]
fn a_from_address_off_the_mailgun_domain_refuses_before_any_client_is_used() {
    let cfg = InvoicingConfig { from_email: Some("billing@elsewhere.test".into()), ..configured() };
    let err = build_clients(cfg, "Bluepeak").map(|_| ()).unwrap_err();
    assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
    assert!(err.to_string().contains("mailgun_domain"));
}

#[test]
fn a_display_name_with_a_line_break_is_refused_by_name() {
    let cfg = InvoicingConfig { from_name: Some("Bluepeak\r\nBcc: x@y.test".into()), ..configured() };
    let err = build_clients(cfg, "").map(|_| ()).unwrap_err().to_string();
    assert!(err.contains("from_name"), "got: {err}");
}

#[test]
fn the_page_contact_falls_back_to_the_from_address() {
    let cfg = InvoicingConfig { contact_email: None, from_email: Some("billing@mg.example.com".into()),
                                ..test_config() };
    assert_eq!(contact_address(&cfg).as_deref(), Some("billing@mg.example.com"));

    let cfg = InvoicingConfig { contact_email: Some("accounts@example.com".into()), ..cfg };
    assert_eq!(contact_address(&cfg).as_deref(), Some("accounts@example.com"),
               "contact_email is what the page prints, whatever the email is sent from");
}

#[test]
fn neither_key_set_is_the_preview_placeholder() {
    let (value, placeholder) = contact_email_for_preview(&test_config());
    assert!(placeholder && value.contains("contact_email"), "got: {value}");
}
```

  `configured()` is a new helper returning a fully-populated `InvoicingConfig`
  whose `from_email` is on its `mailgun_domain`.

- [ ] **Step 2: Verify they fail.** `cargo test --lib cli::invoice 2>&1 | tail -20`

- [ ] **Step 3: Implement** in `cli/invoice.rs`:

```rust
const PREVIEW_CONTACT_PLACEHOLDER: &str = "(contact_email not configured)";

/// The address the published page's direct-deposit line prints. Falls back to
/// the send address, so an installation that never sets it renders exactly the
/// page it rendered before this key existed.
pub(crate) fn contact_address(cfg: &InvoicingConfig) -> Option<String> {
    cfg.contact_email.clone().or_else(|| cfg.from_email.clone())
}

pub(crate) fn contact_email_for_preview(cfg: &InvoicingConfig) -> (String, bool) {
    match contact_address(cfg) {
        Some(email) => (email, false),
        None => (PREVIEW_CONTACT_PLACEHOLDER.to_string(), true),
    }
}
```

  and in `build_clients`, after the two Mailgun keys are required:

```rust
let api_key = require(cfg.mailgun_api_key, "mailgun_api_key")?;
let domain = require(cfg.mailgun_domain, "mailgun_domain")?;
let from_address = require(cfg.from_email, "from_email")?;
validate_from_address(&from_address, &domain)?;

// `from_name` unset falls back to the business name the settings screen
// writes — the same value the subject line already uses.
let from_name = cfg.from_name
    .or_else(|| Some(company.trim().to_string()).filter(|c| !c.is_empty()));
if let Some(ref name) = from_name { validate_header_value(name, "from_name")?; }
if let Some(ref reply) = cfg.reply_to_email { validate_header_value(reply, "reply_to_email")?; }

let mail = MailgunClient {
    api_key, domain,
    envelope: EmailEnvelope { from_address, from_name, reply_to: cfg.reply_to_email },
};
```

  `reply_to_email` gets no domain check — AC #5's asymmetry, stated in a comment
  beside the two calls so it reads as a decision rather than an omission.

- [ ] **Step 4: Update the call sites.**
  - `send()`: `let company = company_name(&conn);` moves **above**
    `build_clients(invoicing_config(), &company)?`, and
    `Branding { contact_email: … }` takes
    `contact_email_for_preview(&cfg).0` — resolve the config once into a
    local so it is not read twice.
  - `preview()`: unchanged apart from the notice string:
    `notice: neither contact_email nor from_email is configured — the direct-deposit contact line is a placeholder`.
  - `cli/invoice_manager.rs` lines 1048 and 1107: both `build_clients` calls take
    the company name, which that screen can get from `company_name(conn)`.

- [ ] **Step 5: Verify.** All three feature builds, clippy, fmt. In particular
  `cargo test --lib cli::invoice` must show the pre-existing
  `missing_from_email_names_the_setting` and
  `preview_requires_no_invoicing_config_at_all` still passing.

---

### Task 4: The HTTP send route and the web's sentence

**Files:** modify `src/server/routes/invoices.rs`,
`web/apps/app/src/screens/invoicing-errors.ts`.

- [ ] **Step 1: Write failing tests** in `routes/invoices.rs`'s test module,
  beside the existing `send_not_configured` tests (they use `TempConfigDir` plus
  a written settings file, so follow whichever helper those four use):

```rust
#[tokio::test]
async fn a_from_address_off_the_mailgun_domain_is_a_409_naming_the_keys() {
    // settings: all nine set, from_email = billing@elsewhere.test
    // POST /api/invoices/1248/send {"confirm": true}
    // 409, details.reason == "send_misconfigured", details.step == "config"
    // body contains neither "billing@elsewhere.test" nor any other value
}

#[tokio::test]
async fn a_configured_reply_to_and_display_name_reach_the_mailer() {
    // send_with + the module's fake Mailer: assert the recorded `from` field is
    // `Bluepeak <billing@mg.example.test>` and the recorded reply-to is set.
}
```

  and in `routes/invoices.rs`'s preview test, retarget the assertion on
  `(from_email not configured)` to `(contact_email not configured)`.

- [ ] **Step 2: Verify they fail.** `cargo test --features serve routes::invoices 2>&1 | tail -30`

- [ ] **Step 3: Implement** in `send()`:

```rust
let config = crate::settings::invoicing_config();
let status = crate::settings::invoicing_status(&config);
if !status.send_configured {
    return Err(not_configured("Sending invoices", &status.missing));
}
let contact_email = crate::cli::invoice::contact_email_for_preview(&config).0;
// One extra connection open on a request about to make five network calls, and
// it keeps `build_clients` — and its 409 — outside the db work below.
let company = with_conn(&state, |conn| Ok(crate::cli::invoice::company_name(conn))).await?;
let (stripe, r2, mail) = crate::cli::invoice::build_clients(config, &company)
    .map_err(misconfigured)?;
```

  with

```rust
/// `build_clients` can now refuse a *set but wrong* value — a from address off
/// the sending domain, a display name carrying a line break. That is a
/// different thing to say than "you have not set a key", so it gets its own
/// reason word beside `send_not_configured`, at the same step.
fn misconfigured(err: NigelError) -> ApiError {
    match err {
        NigelError::Invalid(message) => ApiError::conflict(
            message,
            serde_json::json!({ "reason": "send_misconfigured", "step": SendStep::Config.as_str() }),
        ),
        other => other.into(),
    }
}
```

  Replace the stale comment above `build_clients` (it currently says the call
  "cannot fail"). The `contact_email` local replaces `mail.from.clone()`, and the
  preview route's `contact_email_for_preview` call needs no edit — it already
  goes through the function Task 3 rewrote.

- [ ] **Step 4: Add the web sentence** to
  `web/apps/app/src/screens/invoicing-errors.ts`, beside `send_not_configured`:

```ts
case 'send_misconfigured':
  return 'Nigel cannot send with the current email settings. Check `from_email` against `mailgun_domain` in your Nigel configuration.';
```

  No value is interpolated — the module's rule, and this task's AC #7.

- [ ] **Step 5: Verify.** `cargo test --features serve -- --test-threads=1`,
  then from `web/`: `npm run typecheck`, `npm test`, `npm run lint`.

---

### Task 5: End-to-end coverage

**Files:** modify `tests/cli_dispatch.rs`.

`TestEnv` clears the `NIGEL_*` invoicing variables per command, so these run
with nothing configured unless the test writes a settings file.

- [ ] **Step 1: Write the tests.**

```rust
#[test] fn invoice_send_refuses_a_from_address_off_the_mailgun_domain() {
    // write a settings.json with all nine keys, from_email on the wrong domain
    // `invoice send 1248` fails; stderr names mailgun_domain and no value; and
    // the invoice is still a draft (nothing reached Stripe)
}

#[test] fn invoice_preview_names_contact_email_when_neither_key_is_set() {
    // stderr contains "neither contact_email nor from_email"
    // the html contains "(contact_email not configured)"
}

#[test] fn contact_email_is_what_the_page_prints_not_from_email() {
    // settings with from_email=billing@mg.x.test and contact_email=accounts@x.test
    // preview: html contains "accounts@x.test" and not "billing@mg.x.test"
}
```

  The third is AC #3 end to end and is the one that would catch a regression to
  `Branding { contact_email: &mail.from }`.

- [ ] **Step 2: Verify.**
  `cargo test --test cli_dispatch -- --test-threads=1` and the same with
  `--no-default-features --features gusto`.

---

### Task 6: Documentation

Per CLAUDE.md's Documentation Policy the work is not complete until these land.

- [ ] **Step 1: `docs/invoicing.md` configuration table** — three rows after
  `from_email`, with `Required for` reading `—` and the Default column naming the
  fallback:

```
| `from_name` | `NIGEL_FROM_NAME` | — | the business name |
| `reply_to_email` | `NIGEL_REPLY_TO_EMAIL` | — | no Reply-To header |
| `contact_email` | `NIGEL_CONTACT_EMAIL` | — | `from_email` |
```

  and the same three keys in the sample JSON block below it.

- [ ] **Step 2: `docs/invoicing.md` — a "Who the email is from" subsection**
  under Sending: the four keys and their jobs; that `from_email` must be on
  `mailgun_domain` and the reply-to need not be; that a display name with a
  comma or a quote is encoded for you; that `contact_email` is what the page's
  direct-deposit line prints and defaults to `from_email`. Amend step 4 of the
  numbered publish list, which currently describes only the subject.

- [ ] **Step 3: `docs/invoicing.md` placeholder table** — `{{CONTACT}}`'s row
  reads `Direct-deposit contact address (contact_email, or from_email)`.

- [ ] **Step 4: `CLAUDE.md`** — the **Settings** architecture bullet gains the
  three keys; the Key Design Constraints bullet that currently reads
  "`from_email` is also the direct-deposit contact address on the published
  page" is rewritten: `from_email` is the Mailgun From and is validated against
  `mailgun_domain`; `contact_email` is the page's contact line and falls back to
  it; `from_name` falls back to `company_name`; `reply_to_email` is unconstrained
  and absent by default; display names are header-encoded by
  `mailgun::format_address` and a line break in any of them is refused by name.

- [ ] **Step 5: `README.md`** — the configuration paragraph mentions the email
  envelope keys alongside the existing pointer to `docs/invoicing.md`.

- [ ] **Step 6: Verify.** `git diff --stat` shows all three docs touched, and the
  configuration table still lists every key `invoicing_status` names.

---

## Final verification

- [ ] `cargo test -- --test-threads=1`
- [ ] `cargo test --no-default-features --features gusto -- --test-threads=1`
- [ ] `cargo test --no-default-features -- --test-threads=1`
- [ ] `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`
- [ ] From `web/`: `npm run typecheck`, `npm test`, `npm run lint`
- [ ] `git diff src/invoicing/render_html.rs src/invoicing/templates/invoice.html
      src/invoicing/render.rs` is empty — Stream 2 owns those files.
- [ ] `grep -rn "mail.from" src/` returns nothing.
- [ ] Manual, against a scratch data dir: set `NIGEL_FROM_NAME="Carter, Sam"`
      and `NIGEL_CONTACT_EMAIL=accounts@example.test`, run
      `cargo run -- invoice preview 1248`, and read the direct-deposit line.

## Acceptance criteria mapping

| AC | Verified by |
|---|---|
| #1 display name beside the from address | Task 2 `a_plain_display_name_is_not_quoted`; Task 3 `the_display_name_falls_back_to_the_company_name`; Task 4 `a_configured_reply_to_and_display_name_reach_the_mailer` |
| #2 reply-to, independent of the from address | Task 2 `a_reply_to_becomes_the_mailgun_header_field_and_is_absent_otherwise`; Task 4's mailer assertion |
| #3 page contact and email from separately configurable | Task 3 `the_page_contact_falls_back_to_the_from_address`; Task 5 `contact_email_is_what_the_page_prints_not_from_email` |
| #4 comma or quote encoded | Task 2 `a_comma_or_a_quote_is_encoded_not_concatenated` |
| #5 from validated against `mailgun_domain`, reply-to not | Task 2 `the_from_address_must_be_on_the_mailgun_domain`; Task 3's two refusal tests; the absence of any reply-to domain check, commented at the call site |
| #6 env-first, named in the missing list when required, documented | Task 1 `the_new_envelope_keys_resolve_from_the_environment_first` and `the_optional_envelope_keys_never_appear_in_the_missing_list`; Task 6 Step 1 |
| #7 no value in any response or log | Task 1's extended leak list; Task 2's "must name keys, not values" assertions; Task 4's 409 body assertion |
