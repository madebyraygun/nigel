---
id: TASK-67
title: >-
  Invoicing: published URL must not depend on a directory index, and
  public_base_url needs validation
status: Done
assignee:
  - '@stream-2'
created_date: '2026-08-07 23:10'
updated_date: '2026-08-11 20:49'
labels:
  - invoicing
  - bug
dependencies: []
references:
  - 'archived PR #172'
documentation:
  - docs/superpowers/specs/2026-08-11-task-67-64-publish-pipeline-design.md
  - docs/superpowers/plans/2026-08-11-task-67-64-publish-pipeline.md
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Two related failures from a real send against billing.example.com:

1. Nigel prints and emails {public_base_url}/{token}/, but an R2 custom domain does not serve index documents, so the directory URL 404s while .../{token}/index.html works. Either emit the full index.html URL everywhere the link is printed, emailed, or attached to the Stripe link (robust on any static host), or document that the docs' "served at .../{token}/" claim requires an edge rewrite (Cloudflare rule/Worker appending index.html) and make that a setup step in docs/invoicing.md.

2. public_base_url accepted "billing.example.com" (no scheme, no /i prefix) silently, producing broken links in a real email. Validate at send time: require an http(s) scheme, and warn when the path does not end in the /i prefix Nigel writes keys under.

Found during pre-merge testing of PR #172.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The link printed by send and embedded in the email resolves on a plain R2 custom domain with no edge rewrite, or the rewrite requirement is documented as a required setup step
- [x] #2 A public_base_url without an http(s) scheme is rejected by name before anything is published or emailed
- [x] #3 A public_base_url whose path does not end in /i produces a warning
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- `invoicing::r2::public_url` now names the object: `{base}/{token}/index.html`, with `PAGE_OBJECT`/`PDF_OBJECT` consts so the address and the key `AssetPublisher::publish` writes cannot drift (`the_address_and_the_key_name_the_same_object`).
- Two pure validators beside it. `validate_public_base_url` refuses an empty value, any value containing whitespace, anything without a case-insensitive `http://`/`https://` prefix, and anything with no host before the path — quoting the offending value, because it is a public address typed by the operator who ran the command. `public_base_url_warning` answers the `/i`-prefix sentence and quotes nothing, because it travels on `invoicing_status`, which carries key names only.
- The refusal is called from `cli::invoice::build_clients` — the one constructor both send paths use — after the nine `require()` checks, so the "missing key" ordering in docs/invoicing.md is unchanged and nothing is published, emailed or charged when it fires. `optional_publisher` is deliberately untouched: void and (later) republish only need the upload.
- The warning is computed once, in `settings::invoicing_status`, as `public_base_url_warning`. `nigel invoice send` prints it as `notice:` before doing anything; `GET /api/status` carries it as `invoicing.publicBaseUrlWarning`. The no-values invariant test now also renders a status that carries the warning.
- `POST /api/invoices/{n}/send` tags the `build_clients` failure `SendStep::Config`, so an unusable base URL is filed under the same step every other config refusal is.
- Docs: docs/invoicing.md's "Hosting" section no longer claims the object is served at `…/{token}/` — it is served at `…/{token}/index.html`, and the edge rewrite is documented as an option rather than a requirement; "Configuration" gains the send-time validation rules; "Sending" step 3 and the sample output line updated. docs/api.md gains `publicBaseUrlWarning`, the file-form `publicUrl`, and the config-step 400. CLAUDE.md's Invoicing bullet gains an `r2.rs` clause and the published-invoice constraint records the file-URL rule and where each check lives.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Nigel now hands out an address that resolves on a plain static host, and refuses a `public_base_url` that cannot produce one.

`public_url` names `index.html` instead of the directory, which fixes every surface at once: the CLI's `Sent invoice #N: …` line, `SendResult.publicUrl`, `InvoiceDetail.publicUrl`, the TUI's detail view and the SPA. The email never carried the address, so there was no second place to chase.

The setting behind it is checked in two strengths. `cli::invoice::build_clients` — the single constructor `nigel invoice send` and `POST /api/invoices/{n}/send` both pass through — refuses a value with no http(s) scheme or no host, by name and quoting the value, before any Stripe link, upload or email exists; over HTTP that lands as a 400 tagged `step: "config"`. A base URL whose path does not end in `/i` is a warning instead, computed once in `settings::invoicing_status` so the CLI notice and `/api/status`'s `publicBaseUrlWarning` are one sentence, and carrying no configured value.

`optional_publisher` stays lenient, so void still works on a misconfigured installation.

Tests: 9 in `invoicing::r2` (address, key parity, both validators), 2 in `cli::invoice` (refused / still builds), 3 in `settings` (warns, unset-is-missing-not-warned, the extended no-values invariant), 1 route test (`a_send_with_an_unusable_public_base_url_fails_at_config`), 2 end-to-end in `tests/cli_dispatch.rs` (send refused with the invoice still a draft; preview unaffected).
<!-- SECTION:FINAL_SUMMARY:END -->
