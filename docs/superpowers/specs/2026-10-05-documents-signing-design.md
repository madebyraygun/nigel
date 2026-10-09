# Documents: filing, sending, online response and countersigning

Covers TASK-109.1, 109.2, 109.3, 109.4, 109.5 and 109.7 for **filed PDFs**. Drafting in
Nigel (109.9 Markdown bodies, 109.10 paginated rendering, 109.7's drafting form and
`wc-markdown-editor`) is the next round and is out of scope here.

## Goal

The operator files a PDF (an agreement, proposal or estimate produced elsewhere), sends it
to a named signer and any collaborators, and each recipient responds on a hosted page:
collaborators can request changes, the signer can request changes or accept by typing their
name. Nigel picks responses up asynchronously, the operator revises or countersigns, and the
whole history — every version, who received it, and every response — stays bound to the
checksummed version it concerns. Everything works from the CLI and the web UI.

A simple electronic signature with an audit trail, not an e-signature platform: the record
is a typed name, an explicit consent, a timestamp, IP and user agent, bound to a checksum.
The signer is told their typed name is their signature; Nigel claims no identity
verification and no qualified (eIDAS) signature.

## Non-goals (this round)

- Drafting documents in Nigel (109.9, 109.10, the drafting half of 109.7).
- Emailed one-time code before accepting — follow-up task. The Worker's request and
  response formats leave room for a `code` field without a format change.
- More than one signer per version — follow-up task. The schema already allows it.
- Tracked changes — follow-up task. It attaches to `document_change_requests`.
- Online decline. Decline is recorded by the operator.
- The TUI screen (109.6), which decision-8 removed.
- Kind management in the web UI; kinds are managed from the CLI.

## Data model (migration v15)

```
document_kinds       id, name UNIQUE, active, position
documents            id, client_id → clients, kind_id → document_kinds, title,
                     token UNIQUE, declined_at, decline_note, withdrawn_at,
                     created_at, updated_at
document_versions    id, document_id → documents, number, file_path, checksum,
                     sent_at, created_at          UNIQUE(document_id, number)
document_recipients  id, version_id → document_versions, role ('signer'|'collaborator'),
                     name, email, token UNIQUE, position
document_signatures  id, version_id, recipient_id NULL, role ('client'|'countersign'),
                     name, email NULL, method ('online'|'manual'), signed_at,
                     typed_name NULL, ip NULL, user_agent NULL, checksum
document_change_requests
                     id, version_id, recipient_id NULL, name, email NULL,
                     method ('online'|'manual'), requested_at, note,
                     ip NULL, user_agent NULL, checksum
```

- Proposal, Estimate and Agreement are seeded by the migration and never reseeded. Kinds are
  rows; there is no Rust enum.
- A version is immutable once `sent_at` is set. The latest version with `sent_at` NULL is the
  draft. `checksum` is SHA-256 of the PDF bytes; `file_path` points into
  `<data_dir>/documents/<document id>/v<n>.pdf`.
- At most one online response per recipient per version: a partial unique index on
  `(version_id, recipient_id)` across signatures and change requests is enforced in the data
  layer (one transaction checks both tables). This is what makes sync idempotent.
- `checksum` on signatures and change requests is the checksum the respondent saw; it must
  equal the version's.
- A send requires exactly one `signer` recipient; the data layer refuses zero or several.
- `clients::delete_blocker` counts documents.
- Structs serialize camelCase. Dates go through `validate_date`.

### Status

Derived by one function from the rows, never written:

| Status | Condition (first match wins) |
|---|---|
| `withdrawn` | `withdrawn_at` set |
| `executed` | a `countersign` signature exists |
| `declined` | `declined_at` set |
| `accepted` | the signer of the latest sent version has a `client` signature on it |
| `changes_requested` | a change request exists on the latest sent version |
| `sent` | the latest version is sent |
| `draft` | otherwise |

Terminal: `withdrawn`, `executed`, `declined`.

### Transitions and guards (typed `NigelError`s in the data layer)

