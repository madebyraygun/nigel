# The outgoing email envelope — display name, from address, reply-to

Task: TASK-80 (epic TASK-86, *invoicing polish and web theme*, Stream 3).

## Problem

`mailgun::message_fields(from, to, subject, html)` is the whole message. `from`
is `MailgunClient.from`, a bare address, so an invoice arrives from
`billing@example.com` with nothing beside it, and there is no `Reply-To` at all —
a client who hits reply writes to the billing alias whether or not anyone reads
it.

Three different things are wanted and today there is one setting for all of
them:

| Want | Constrained by | Today |
|---|---|---|
| A display name the client sees beside the address | nothing (but must be header-encoded) | does not exist |
| The envelope From | must be inside `mailgun_domain` | `from_email` |
| A Reply-To, often a person rather than an alias | nothing | does not exist |

And there is a fourth job `from_email` is quietly doing. `cli/invoice.rs`'s
`send()` builds `Branding { contact_email: &mail.from }` (line 458) and
`server/routes/invoices.rs`'s `send()` does the same (`let contact_email =
mail.from.clone()`, line 592), so the Mailgun From is also the address printed
into `{{CONTACT}}` — *"Contact `billing@example.com` for account details"* in the
direct-deposit paragraph of `templates/invoice.html`. `invoice preview` reads
the same key through `contact_email_for_preview` and prints
`(from_email not configured)` when it is unset.

Those two jobs diverge the moment a reply-to exists: the envelope From is
whatever Mailgun will accept on the verified domain, and the contact line is
whatever a human should write to about money.

## Decision summary

| Key | Env | Required for `send` | Job |
|---|---|---|---|
| `from_email` | `NIGEL_FROM_EMAIL` | yes (unchanged) | the Mailgun **From address**, and nothing else |
| `from_name` | `NIGEL_FROM_NAME` | no | display name beside it; falls back to `company_name` |
| `reply_to_email` | `NIGEL_REPLY_TO_EMAIL` | no | `Reply-To`; absent means no header, today's behaviour |
| `contact_email` | `NIGEL_CONTACT_EMAIL` | no | the `{{CONTACT}}` line on the page; falls back to `from_email` |

Four properties fall out of that table and they are the design:

1. **`from_email` keeps its name and keeps meaning the From address.** Renaming
   it would break every existing `settings.json`, every documented `NIGEL_*`
   variable, `docs/invoicing.md`, the `invoicing_status` missing-list order and
   three tests that assert that order. The key whose name says "from" keeps the
   From job; the page's contact line is what moves out.
2. **The three new keys are all optional**, because all three have an honest
   fallback (company name, no header, the from address). None of them appears in
   `invoicing_status.missing`, and the nine-key list in `settings.rs` is
   unchanged — see "The missing list" below, which is AC #6's real answer.
3. **The from address is validated against `mailgun_domain`; the reply-to is
   not.** Different constraints, so different code paths, both pure functions in
   `mailgun.rs`.
4. **Nothing about a display name is concatenated into a header.** One function
   builds the header value, and it quotes, escapes, and refuses control
   characters.

## Configuration

### `from_name` and the `company_name` fallback

`from_name` resolves through `invoicing_config()` like the other nine
(`env_or("NIGEL_FROM_NAME", &s.from_name)`), so precedence, the `TempConfigDir`
env suppression, and the settings file all work with no new machinery.

When it is unset, the display name falls back to the database's `company_name`
metadata — the same value `send.rs` already puts in the subject line
(`Invoice #1248 from Acme LLC`) and the same one `Branding.company` carries onto
the page. A business that has set its name in the settings screen therefore gets
AC #1 with no new configuration at all, and `from_name` is the override for the
case where the mail name differs from the legal name.

`settings::invoicing_config()` cannot see the database, so the fallback is
applied where both halves are already in hand — the CLI layer, which is where
`src/invoicing/`'s "reads no settings" rule puts all config resolution:

```rust
// src/cli/invoice.rs
pub(crate) fn build_clients(
    cfg: InvoicingConfig,
    company: &str,
) -> Result<(StripeClient, R2Publisher, MailgunClient)>;
```

`company` is `company_name(conn)`. The CLI's `send()` already has a connection.
The HTTP route resolves it in its own `with_conn` before building the clients —
one extra connection open on a request that is about to make five network calls,
and it keeps `build_clients` outside `with_conn_api` where the `409` for missing
config still lands before the database is touched.

Empty company and unset `from_name` means no display name: a bare address, which
is exactly today's message.

### `contact_email` and the page

`Branding.contact_email` stops being `mail.from` and becomes:

```rust
pub(crate) fn contact_address(cfg: &InvoicingConfig) -> Option<String>
    // cfg.contact_email.or(cfg.from_email)
```

Falling back to `from_email` means every existing installation renders exactly
the page it rendered before. `contact_email_for_preview` is rewritten in terms
of `contact_address`, and its placeholder constant becomes:

```rust
const PREVIEW_CONTACT_PLACEHOLDER: &str = "(contact_email not configured)";
```

