---
id: TASK-80
title: 'Invoicing: outgoing email needs a display name, a from address and a reply-to'
status: Done
assignee:
  - '@stream-3'
created_date: '2026-08-10 21:48'
updated_date: '2026-08-11 23:30'
labels:
  - enhancement
  - invoicing
  - email
dependencies: []
documentation:
  - docs/superpowers/specs/2026-08-11-task-80-email-envelope-design.md
  - docs/superpowers/plans/2026-08-11-task-80-email-envelope.md
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
mailgun::message_fields takes from, to, subject and html, and that is the whole message. The From is a bare address, so an invoice arrives from billing@example.com with no business name beside it, and there is no Reply-To at all — a client hitting reply writes to whatever address happens to be sending.

Three things are wanted and they are not the same field: a display name (the business name the client sees), the from address (what the envelope and Mailgun use, constrained to the verified mailgun_domain), and a reply-to (which is unconstrained and is often a person rather than a billing alias).

There is an existing conflation to resolve. from_email is doing two unrelated jobs: it is the Mailgun From, and it is also the direct-deposit contact line printed on the published invoice page — cli/invoice.rs passes it in as the CONTACT value, and warns and prints a placeholder when it is unset. Those two want to diverge as soon as a reply-to exists, so the settings need to say which is which rather than reusing one key for both.

Note that mailgun_domain constrains the from address but not the reply-to, so validation differs between the two, and a display name containing a comma or a quote has to be encoded rather than concatenated into the header.

New settings need the full treatment the existing nine get: a key in settings.json, a matching NIGEL_* variable, resolution through invoicing_config with env winning, a name in the missing list that send_not_configured reports, and a row in docs/invoicing.md. Whether any of them are required or all optional is part of the decision — today from_email is required for send.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A sent invoice arrives with the business display name beside the from address
- [x] #2 A reply-to address can be set and is honoured, independently of the from address
- [x] #3 The published page contact line and the email from address are separately configurable — from_email no longer does both jobs implicitly
- [x] #4 A display name containing a comma or a quote is encoded correctly in the header
- [x] #5 The from address is validated against mailgun_domain; the reply-to is not
- [x] #6 Any new key resolves from its NIGEL_* variable first, appears by name in send_not_configured when required and unset, and is documented in docs/invoicing.md
- [x] #7 No response or log carries a configured value, only key names — the existing status contract holds
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Three optional settings — `from_name`, `reply_to_email`, `contact_email` — resolve
through `invoicing_config()` beside the existing nine, env-first, and none of them
appears in `invoicing_status.missing`: each has an honest fallback (the business
name, no header, `from_email`).

`src/invoicing/mailgun.rs` gained `EmailEnvelope` plus three pure functions —
`format_address` (RFC 5322 `name-addr`: quotes when the name leaves `atext`,
escapes `\` and `"`, passes UTF-8 through because Mailgun's API is UTF-8),
`from_address_domain_warning`, and `validate_header_value`. `MailgunClient.from`
became `.envelope`, and `message_fields` emits `h:Reply-To` only when one is set.

Orchestrator OVERRIDE applied: a from address that does not match `mailgun_domain`
**warns and sends** rather than refusing, so the spec's `validate_from_address ->
Result<()>` shipped as `from_address_domain_warning -> Option<String>` and
`build_clients` prints it on stderr. A CR/LF in `from_name` or `reply_to_email`
stays a hard refusal naming the key, which is what still produces the route's
`409 send_misconfigured`.

`cli::invoice::build_clients(cfg, company)` now resolves the envelope, with an
unset `from_name` falling back to the database's `company_name`.
`Branding.contact_email` comes from `contact_address(cfg)` — `contact_email` or
`from_email` — so the page's direct-deposit line and the Mailgun From are two
settings. `PREVIEW_CONTACT_PLACEHOLDER` became `(contact_email not configured)`.

The HTTP send route resolves the company name before building clients and maps
`NigelError::Invalid` from that call to a 409 `send_misconfigured` at step
`config`; `invoicing-errors.ts` and `CONFLICT_REASONS` gained the matching entry.

Docs: `docs/invoicing.md` (three config rows, sample JSON, a "Who the email is
from" section, the `{{CONTACT}}` placeholder row), `CLAUDE.md` (Settings bullet
and the rewritten invoicing-config design constraint), `README.md`.
<!-- SECTION:NOTES:END -->