| Action | Allowed from | Effect |
|---|---|---|
| edit title/kind | `draft` | — |
| send | `draft` | freezes the latest version, records recipients |
| revise (new PDF) | `sent`, `changes_requested` | new unsent version; live pages show "being revised" |
| accept (manual) | `sent`, `changes_requested` | `client` signature on the latest sent version |
| request changes (manual) | `sent` | change request on the latest sent version |
| decline (manual, optional note) | `sent`, `changes_requested` | `declined_at` |
| countersign (manual, `--name` required) | `accepted` | `countersign` signature → `executed` |
| withdraw | `draft`, `sent`, `changes_requested` | `withdrawn_at`; pages replaced with a withdrawn notice |

`accepted` admits only countersign. An archived client refuses add, send and revise with the
data layer's sentence. Filing the same checksum for the same client is a structured Conflict
naming the existing document.

## Storage layout

Two R2 buckets:

- **Public bucket** (the existing `r2_bucket`):
  - `d/{token}/v{n}/document.pdf`
  - `d/{token}/{recipient token}/index.html`: one page per recipient, carrying only the
    form for that recipient's role
- **Private bucket** (new `r2_private_bucket`, no public domain). Holds what must not be
  guessable from a page URL:
  - `d/{token}/manifest.json`: written by Nigel at send
  - `d/{token}/v{n}/{recipient token}.json`: written by the Worker

Document URLs are `documents_base_url` if set, otherwise `public_base_url` with its trailing
`/i` replaced by `/d`. Tokens are 128-bit random (`gen_token`). Pages carry
`<meta name="robots" content="noindex">`.

### Manifest

```json
{
  "version": 2,
  "checksum": "sha256:…",
  "state": "open",
  "recipients": [
    { "token": "…", "role": "signer", "name": "Pat Example" },
    { "token": "…", "role": "collaborator", "name": "Sam Example" }
  ]
}
```

`state` is `open` only while the document is `sent`. Any change request, revise, accept,
decline, withdraw or countersign closes it, and Nigel rewrites the manifest when that
happens. A change request therefore stops further online responses on that version: the
other recipients see the "changes requested" notice until the revised version goes out.

### Response (written by the Worker)

```json
{
  "action": "accept",
  "version": 2,
  "checksum": "sha256:…",
  "recipientToken": "…",
  "typedName": "Pat Example",
  "consent": true,
  "note": null,
  "receivedAt": "2026-10-05T17:04:11Z",
  "ip": "203.0.113.7",
  "userAgent": "…"
}
```

`action` is `accept` or `request_changes`. For `request_changes`, `note` is required and
`consent` and `typedName` are omitted.

## Sending (109.3)

`nigel document send <id> [--signer "Name <email>"] [--collaborator "Name <email>"]… [--yes]`.
The signer defaults to the client's billing contact; a billing contact without a name
requires `--signer`. Confirmation is required, as for invoices.

Step trace:

1. config
2. load and guards
3. render pages
4. freeze version
5. publish the PDF and the pages
6. write the manifest
7. email each recipient
8. mark sent

Any failure before step 8 rolls back: the version stays unsent and its recipients are
removed. Objects already published sit at fresh paths and are overwritten on retry.
Outcomes come back as data: `SendStep`, `StepOutcome` and `SendFailure`, in the
`invoicing/send.rs` shape.

- **Emails:** plain text, with the PDF attached and the personal link in the body. The
  signer's subject is "{Company}: {title}: please review and sign"; collaborators get
  "{Company}: {title}: for your review".
- **Preview:** `nigel document preview <id>` renders the pages to local files with no
  network and no configuration. It joins the launch-sync skip list.
- **Revise:** `nigel document revise <id> --file new.pdf` creates the next version as a draft
  and immediately republishes every live page as "This document is being revised", with no
  form, and closes the manifest. The next send reuses the same document token, with new
  recipient tokens.
- **Withdraw:** commits first, then replaces the pages with a withdrawn notice and closes the
  manifest, best-effort. Warnings come back as data.

### Generalizing the invoicing machinery

- **`invoicing/r2.rs`:** `object_key` and the PDF object name take a prefix (`i/` or `d/`),
  and the publisher gains `get` for the private bucket.
- **`invoicing/gateway.rs`:** `AssetPublisher` takes the prefix. A new `ResponseSource`
  trait (`fetch(token, version, recipient_token) -> Option<Response>`, `put_manifest`) is
  backed by R2 now and by Nigel Cloud later (115.4).