The notice `preview` prints names both keys, because either one satisfies it:

```
notice: neither contact_email nor from_email is configured — the direct-deposit contact line is a placeholder
```

`server/routes/invoices.rs`'s preview route calls the same function and its test
asserting `(from_email not configured)` moves with the constant.

### The missing list

`invoicing_status` keeps its nine keys in the documented order. AC #6 says a new
key must appear "by name in `send_not_configured` **when required** and unset";
none of the three is required, so none appears — and the guard against that
being an accident is a test:

```rust
#[test]
fn the_optional_envelope_keys_never_appear_in_the_missing_list() {
    let mut cfg = fully_configured();
    cfg.from_name = None;
    cfg.reply_to_email = None;
    cfg.contact_email = None;
    let status = invoicing_status(&cfg);
    assert!(status.send_configured, "optional keys cannot block a send");
    assert!(status.missing.is_empty());
}
```

AC #7 holds by construction: `InvoicingStatus` still serializes only
`&'static str` key names, and
`the_invoicing_status_never_carries_a_value` gains the three new values to its
leak list.

## The header

### One function builds the From value

```rust
// src/invoicing/mailgun.rs

/// The sender identity of one message: what Mailgun puts in `From` and, when
/// set, in `Reply-To`. A value type with no configuration in it, because
/// `src/invoicing/` reads no settings — `cli::invoice::build_clients` resolves
/// it and passes it in.
pub struct EmailEnvelope {
    pub from_address: String,
    pub from_name: Option<String>,
    pub reply_to: Option<String>,
}

/// `Name <addr>` per RFC 5322, or the bare address when there is no name.
pub fn format_address(display_name: Option<&str>, address: &str) -> String;

/// The from address must be on the verified sending domain; Mailgun will
/// refuse anything else with a 400 the operator has to go and read.
pub fn validate_from_address(from: &str, domain: &str) -> Result<()>;

/// No header value may carry CR or LF.
pub fn validate_header_value(value: &str, what: &str) -> Result<()>;
```

`MailgunClient` becomes `{ api_key, domain, envelope }` and `message_fields`
takes the envelope:

```rust
pub fn message_fields(
    envelope: &EmailEnvelope,
    to: &str,
    subject: &str,
    html: &str,
) -> Vec<(String, String)>;
```

producing `from`, `to`, `subject`, `html`, and — only when `reply_to` is set —
`h:Reply-To`, which is Mailgun's form field for a custom header. An absent
reply-to emits no field at all, rather than an empty one.

### Encoding rules (AC #4)

`format_address` implements the RFC 5322 `name-addr` production, no more:

| Display name | Header value |
|---|---|
| `None`, or empty/whitespace after trimming | `billing@example.com` |
| `Bluepeak` | `Bluepeak <billing@example.com>` |
| `Carter, Sam` | `"Carter, Sam" <billing@example.com>` |
| `Sam "Nigel" Carter` | `"Sam \"Nigel\" Carter" <billing@example.com>` |
| `Bluepeak \ Co` | `"Bluepeak \\ Co" <billing@example.com>` |
| `Bluepeak — Books` (non-ASCII) | `Bluepeak — Books <billing@example.com>`, UTF-8 |