- **`invoicing/mailgun.rs`:** the `Mailer` gains a generic send with an attachment filename;
  `send_invoice` becomes a thin wrapper around it.
- **`invoicing/wiring.rs`:** a builder that does not require Stripe.
- **`server/uploads.rs`:** an allowed-extensions list per upload area, plus a PDF magic-byte
  check.

## The Worker (109.4)

`workers/document-response/`: TypeScript, `wrangler.toml`, bindings to the private bucket and
a rate limiter, and deploy docs in `docs/invoicing.md` (Documents section). It is routed on
the documents hostname (`…/d/respond`), so the page posts same-origin and needs no CORS.

`POST /d/respond` with `{ token, recipientToken, version, checksum, action, typedName?, consent?, note? }`:

1. Rate-limit per `recipientToken`; over the limit returns 429.
2. Load `d/{token}/manifest.json`. If it is missing, or `state` isn't `open`, return 404 or
   409.
3. The recipient must be in the manifest, and the role must allow the action: signers may
   `accept` or `request_changes`, collaborators only `request_changes`. Otherwise 403.
4. `version` and `checksum` must equal the manifest's, or return 409 `version_mismatch`.
5. For `accept`: `consent === true`, and `typedName` must equal the recipient's name after
   trimming, collapsing internal whitespace and Unicode case-folding. Otherwise 422.
6. For `request_changes`: `note` is required, 1–4000 characters, plain text. Otherwise 422.
7. Write `d/{token}/v{n}/{recipientToken}.json` with `onlyIf: { etagDoesNotMatch: "*" }`.
   If a response already exists, return 409 `already_responded`.
8. Return 200. The page then shows "Received: thank you".

The Worker never edits pages or the public bucket.

The page form posts with `fetch` and works with JavaScript only. Without JavaScript the page
still shows the document and says how to respond by email. When no `document_response_url` is
configured, pages have no form and manual recording still works.

## Sync (109.4)

`nigel document sync` runs from cron (documented next to `schedule run`) and in the launch
sync next to invoice sync. For each document whose status is `sent` or `changes_requested`:

1. For each recipient of the latest sent version without a recorded response, call
   `ResponseSource::fetch`.
2. Refuse a response whose version or checksum doesn't match the database. This is reported,
   and nothing is recorded.
3. Record it in one transaction: an `accept` becomes a `client` signature, a
   `request_changes` becomes a change request. The `(version, recipient)` uniqueness makes a
   second run a no-op.
4. If the status changed, close the manifest and republish the pages:
   - accepted: stamped "Accepted by {name} on {date}", no form;
   - changes requested: "Changes requested: a revised version is on its way", no form;
   - countersign (manual): stamped with both signatures.

   A failed republish is a warning, not an error.

The report comes back as data (`SyncReport` shape), one line per document: new responses,
refusals, warnings. Exit status follows `invoice sync`.

## CLI (109.2, 109.4)

```
nigel document kinds [list|add <name>|rename <id> <name>|deactivate <id>]
nigel document add --client <id> --kind <name> --title <t> --file <x.pdf>
nigel document list [--client <id>] [--status <s>]
nigel document show <id>
nigel document preview <id>
nigel document send <id> [--signer …] [--collaborator …]… [--yes]
nigel document revise <id> --file <x.pdf>
nigel document accept <id> --name <n> [--date <d>]
nigel document request-changes <id> --name <n> --note <text> [--date <d>]
nigel document decline <id> [--note <text>] [--date <d>]
nigel document countersign <id> --name <n> [--date <d>]
nigel document withdraw <id> [--yes]
nigel document sync
```

All output goes through pure `format_*` functions. `show` lists each version with its
checksum, `sent_at`, recipients, and every signature and change request bound to it. Manual
responses record `method = manual` and no IP or user agent.

## HTTP API (109.5)

These routes sit under the existing loopback-only, cookie-authenticated `data_router`. The
API is for the SPA only; clients never reach it. Errors use the standard envelope.