- A name is quoted when it contains anything outside RFC 5322 `atext` plus
  spaces — that set covers the comma, the semicolon, the colon, `<`, `>`, `@`,
  the parentheses, the brackets, the backslash and the double quote. Inside the
  quotes only `"` and `\` are escaped, which is all `quoted-string` allows.
- A name is trimmed first, so a name of only spaces produces a bare address
  rather than `"" <a@b>`.
- **CR and LF are refused, not stripped**, by `validate_header_value` on the
  display name, the from address and the reply-to. Stripping would silently send
  a message the operator did not write; refusing names the setting. This is
  header-injection defence: a `from_name` of `"Bluepeak\r\nBcc: someone@else"`
  would otherwise add a recipient nobody chose. It is a real risk even though
  the value comes from local configuration, because `NIGEL_FROM_NAME` is an
  environment variable and environments get assembled by CI systems.
- **RFC 2047 encoded-words are deliberately not implemented.** Mailgun's API is
  UTF-8 and accepts a UTF-8 display name in the `from` form field; implementing
  `=?UTF-8?B?…?=` would mean adding a base64 dependency for a case the upstream
  already handles. A test pins the pass-through so the decision is visible if it
  ever turns out to be wrong.

### Domain validation (AC #5)

```
from_email = billing@mg.example.com,  mailgun_domain = mg.example.com   → ok
from_email = billing@MG.EXAMPLE.COM,  mailgun_domain = mg.example.com   → ok (ASCII case-insensitive)
from_email = billing@example.com,     mailgun_domain = mg.example.com   → refused
from_email = billing@sub.mg.example.com, mailgun_domain = mg.example.com → refused
from_email = billing                                                     → refused
```

The comparison is an exact, case-insensitive match on the domain part.
Subdomains are refused because Mailgun would refuse them too, and the point of
validating locally is to fail before a network call rather than after one.

The refusal is a `NigelError::Invalid` naming both keys and neither value:

```
from_email is not on mailgun_domain — Mailgun will refuse it. The from address's domain must match mailgun_domain exactly.
```

Naming no value keeps AC #7 true for the error path as well as the status path.

`reply_to_email` gets no domain check and no shape check beyond
`validate_header_value`. That is the same posture the rest of the codebase takes
on addresses — `nigel client add` does not shape-check `--email`, and
`wc-client-form` deliberately does not either, with the reason recorded in its
doc comment. `from_email` is the exception only because AC #5 requires it, and
what it checks is a *domain match*, not address well-formedness.

### Where the validation runs

Inside `build_clients`, immediately after the four Mailgun-ish values are
resolved, so both front ends refuse identically and neither reaches Mailgun with
a message it will bounce. The CLI prints the sentence. The HTTP send route gains
one arm: `build_clients` can now fail for a reason other than a missing key, and
the route's comment claiming it "cannot fail" stops being true, so the route maps
`NigelError::Invalid` from that call to a `409` with

```json
{ "reason": "send_misconfigured", "step": "config" }
```

beside the existing `send_not_configured`. Same shape, same step, a different
reason word — because "you have not set a key" and "the key you set is wrong"
are different things for a screen to say. `invoicing-errors.ts` gains the
matching entry; the two 409s render different sentences and neither carries a
value.

## What a sent message looks like

```
From: Bluepeak <billing@mg.example.com>
Reply-To: sam@example.com
To: ap@acme.test
Subject: Invoice #1248 from Bluepeak
```

with the page's direct-deposit paragraph reading *"Contact
`accounts@example.com` for account details"* when `contact_email` is set to
something other than the From — which is the whole point of AC #3.

## Files this touches

| File | Change |
|---|---|
| `src/settings.rs` | four fields on `Settings` and `InvoicingConfig`; three new `env_or` lines; `invoicing_status` unchanged |
| `src/invoicing/mailgun.rs` | `EmailEnvelope`, `format_address`, `validate_from_address`, `validate_header_value`; `MailgunClient.from` → `.envelope`; `message_fields` signature |
| `src/cli/invoice.rs` | `build_clients(cfg, company)`; `contact_address`; `contact_email_for_preview` and its constant; `send()` and `preview()` call sites |
| `src/server/routes/invoices.rs` | resolve `company_name` before `build_clients`; `send_misconfigured` 409; preview route's contact resolution |
| `src/cli/invoice_manager.rs` | two `build_clients` call sites (lines 1048, 1107) gain the company argument |
| `web/apps/app/src/screens/invoicing-errors.ts` | `send_misconfigured` sentence |
| `docs/invoicing.md` | three rows in the configuration table, the sample JSON, a short "Who the email is from" section, `{{CONTACT}}`'s row in the placeholder table |
| `CLAUDE.md`, `README.md` | settings list and the invoicing key design constraint |

Nothing in `src/invoicing/send.rs`, `render.rs` or `render_html.rs` changes:
`Branding` keeps its three fields and `{{CONTACT}}` keeps its name. The value
flowing into `contact_email` is the only thing that moves.

## Out of scope

- Editing any of these keys from the settings screen or the web settings API.
  All twelve invoicing keys are file-and-environment only today and this task
  does not change that.
- A per-client or per-invoice reply-to. One installation, one envelope.
- Multiple recipients — that is TASK-77, which builds `to`/`cc` on top of
  `format_address` (see below).
- The email *body*: subject wording, a text part, or templating the message.

## Interaction with TASK-77 (PR-3c)

TASK-77 gives a client several contacts, each with a name, and needs
`Cc:` with display names on it. That is `format_address` applied to a recipient
instead of a sender, so **TASK-80 must land first** and TASK-77 adds `to`/`cc`
to `message_fields` around an envelope that already exists. Doing them the other
way round would mean writing recipient-header formatting twice, or writing 77's
cc support against a `from` that is still a bare string.

## Open questions for the orchestrator

1. **`contact_email` as the new key, rather than renaming `from_email` to
   something like `mail_from`.** Settled above on compatibility grounds. Say the
   word if you would rather break the key name once and be left with names that
   read better, and this becomes a rename plus a one-release fallback.
2. **The `company_name` fallback for `from_name`.** It makes AC #1 true without
   configuration for anyone who has set a business name, at the cost of
   `build_clients` taking an argument it did not take before. The alternative is
   that an unset `from_name` means no display name, full stop.
3. **UTF-8 display names pass through unencoded.** Confirmed working through
   Mailgun's API in normal use, but not tested against their service here. If
   you want RFC 2047, it is a base64 dependency plus about fifteen lines.
4. **`send_misconfigured` as a second config-time 409 code.** The alternative is
   to fold it into `send_not_configured` with a `problem` field, which keeps the
   web table smaller but makes one code mean two things.
5. **Subdomains refused.** If you actually send from `billing@example.com` with
   `mailgun_domain = mg.example.com`, this validation will refuse a configuration
   Mailgun might accept via a parent-domain setup. Confirm the exact-match rule
   against your live Mailgun domain before this ships.