| Route | Purpose |
|---|---|
| `GET /api/documents` | list (filters: client, status) |
| `POST /api/documents` | multipart: clientId, kind, title, file (PDF magic-checked) |
| `GET /api/documents/{id}` | detail: versions, recipients, signatures, change requests, `can*` flags |
| `PATCH /api/documents/{id}` | title, kind (draft only) |
| `POST /api/documents/{id}/send` | recipients + `confirm`; `confirmation_required` 400 without it; returns the step trace |
| `POST /api/documents/{id}/revise` | multipart PDF |
| `POST /api/documents/{id}/accept` | manual |
| `POST /api/documents/{id}/request-changes` | manual |
| `POST /api/documents/{id}/decline` | manual |
| `POST /api/documents/{id}/countersign` | manual |
| `POST /api/documents/{id}/withdraw` | |
| `POST /api/documents/sync` | |
| `GET /api/documents/{id}/preview`, `/preview.pdf` | sandboxed preview |
| `GET /api/document-kinds` | active kinds |

The `can*` flags call the same guards the data layer enforces. Documented in `docs/api.md`.

## Web UI (109.7, filed PDFs)

`#/documents`, laid out like the invoices screen:

- **List:** `wc-document-table` with a `wc-document-status` badge, filterable by client and
  status. Filing works by dropping a PDF on `wc-dropzone` (`accept=".pdf"`) with client, kind
  and title.
- **Detail:**
  - the preview in `wc-document-frame`;
  - a `wc-document-timeline` with one block per version: checksum, sent date, recipients and
    their role, each signature and change request with method, time and evidence. Notes are
    rendered as text, never as HTML;
  - actions shown only where the server's `can*` flag allows them, each confirmed through
    `wc-confirm`.
- **Send:** `wc-send-dialog` hosts a new `wc-recipient-editor`, with exactly one signer
  (defaulting to the billing contact) and any number of collaborators picked from the
  client's contacts or typed in. It shows the step trace on completion.
- **Errors:** `documents-errors.ts` maps codes to sentences, with the two fallbacks.

New `@nigel/ui` components: `wc-document-table`, `wc-document-status`,
`wc-document-timeline` and `wc-recipient-editor`. Each gets a `.preview.ts` covering its
states and a test calling `describePreviewA11y`, reads `@nigel/theme` tokens, and adopts
`controlsCss` where it hosts `wa-*` primitives. All server access goes through `src/api`.

## Settings

| Key | Purpose |
|---|---|
| `r2_private_bucket` | manifest and responses |
| `documents_base_url` | optional override of the derived `/d` base |
| `document_response_url` | the Worker endpoint; absent means no form |

Each has a `NIGEL_*` environment override, as the invoicing keys do.

## Testing

- **Data layer:** every status in the table, every guard, single-signer enforcement, a
  second response for the same recipient and version refused, checksum binding, and the
  migration on an existing v14 database.
- **CLI:** each verb through `TestEnv`, with the fake publisher, mailer and `ResponseSource`.
  This covers send rollback at each step, revise closing pages, sync idempotence, refusal of
  a version mismatch, and partial republish warnings.
- **API:** route tests per endpoint, including the `can*` flags and `confirmation_required`.
- **Worker:** vitest with an in-memory R2 fake. Covers the role matrix, name normalization,
  consent, note bounds, a closed manifest, a version mismatch, write-once (the second post
  gets 409) and the rate limit.
- **Web:** component previews with a11y, and screen tests through the fake API client: the
  draft → sent → changes requested → revise → sent → accepted → executed flow.
- **Fixtures:** the fictional cast only (Cedar Systems, Juniper Labs, …).

## Build order

1. 109.1, the data layer and migration.
2. In parallel: 109.2, CLI filing, and generalizing the invoicing machinery
   (prefix, `ResponseSource`, mailer, wiring, uploads).
3. 109.3, send, preview, revise and withdraw.
4. In parallel: 109.4, sync and the manual verbs, and the Worker (it depends only on the
   manifest and response formats above).
5. 109.5, the API.
6. 109.7, the web screen. Its components can start alongside 5.

## Backlog changes

- **ACs to update before code:**
  - 109.1: the signatures table, recipients, change requests, the status table;
  - 109.3: recipients, per-recipient pages, the manifest, the private bucket, revise
    closing pages;
  - 109.4: `request_changes`, the Worker contract, response files per version;
  - 109.5: multipart filing and drafted create deferred;
  - 109.7: drafting form and editor deferred, timeline and recipient editor added.
- **Follow-ups to file:** emailed one-time code before accepting; multiple signers per
  version; tracked changes on change requests.
