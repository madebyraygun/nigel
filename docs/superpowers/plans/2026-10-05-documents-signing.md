# Documents Signing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** File a PDF against a client, send it to one signer and any collaborators, take their responses (accept by typed name, or request changes) through a hosted page and a Cloudflare Worker, sync those responses back, revise or countersign, and keep every version, recipient and response bound to the checksummed version it concerns — from the CLI and the web UI.

**Architecture:** A new `nigel_core::documents` module (model, status, guards, store, record, wire, render, send, lifecycle, sync) sits beside `invoicing` and reuses its machinery through narrow generalizations: an R2 key prefix and a `DocumentPublisher` trait, a private-bucket `ResponseSource`, a generic `Mailer::send` with an attachment filename, and a Stripe-free client builder. Status is derived from rows by one function and never written. The CLI (`crates/nigel/src/cli/document.rs`), the HTTP routes (`server/routes/documents.rs`) and the SPA screen (`#/documents`) call the same data layer. A small TypeScript Worker (`workers/document-response/`) validates responses against a private manifest and writes each response once.

**Tech Stack:** Rust (rusqlite/SQLCipher, rusty-s3, reqwest blocking, axum), Lit + Web Awesome (`@nigel/ui`, `@nigel/app`), vitest, Cloudflare Workers (R2 binding, Rate Limiting binding).

**Spec:** `docs/superpowers/specs/2026-10-05-documents-signing-design.md` (authoritative). Backlog: TASK-109.1, 109.2, 109.3, 109.4, 109.5, 109.7.

---

## Global Constraints

- The spec is authoritative; where a backlog description disagrees (109.3's single `d/{token}/index.html`, 109.5's separate `/upload` route), the spec wins and the ACs already say so.
- Public repository: no real book data anywhere — code, tests, fixtures, docs, commit messages. Fixtures use the fictional cast only (Cedar Systems, Juniper Labs, Harbor & Vale, Acme, Globex, Initech) and the people "Pat Example" and "Sam Example".
- Every commit passes `.githooks/pre-commit`; judge `./scripts/check-no-real-data.sh` by exit status, never by grepping its output.
- Comments: none that justify an edit, explain why a config entry exists, or warn against reverting. Docs describe the current state; no "added in", no migration history.
- Rust tests always run serially: `-- --test-threads=1`.
- Every visual change ships through `@nigel/ui`: `wc-foo.ts` + `wc-foo.preview.ts` (all visible states) + `wc-foo.test.ts` calling `describePreviewA11y(preview)`; no primitives in `web/apps/app/src/components/`.
- A component that renders a `wa-*` primitive adopts `controlsCss`: `static styles = [controlsCss, css\`…\`]`; Web Awesome imports are cherry-picked (`@awesome.me/webawesome/dist/components/<x>/<x>.js`).
- All SPA server access goes through `web/apps/app/src/api/` (`api-seam.test.ts` stays green).
- Schema: migration **v15**, appended to `MIGRATIONS` in `crates/nigel-core/src/migrations.rs`; `LATEST_VERSION` follows automatically.
- Kinds are rows (Proposal, Estimate, Agreement seeded once); no Rust enum mirrors the kind list.
- Status is derived by one function from the rows and never written; `documents` has no `status` column.
- A send requires **exactly one** `signer` recipient.
- A change-request note is 1–4000 characters of plain text.
- Document and recipient tokens are **128-bit** random: 32 lowercase hex characters.
- Checksums are stored and compared as `sha256:<64 lowercase hex>` everywhere (DB, manifest, response).
- At most one online response per `(version_id, recipient_id)` across signatures and change requests, enforced in one data-layer transaction and backed by a partial unique index per table.
- New settings keys: `r2_private_bucket`, `documents_base_url`, `document_response_url`, with env overrides `NIGEL_R2_PRIVATE_BUCKET`, `NIGEL_DOCUMENTS_BASE_URL`, `NIGEL_DOCUMENT_RESPONSE_URL`.
- Object layout: public `d/{token}/v{n}/document.pdf`, `d/{token}/{recipient token}/index.html`; private `d/{token}/manifest.json`, `d/{token}/v{n}/{recipient token}.json`.
- Every printed or returned page URL names the `index.html` object, never the directory.
- Pages carry `<meta name="robots" content="noindex">`; notes and names are HTML-escaped on every surface.
- All outbound traffic goes through `DocumentPublisher` / `Mailer` / `ResponseSource`; no test can reach the network.
- Structs on the wire serialize camelCase; `token` fields are `#[serde(skip_serializing)]`.
- Dates go through `invoicing::invoices::validate_date`; online timestamps through `documents::model::validate_moment`.
- `nigel-core` never reads settings inside `documents/` except through `invoicing/wiring.rs` and `settings.rs`; the data layer never reads the clock (callers pass `today`).
- Recorded assent, not a legal e-signature product — say so on every surface that describes signing.

## Review Focus

Six inputs the happy path never exercises, each pinned by a named test in the owning task:

1. **A file named `.pdf` that is not a PDF** (HTML, PNG, `%PDF-` appearing after byte 0) — refused by content. Pinned in Task 5 (`ensure_pdf_refuses_content_that_only_claims_to_be_a_pdf`), Task 13 (`a_document_upload_named_pdf_holding_html_is_refused_by_content`) and Task 25 (`a_document_upload_named_pdf_holding_html_is_a_400`).
2. **A signer name typed with different case, extra or internal whitespace, or NFD diacritics** — accepted; a different letter (`Zoe` for `Zoë`) refused. Pinned in Task 22 (`names_match_across_case_whitespace_and_normalization_but_not_letters`).
3. **A response written for version 1 synced after revise sent version 2** — never recorded. Pinned in Task 20 (`a_response_for_an_earlier_version_is_refused_and_records_nothing`).
4. **Send failing after some emails went out** — recipients removed, version still a draft, manifest closed, the failure lists who was emailed. Pinned in Task 16 (`a_mail_failure_after_the_first_recipient_rolls_back_and_closes_the_manifest`).
5. **A double submit racing at the Worker** — exactly one response object, the second post gets 409 `already_responded`. Pinned in Task 23 (`two_concurrent_posts_write_one_response`).
6. **HTML or script in a change-request note** — stored verbatim, rendered as text by the CLI, the published page and the timeline. Pinned in Task 6 (`a_note_with_markup_is_stored_verbatim`), Task 8 (`a_note_prints_as_its_own_text`), Task 14 (`a_note_or_name_with_markup_is_escaped_on_the_page`) and Task 30 (`a_note_with_markup_renders_as_text`).

## File Structure

### Rust — `crates/nigel-core`

| File | Responsibility |
|---|---|
| `src/lib.rs` (modify) | `pub mod documents;` |
| `src/migrations.rs` (modify) | v15: six tables, partial unique indexes, one-time kind seed |
| `src/error.rs` (modify) | `BlockReason::HasDocuments(i64)`, `DeleteBlock::documents` |
| `src/documents/mod.rs` | Module map and the recorded-assent statement |
| `src/documents/model.rs` | Row structs, enums (`DocumentStatus`, `RecipientRole`, `SignatureRole`, `Method`), tokens, checksums, recipient parsing, note and moment validation |
| `src/documents/kinds.rs` | Kind rows: list, add, rename, deactivate, resolve by name |
| `src/documents/status.rs` | `StatusFacts`, `status_from`, `document_status` — the one status function |
| `src/documents/guards.rs` | `Action`, the guard table, `can`, `ensure_allowed`, archived-client guard |
| `src/documents/store.rs` | PDF check, file storage, filing, reads, list, detail record, draft edit |
| `src/documents/record.rs` | Transitions: add version, freeze/unfreeze recipients, mark sent, manual and online responses, decline, countersign, withdraw |
| `src/documents/wire.rs` | Manifest and Worker-response JSON formats, private keys |
| `src/documents/render.rs` | Recipient pages, notices, email text, attachment name — one seam for preview and send |
| `src/documents/send.rs` | Traced send: steps, outcome, failure, rollback; default signer; preview writer |
| `src/documents/lifecycle.rs` | Revise and withdraw with best-effort republish; republish after a status change |
| `src/documents/sync.rs` | Pull responses through `ResponseSource`, refuse mismatches, record once, report as data |
| `src/documents/testing.rs` | `cfg(any(test, feature = "testutil"))` fakes and fixtures shared by core, CLI and route tests |
| `src/invoicing/clients.rs` (modify) | `ensure_client_active_for`; `delete_blocker` counts documents |
| `src/invoicing/r2.rs` (modify) | `KeyPrefix`, prefixed `object_key`, document object names/URLs, `R2PrivateStore` (get/put) implementing `ResponseSource`, `DocumentPublisher` for `R2Publisher` |
| `src/invoicing/gateway.rs` (modify) | `DocumentPublisher`, `ResponseSource`, generic `Mailer::send` + `OutgoingMail`/`Attachment` |
| `src/invoicing/mailgun.rs` (modify) | `MailgunClient` implements `Mailer::send` with the attachment filename |
| `src/invoicing/wiring.rs` (modify) | Shared mail builder; `DocumentClients`, `build_document_clients`, optional publisher/source |
| `src/invoicing/render_html.rs` (modify) | `esc` becomes `pub(crate)` |
| `src/invoicing/{send,schedules}.rs` (modify, tests only) | Fake mailers implement `send` |
| `src/settings.rs` (modify) | Three keys, `DocumentsConfig`, `documents_config`, `DocumentsStatus`, `derive_documents_base` |
| `src/server/uploads.rs` (modify) | `UploadArea`, per-area allowed extensions, `check_content` (PDF magic) |
| `src/server/routes/documents.rs` | `/api/documents*` and `/api/document-kinds` |
| `src/server/routes/mod.rs` (modify) | Mount `documents::routes()` in `data_router` |
| `src/server/routes/invoices.rs` (modify, tests only) | Fake mailer implements `send` |
| `src/server/error.rs` (modify) | `impl From<DocumentSendFailure> for ApiError` |
| `src/server/testutil.rs` (modify) | Document routes in `DATA_ROUTES`/`WRITE_ROUTES`/`PREVIEW_ROUTES`; `multipart_form`; `seed_document` |

### Rust — `crates/nigel`

| File | Responsibility |
|---|---|
| `src/cli/document.rs` | Every `nigel document` verb, `format_*` printers, `*_with` seams for fakes |
| `src/cli/mod.rs` (modify) | `Commands::Document`, `DocumentCommands`, `DocumentKindsCommands` |
| `src/main.rs` (modify) | Dispatch; one `launch_sync_allowed` predicate; launch document sync |
| `src/cli/invoice_manager.rs` (modify, tests only) | Fake mailer implements `send` |
| `tests/common/mod.rs` (modify) | Three new env vars cleared per command |
| `tests/cli_dispatch.rs` (modify) | End-to-end document verbs that reach no network |

### Worker — `workers/document-response/`

| File | Responsibility |
|---|---|
| `package.json`, `package-lock.json`, `tsconfig.json`, `vitest.config.ts`, `.gitignore` | Standalone npm project (outside the `web/` workspaces) |
| `wrangler.toml` | R2 binding `PRIVATE`, rate limiter `RATE_LIMITER`, route on `…/d/respond` |
| `src/env.ts` | Minimal binding interfaces (`PrivateBucket`, `Limiter`, `Env`) |
| `src/validate.ts` | Pure: name normalization, request parsing, the role/version/consent/note rules |
| `src/index.ts` | `fetch` handler: rate limit → manifest → validate → write-once → 200 |
| `test/memory-r2.ts` | In-memory R2 fake honouring `onlyIf.etagDoesNotMatch: "*"` |
| `test/validate.test.ts`, `test/index.test.ts` | Role matrix, normalization, consent, note bounds, closed manifest, mismatch, write-once, rate limit |
| `.github/workflows/ci.yml` (modify) | Install, typecheck and test the Worker |

### Web — `web/packages/ui/src/components/`

| File | Responsibility |
|---|---|
| `wc-document-status.ts` (+ `.preview.ts`, `.test.ts`) | Status chip for the seven document statuses |
| `wc-document-table.ts` (+ preview, test) | Document list |
| `wc-document-timeline.ts` (+ preview, test) | One block per version: checksum, sent date, recipients, signatures, change requests with evidence; notes as text |
| `wc-recipient-editor.ts` (+ preview, test) | Exactly one signer and any collaborators, from contacts or typed |
| `wc-send-dialog.ts` (+ preview, test) (modify) | `mode="document"` and a `recipients` slot |
| `index.ts` (modify) | Exports |

### Web — `web/apps/app/src/`

| File | Responsibility |
|---|---|
| `api/types.ts` (modify) | Document wire types, new conflict reasons |
| `api/client.ts` (modify) | `ApiClient` document methods + `FetchApiClient` implementations |
| `api/desktop-client.ts` (modify) | `documentPreviewTarget` saves through the native side |
| `__mocks__/fake-api-client.ts` (modify) | Document fakes with the status transitions |
| `screens/documents-data.ts` (+ test) | Pure mappers: rows, filters, timeline views, recipient defaults, send step labels, requests |
| `screens/documents-errors.ts` (+ test) | Reason → sentence, with the two fallbacks; send failure view |
| `screens/documents.ts` (+ test) | `nigel-documents-screen`: list, filing, detail, actions, send |
| `screens/registry.ts` (modify) | `documents` screen def |
| `screens/invoicing-errors.ts` (modify) | `has_documents` sentence for client delete |

### Docs

`README.md`, `docs/architecture.md`, `docs/commands.md`, `docs/api.md`, `docs/invoicing.md` (Documents section, Worker deploy, cron), `docs/design-constraints.md`.

## Phases and parallelism

| Phase | Tasks | Runs |
|---|---|---|
| 1 Data layer (109.1) | 1–6 | Sequential |
| 2 | Track A CLI filing (109.2): 7–8 · Track B machinery: 9–13 | A ∥ B; sequential inside each track |
| 3 Send/preview/revise/withdraw (109.3) | 14–18 | Sequential |
| 4 | Track A sync + manual verbs (109.4): 19–21 · Track B Worker: 22–23 | A ∥ B |
| 5 HTTP API (109.5) | 24–27 | Sequential; ∥ Phase 6 UI track |
| 6 Web (109.7) | UI track 28–32 (may start with Phase 5) · App track 33–37 (after Phase 5) | UI ∥ API; App after both |
| 7 Docs + gate | 38 ∥ 39, then 40 | |

**File ownership for parallel tracks** (a parallel implementer touches only its own files):

- Phase 2 Track A: `crates/nigel/src/cli/document.rs`, `crates/nigel/src/cli/mod.rs`, `crates/nigel/src/main.rs`, `crates/nigel/tests/cli_dispatch.rs`.
- Phase 2 Track B: `crates/nigel-core/src/invoicing/{r2,gateway,mailgun,wiring,send,schedules}.rs`, `crates/nigel-core/src/settings.rs`, `crates/nigel-core/src/server/uploads.rs`, `crates/nigel-core/src/server/routes/invoices.rs` (test module), `crates/nigel-core/src/server/routes/imports.rs`, `crates/nigel/src/cli/invoice_manager.rs` (test module), `crates/nigel/tests/common/mod.rs`, `crates/nigel-core/src/documents/{mod,wire,testing}.rs`.
- Phase 4 Track A: `crates/nigel-core/src/documents/{lifecycle,sync,mod}.rs`, `crates/nigel/src/cli/{document,mod}.rs`, `crates/nigel/src/main.rs`, `crates/nigel/tests/cli_dispatch.rs`.
- Phase 4 Track B: `workers/document-response/**`, `.github/workflows/ci.yml`.
- Phase 5 vs Phase 6 UI track: Phase 5 owns `crates/**`; the UI track owns `web/packages/ui/**`.
- Phase 7: Task 38 owns `docs/api.md`, `docs/architecture.md`, `docs/design-constraints.md`; Task 39 owns `README.md`, `docs/commands.md`, `docs/invoicing.md`.

---

## Shared interfaces (the contract parallel implementers agree on)

These are defined in the task named and consumed verbatim by later tasks.

```rust
// documents/model.rs (Task 2)
pub enum DocumentStatus { Draft, Sent, ChangesRequested, Accepted, Declined, Executed, Withdrawn }
pub enum RecipientRole { Signer, Collaborator }      // wire: "signer" | "collaborator"
pub enum SignatureRole { Client, Countersign }       // "client" | "countersign"
pub enum Method { Online, Manual }                   // "online" | "manual"
pub struct NewRecipient { pub role: RecipientRole, pub name: String, pub email: String }
pub enum ResponseKind<'a> { Accept { typed_name: &'a str }, RequestChanges { note: &'a str } }

// invoicing/gateway.rs (Tasks 9–11)
pub trait DocumentPublisher {
    fn publish_pdf(&self, token: &str, version: i64, pdf: &[u8]) -> Result<String>;
    fn publish_page(&self, token: &str, recipient_token: &str, html: &[u8]) -> Result<String>;
    fn public_base(&self) -> &str;
}
pub struct Attachment<'a> { pub filename: &'a str, pub content_type: &'a str, pub bytes: &'a [u8] }
pub struct OutgoingMail<'a> { pub to: &'a str, pub cc: &'a [String], pub subject: &'a str, pub text: &'a str, pub attachment: Option<Attachment<'a>> }
pub trait Mailer { fn send(&self, mail: &OutgoingMail<'_>) -> Result<()>; fn send_invoice(/* unchanged signature, provided */) -> Result<()>; }
pub trait ResponseSource {
    fn fetch(&self, token: &str, version: i64, recipient_token: &str) -> Result<Option<DocumentResponse>>;
    fn put_manifest(&self, token: &str, manifest: &Manifest) -> Result<()>;
}
```

```ts
// web/apps/app/src/api/types.ts (Task 33) — mirrors the Rust DTOs in Task 24
export type DocumentStatus = 'draft' | 'sent' | 'changes_requested' | 'accepted' | 'declined' | 'executed' | 'withdrawn';
// full set in Task 33
```

---

# Phase 1 — Data layer (TASK-109.1)

### Task 1: Migration v15 — tables, indexes, one-time kind seed

**Files:**
- Modify: `crates/nigel-core/src/migrations.rs`

**Interfaces:**
- Produces: tables `document_kinds`, `documents`, `document_versions`, `document_recipients`, `document_signatures`, `document_change_requests`; indexes `idx_document_signatures_online`, `idx_document_change_requests_online`, `idx_documents_client`; metadata key `document_kinds_seeded`.

- [ ] **Step 1: Write the failing tests** (in `migrations.rs`'s `mod tests`)

```rust
fn table_exists(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |r| r.get(0),
    )
    .unwrap()
}

fn kind_names(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT name FROM document_kinds ORDER BY position, id")
        .unwrap();
    stmt.query_map([], |r| r.get(0))
        .unwrap()
        .collect::<std::result::Result<_, _>>()
        .unwrap()
}

const DOCUMENT_TABLES: [&str; 6] = [
    "document_kinds",
    "documents",
    "document_versions",
    "document_recipients",
    "document_signatures",
    "document_change_requests",
];

#[test]
fn v15_creates_the_document_tables_and_seeds_three_kinds() {
    let (_dir, conn) = test_db();
    for table in DOCUMENT_TABLES {
        assert!(table_exists(&conn, table), "{table}");
    }
    assert_eq!(kind_names(&conn), ["Proposal", "Estimate", "Agreement"]);
}

#[test]
fn v15_never_reseeds_a_kind_the_operator_renamed_or_removed() {
    let (_dir, conn) = test_db();
    conn.execute_batch(
        "UPDATE document_kinds SET name = 'Quote' WHERE name = 'Proposal';
         DELETE FROM document_kinds WHERE name = 'Estimate';",
    )
    .unwrap();
    set_metadata(&conn, "schema_version", "14").unwrap();
    run_migrations(&conn).unwrap();
    assert_eq!(kind_names(&conn), ["Quote", "Agreement"]);
}

#[test]
fn v15_migrates_an_existing_v14_database_and_leaves_invoices_alone() {
    let (_dir, conn) = test_db();
    conn.execute_batch(
        "INSERT INTO clients (name) VALUES ('Cedar Systems');
         INSERT INTO invoices (number, client_id, issue_date, token)
             VALUES (1248, 1, '2026-01-01', 'tok-1248');
         DROP TABLE document_change_requests;
         DROP TABLE document_signatures;
         DROP TABLE document_recipients;
         DROP TABLE document_versions;
         DROP TABLE documents;
         DROP TABLE document_kinds;
         DELETE FROM metadata WHERE key = 'document_kinds_seeded';",
    )
    .unwrap();
    set_metadata(&conn, "schema_version", "14").unwrap();
    run_migrations(&conn).unwrap();
    assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
    for table in DOCUMENT_TABLES {
        assert!(table_exists(&conn, table), "{table}");
    }
    assert_eq!(kind_names(&conn), ["Proposal", "Estimate", "Agreement"]);
    let invoices: i64 = conn
        .query_row("SELECT COUNT(*) FROM invoices", [], |r| r.get(0))
        .unwrap();
    assert_eq!(invoices, 1);
}

#[test]
fn v15_is_replayable() {
    let (_dir, conn) = test_db();
    set_metadata(&conn, "schema_version", "14").unwrap();
    run_migrations(&conn).unwrap();
    run_migrations(&conn).unwrap();
    assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
    assert_eq!(kind_names(&conn).len(), 3);
}

#[test]
fn v15_refuses_a_second_online_signature_for_one_recipient_and_version() {
    let (_dir, conn) = test_db();
    conn.execute_batch(
        "INSERT INTO clients (name) VALUES ('Juniper Labs');
         INSERT INTO documents (client_id, kind_id, title, token, created_at, updated_at)
             VALUES (1, 1, 'Statement of work', 'a', '2026-10-01', '2026-10-01');
         INSERT INTO document_versions (document_id, number, file_path, checksum, sent_at, created_at)
             VALUES (1, 1, 'documents/1/v1.pdf', 'sha256:00', '2026-10-01', '2026-10-01');
         INSERT INTO document_recipients (version_id, role, name, email, token, position)
             VALUES (1, 'signer', 'Pat Example', 'pat@juniper.test', 'r', 0);
         INSERT INTO document_signatures
             (version_id, recipient_id, role, name, method, signed_at, typed_name, checksum)
             VALUES (1, 1, 'client', 'Pat Example', 'online', '2026-10-02', 'Pat Example', 'sha256:00');",
    )
    .unwrap();
    let again = conn.execute_batch(
        "INSERT INTO document_signatures
             (version_id, recipient_id, role, name, method, signed_at, typed_name, checksum)
             VALUES (1, 1, 'client', 'Pat Example', 'online', '2026-10-03', 'Pat Example', 'sha256:00');",
    );
    assert!(again.is_err());
    conn.execute_batch(
        "INSERT INTO document_signatures (version_id, role, name, method, signed_at, checksum)
             VALUES (1, 'countersign', 'Sam Example', 'manual', '2026-10-04', 'sha256:00');",
    )
    .unwrap();
}

#[test]
fn v15_ties_online_to_a_recipient_and_manual_to_none() {
    let (_dir, conn) = test_db();
    conn.execute_batch(
        "INSERT INTO clients (name) VALUES ('Juniper Labs');
         INSERT INTO documents (client_id, kind_id, title, token, created_at, updated_at)
             VALUES (1, 1, 'T', 'a', '2026-10-01', '2026-10-01');
         INSERT INTO document_versions (document_id, number, file_path, checksum, created_at)
             VALUES (1, 1, 'documents/1/v1.pdf', 'sha256:00', '2026-10-01');",
    )
    .unwrap();
    assert!(conn
        .execute_batch(
            "INSERT INTO document_change_requests
                 (version_id, name, method, requested_at, note, checksum)
                 VALUES (1, 'Sam Example', 'online', '2026-10-02', 'x', 'sha256:00');",
        )
        .is_err());
}
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p nigel-core migrations::tests::v15 -- --test-threads=1`
Expected: FAIL — `no such table: document_kinds` / assertion on `table_exists`.

- [ ] **Step 3: Implement** — append to `MIGRATIONS`:

```rust
Migration {
    version: 15,
    description: "documents: kinds, documents, versions, recipients, signatures and change requests",
    up: |conn| {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS document_kinds (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE COLLATE NOCASE,
                active INTEGER NOT NULL DEFAULT 1,
                position INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS documents (
                id INTEGER PRIMARY KEY,
                client_id INTEGER NOT NULL,
                kind_id INTEGER NOT NULL,
                title TEXT NOT NULL,
                token TEXT NOT NULL UNIQUE,
                declined_at TEXT,
                decline_note TEXT,
                withdrawn_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (client_id) REFERENCES clients(id),
                FOREIGN KEY (kind_id) REFERENCES document_kinds(id)
             );
             CREATE INDEX IF NOT EXISTS idx_documents_client ON documents(client_id);
             CREATE TABLE IF NOT EXISTS document_versions (
                id INTEGER PRIMARY KEY,
                document_id INTEGER NOT NULL,
                number INTEGER NOT NULL CHECK (number >= 1),
                file_path TEXT NOT NULL,
                checksum TEXT NOT NULL,
                sent_at TEXT,
                created_at TEXT NOT NULL,
                UNIQUE (document_id, number),
                FOREIGN KEY (document_id) REFERENCES documents(id)
             );
             CREATE TABLE IF NOT EXISTS document_recipients (
                id INTEGER PRIMARY KEY,
                version_id INTEGER NOT NULL,
                role TEXT NOT NULL CHECK (role IN ('signer', 'collaborator')),
                name TEXT NOT NULL,
                email TEXT NOT NULL,
                token TEXT NOT NULL UNIQUE,
                position INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (version_id) REFERENCES document_versions(id)
             );
             CREATE TABLE IF NOT EXISTS document_signatures (
                id INTEGER PRIMARY KEY,
                version_id INTEGER NOT NULL,
                recipient_id INTEGER,
                role TEXT NOT NULL CHECK (role IN ('client', 'countersign')),
                name TEXT NOT NULL,
                email TEXT,
                method TEXT NOT NULL CHECK (method IN ('online', 'manual')),
                signed_at TEXT NOT NULL,
                typed_name TEXT,
                ip TEXT,
                user_agent TEXT,
                checksum TEXT NOT NULL,
                CHECK ((method = 'online') = (recipient_id IS NOT NULL)),
                FOREIGN KEY (version_id) REFERENCES document_versions(id),
                FOREIGN KEY (recipient_id) REFERENCES document_recipients(id)
             );
             CREATE UNIQUE INDEX IF NOT EXISTS idx_document_signatures_online
                 ON document_signatures(version_id, recipient_id)
                 WHERE recipient_id IS NOT NULL;
             CREATE TABLE IF NOT EXISTS document_change_requests (
                id INTEGER PRIMARY KEY,
                version_id INTEGER NOT NULL,
                recipient_id INTEGER,
                name TEXT NOT NULL,
                email TEXT,
                method TEXT NOT NULL CHECK (method IN ('online', 'manual')),
                requested_at TEXT NOT NULL,
                note TEXT NOT NULL,
                ip TEXT,
                user_agent TEXT,
                checksum TEXT NOT NULL,
                CHECK ((method = 'online') = (recipient_id IS NOT NULL)),
                FOREIGN KEY (version_id) REFERENCES document_versions(id),
                FOREIGN KEY (recipient_id) REFERENCES document_recipients(id)
             );
             CREATE UNIQUE INDEX IF NOT EXISTS idx_document_change_requests_online
                 ON document_change_requests(version_id, recipient_id)
                 WHERE recipient_id IS NOT NULL;",
        )?;
        // Seeded once per database, never again: a replay must not bring back a
        // kind the operator renamed or removed.
        if crate::db::get_metadata(conn, "document_kinds_seeded").is_none() {
            conn.execute_batch(
                "INSERT OR IGNORE INTO document_kinds (name, position) VALUES
                     ('Proposal', 0), ('Estimate', 1), ('Agreement', 2);",
            )?;
            set_metadata(conn, "document_kinds_seeded", "1")?;
        }
        Ok(())
    },
},
```

- [ ] **Step 4: Run** `cargo test -p nigel-core migrations -- --test-threads=1` — Expected: PASS (including `migration_versions_are_contiguous_from_one`).

- [ ] **Step 5: Commit**

```bash
git add crates/nigel-core/src/migrations.rs
git commit -m "Documents schema: migration v15 with kinds seeded once (TASK-109.1)"
```

---

### Task 2: Model types and kind rows

**Files:**
- Create: `crates/nigel-core/src/documents/mod.rs`, `crates/nigel-core/src/documents/model.rs`, `crates/nigel-core/src/documents/kinds.rs`, `crates/nigel-core/src/documents/testing.rs`
- Modify: `crates/nigel-core/src/lib.rs` (`pub mod documents;`)

**Interfaces — Produces:**

```rust
// documents/mod.rs
//! Documents: a filed PDF, sent to one signer and any collaborators, answered
//! online or recorded by hand, revised and countersigned. Recorded assent, not
//! a legal e-signature: a typed name, an explicit consent, a time, an IP and a
//! user agent, bound to a SHA-256 checksum of the exact PDF.
pub mod kinds;
pub mod model;
#[cfg(any(test, feature = "testutil"))]
pub mod testing;
```
(Tasks 3–6 add `pub mod status;`, `guards`, `store` and `record` as each file lands.)

```rust
// documents/model.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentStatus { Draft, Sent, ChangesRequested, Accepted, Declined, Executed, Withdrawn }
impl DocumentStatus {
    pub const ALL: [DocumentStatus; 7];
    pub fn as_str(self) -> &'static str;           // "draft" … "changes_requested" … "withdrawn"
    pub fn parse(value: &str) -> Option<Self>;
    pub fn is_terminal(self) -> bool;              // Withdrawn | Executed | Declined
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)] #[serde(rename_all = "snake_case")]
pub enum RecipientRole { Signer, Collaborator }    // as_str / parse
#[derive(...same...)] pub enum SignatureRole { Client, Countersign }
#[derive(...same...)] pub enum Method { Online, Manual }

#[derive(Debug, Clone, PartialEq, Eq, Serialize)] #[serde(rename_all = "camelCase")]
pub struct DocumentKind { pub id: i64, pub name: String, pub active: bool, pub position: i64 }

#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: i64, pub client_id: i64, pub kind_id: i64, pub kind: String, pub title: String,
    #[serde(skip_serializing)] pub token: String,
    pub declined_at: Option<String>, pub decline_note: Option<String>, pub withdrawn_at: Option<String>,
    pub created_at: String, pub updated_at: String,
    pub status: DocumentStatus,
}
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct DocumentVersion {
    pub id: i64, pub document_id: i64, pub number: i64,
    #[serde(skip_serializing)] pub file_path: String,
    pub checksum: String, pub sent_at: Option<String>, pub created_at: String,
}
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct Recipient {
    pub id: i64, pub version_id: i64, pub role: RecipientRole, pub name: String, pub email: String,
    #[serde(skip_serializing)] pub token: String, pub position: i64,
}
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct Signature {
    pub id: i64, pub version_id: i64, pub recipient_id: Option<i64>, pub role: SignatureRole,
    pub name: String, pub email: Option<String>, pub method: Method, pub signed_at: String,
    pub typed_name: Option<String>, pub ip: Option<String>, pub user_agent: Option<String>, pub checksum: String,
}
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct ChangeRequest {
    pub id: i64, pub version_id: i64, pub recipient_id: Option<i64>, pub name: String, pub email: Option<String>,
    pub method: Method, pub requested_at: String, pub note: String,
    pub ip: Option<String>, pub user_agent: Option<String>, pub checksum: String,
}
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct VersionRecord {
    #[serde(flatten)] pub version: DocumentVersion,
    pub recipients: Vec<Recipient>, pub signatures: Vec<Signature>, pub change_requests: Vec<ChangeRequest>,
}
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct DocumentRecord { #[serde(flatten)] pub document: Document, pub client_name: String, pub versions: Vec<VersionRecord> }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct DocumentListRow {
    pub id: i64, pub title: String, pub kind: String, pub client_id: i64, pub client_name: Option<String>,
    pub status: DocumentStatus, pub latest_version: i64, pub sent_at: Option<String>, pub updated_at: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRecipient { pub role: RecipientRole, pub name: String, pub email: String }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseKind<'a> { Accept { typed_name: &'a str }, RequestChanges { note: &'a str } }

pub const CHECKSUM_PREFIX: &str = "sha256:";
pub const NOTE_MAX_CHARS: usize = 4000;
pub fn gen_document_token() -> String;                               // 16 random bytes, hex
pub fn checksum_of(bytes: &[u8]) -> String;                          // "sha256:<hex>"
pub fn parse_recipient(raw: &str, role: RecipientRole) -> Result<NewRecipient>;
pub fn validate_note(note: &str) -> Result<String>;
pub fn validate_moment(value: &str, what: &str) -> Result<String>;   // YYYY-MM-DD or RFC 3339 → normalized
```

```rust
// documents/kinds.rs
pub fn list_kinds(conn: &Connection, include_inactive: bool) -> Result<Vec<DocumentKind>>;
pub fn add_kind(conn: &Connection, name: &str) -> Result<i64>;              // DuplicateName { kind: "Document kind", .. }
pub fn rename_kind(conn: &Connection, id: i64, name: &str) -> Result<()>;
pub fn deactivate_kind(conn: &Connection, id: i64) -> Result<()>;           // Conflict "already_inactive" when it is
pub fn active_kind_by_name(conn: &Connection, name: &str) -> Result<DocumentKind>; // NotFound / Conflict "kind_inactive"
```

- [ ] **Step 1: Failing tests** (`model.rs` `mod tests`, `kinds.rs` `mod tests`)

```rust
// model.rs
#[test]
fn tokens_are_128_bit_hex_and_distinct() {
    let a = gen_document_token();
    let b = gen_document_token();
    assert_eq!(a.len(), 32);
    assert!(a.bytes().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_ne!(a, b);
}

#[test]
fn checksums_carry_their_algorithm() {
    assert_eq!(
        checksum_of(b"abc"),
        "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn every_status_round_trips_through_its_wire_word() {
    for status in DocumentStatus::ALL {
        assert_eq!(DocumentStatus::parse(status.as_str()), Some(status));
        assert_eq!(serde_json::to_value(status).unwrap(), status.as_str());
    }
    assert_eq!(DocumentStatus::ChangesRequested.as_str(), "changes_requested");
    assert!(DocumentStatus::Executed.is_terminal());
    assert!(!DocumentStatus::Accepted.is_terminal());
}

#[test]
fn a_recipient_is_name_and_angle_bracketed_address() {
    let r = parse_recipient("Pat Example <pat@juniper.test>", RecipientRole::Signer).unwrap();
    assert_eq!((r.name.as_str(), r.email.as_str()), ("Pat Example", "pat@juniper.test"));
    let bare = parse_recipient("sam@juniper.test", RecipientRole::Collaborator).unwrap();
    assert_eq!(bare.name, "sam@juniper.test");
    assert!(parse_recipient("pat@juniper.test", RecipientRole::Signer).is_err());
    assert!(parse_recipient("Pat <pat@juniper.test>\r\nBcc: x@y.test", RecipientRole::Signer).is_err());
    assert!(parse_recipient("Pat Example <>", RecipientRole::Signer).is_err());
}

#[test]
fn a_note_is_one_to_four_thousand_characters_of_text() {
    assert!(validate_note("").is_err());
    assert!(validate_note("   \n").is_err());
    assert!(validate_note(&"é".repeat(4000)).is_ok());
    assert!(validate_note(&"é".repeat(4001)).is_err());
    assert!(validate_note("line one\nline two\ttabbed").is_ok());
    assert!(validate_note("bell\u{7}").is_err());
}

#[test]
fn a_moment_is_a_day_or_an_rfc3339_instant() {
    assert_eq!(validate_moment("2026-10-5", "accept").unwrap(), "2026-10-05");
    assert_eq!(
        validate_moment("2026-10-05T10:04:11-07:00", "response").unwrap(),
        "2026-10-05T17:04:11Z"
    );
    assert!(validate_moment("yesterday", "accept").is_err());
}
```

```rust
// kinds.rs
#[test]
fn kinds_are_rows_that_can_be_added_renamed_and_deactivated() {
    let (_d, conn) = crate::documents::testing::test_conn();
    let id = add_kind(&conn, "Statement of work").unwrap();
    rename_kind(&conn, id, "SOW").unwrap();
    deactivate_kind(&conn, id).unwrap();
    let active: Vec<String> = list_kinds(&conn, false).unwrap().into_iter().map(|k| k.name).collect();
    assert_eq!(active, ["Proposal", "Estimate", "Agreement"]);
    let all = list_kinds(&conn, true).unwrap();
    assert!(all.iter().any(|k| k.name == "SOW" && !k.active));
    assert!(matches!(
        active_kind_by_name(&conn, "sow").unwrap_err(),
        NigelError::Conflict { code: "kind_inactive", .. }
    ));
    assert!(matches!(add_kind(&conn, "proposal"), Err(NigelError::DuplicateName { .. })));
    assert!(matches!(active_kind_by_name(&conn, "Memo"), Err(NigelError::NotFound(_))));
}

#[test]
fn no_rust_enum_mirrors_the_kind_list() {
    let sources = [include_str!("model.rs"), include_str!("kinds.rs")];
    for source in sources {
        assert!(!source.contains(concat!("enum ", "DocumentKind")), "a compiled-in kind list");
        assert!(!source.contains(concat!("Proposal", " =>")), "a compiled-in kind name");
    }
}
```

`documents::testing::test_conn` is created in this task as the first content of `documents/testing.rs`:

```rust
//! Fixtures and fakes for document tests in this crate, the CLI crate and the
//! route tests. Compiled only for tests and the `testutil` feature.
pub fn test_conn() -> (tempfile::TempDir, rusqlite::Connection) {
    let dir = tempfile::tempdir().unwrap();
    let conn = crate::db::get_connection(&dir.path().join("t.db")).unwrap();
    crate::db::init_db(&conn).unwrap();
    (dir, conn)
}
```

- [ ] **Step 2: Run** `cargo test -p nigel-core documents -- --test-threads=1` — Expected: compile errors for the missing items, then FAIL.

- [ ] **Step 3: Implement.** Non-obvious parts:

```rust
pub fn gen_document_token() -> String {
    let mut bytes = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    hex::encode(bytes)
}

pub fn checksum_of(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{CHECKSUM_PREFIX}{}", hex::encode(Sha256::digest(bytes)))
}

pub fn parse_recipient(raw: &str, role: RecipientRole) -> Result<NewRecipient> {
    let raw = raw.trim();
    crate::invoicing::mailgun::validate_header_value(raw, "recipient")?;
    let (name, email) = match raw.rsplit_once('<') {
        Some((name, rest)) => {
            let email = rest.strip_suffix('>').ok_or_else(|| invalid_recipient(raw))?;
            (name.trim().to_string(), email.trim().to_string())
        }
        None => (String::new(), raw.to_string()),
    };
    if email.is_empty() || !email.contains('@') || email.contains(char::is_whitespace) {
        return Err(invalid_recipient(raw));
    }
    let name = match (name.is_empty(), role) {
        (true, RecipientRole::Signer) => {
            return Err(NigelError::Invalid(format!(
                "The signer needs a name to type when accepting: write it as \"Name <{email}>\"."
            )))
        }
        (true, RecipientRole::Collaborator) => email.clone(),
        (false, _) => name,
    };
    Ok(NewRecipient { role, name, email })
}

pub fn validate_note(note: &str) -> Result<String> {
    let count = note.chars().count();
    if note.trim().is_empty() || count > NOTE_MAX_CHARS {
        return Err(NigelError::Invalid(format!(
            "A change request note must be 1 to {NOTE_MAX_CHARS} characters, got {count}."
        )));
    }
    if note.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) {
        return Err(NigelError::Invalid("A change request note may not carry control characters.".into()));
    }
    Ok(note.to_string())
}

pub fn validate_moment(value: &str, what: &str) -> Result<String> {
    if let Ok(instant) = chrono::DateTime::parse_from_rfc3339(value.trim()) {
        return Ok(instant.with_timezone(&chrono::Utc).format("%Y-%m-%dT%H:%M:%SZ").to_string());
    }
    crate::invoicing::invoices::validate_date(value, what)
}
```

`kinds.rs`: `name` trimmed and non-empty (`Invalid` otherwise); a UNIQUE violation maps to `NigelError::DuplicateName { kind: "Document kind", name }` by probing `SELECT EXISTS(… WHERE name = ?1 COLLATE NOCASE AND id IS NOT ?2)` first (the `clients::name_taken` shape); `add_kind` appends at `COALESCE(MAX(position) + 1, 0)`; `rename_kind`/`deactivate_kind` answer `NotFound("Document kind not found: id {id}")` on zero rows changed.

- [ ] **Step 4: Run** `cargo test -p nigel-core documents -- --test-threads=1` — Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/nigel-core/src/lib.rs crates/nigel-core/src/documents
git commit -m "Documents model types and operator-defined kinds (TASK-109.1)"
```

---

### Task 3: The status function

**Files:**
- Create: `crates/nigel-core/src/documents/status.rs`
- Modify: `crates/nigel-core/src/documents/mod.rs` (`pub mod status;`), `crates/nigel-core/src/documents/kinds.rs` (test source list)

**Interfaces — Produces:**

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatusFacts {
    pub withdrawn: bool,
    pub declined: bool,
    pub countersigned: bool,
    pub latest_is_sent: bool,
    pub client_signed_latest: bool,
    pub change_requested_latest: bool,
}
pub fn status_from(facts: &StatusFacts) -> DocumentStatus;
pub fn status_facts(conn: &Connection, document_id: i64) -> Result<StatusFacts>;
pub fn document_status(conn: &Connection, document_id: i64) -> Result<DocumentStatus>;
```

**Resolved ambiguity:** the spec's `accepted` and `changes_requested` rows look at "the latest sent version". After a revise from `changes_requested` the newest version is an unsent draft while the change request still sits on the previous version; read literally the document would stay `changes_requested` and could never be sent again. The derivation therefore asks the two questions only when the newest version is itself sent; a newer unsent version reads as `draft`. This is the only reading under which the spec's flow (changes requested → revise → sent) is reachable.

- [ ] **Step 1: Failing tests**

```rust
fn facts() -> StatusFacts { StatusFacts::default() }

#[test]
fn every_row_of_the_status_table() {
    use DocumentStatus::*;
    let cases = [
        (facts(), Draft),
        (StatusFacts { latest_is_sent: true, ..facts() }, Sent),
        (StatusFacts { latest_is_sent: true, change_requested_latest: true, ..facts() }, ChangesRequested),
        (StatusFacts { latest_is_sent: true, client_signed_latest: true, ..facts() }, Accepted),
        (StatusFacts { latest_is_sent: true, client_signed_latest: true, change_requested_latest: true, ..facts() }, Accepted),
        (StatusFacts { latest_is_sent: true, declined: true, ..facts() }, Declined),
        (StatusFacts { latest_is_sent: true, client_signed_latest: true, countersigned: true, ..facts() }, Executed),
        (StatusFacts { countersigned: true, declined: true, ..facts() }, Executed),
        (StatusFacts { withdrawn: true, countersigned: true, ..facts() }, Withdrawn),
    ];
    for (input, expected) in cases {
        assert_eq!(status_from(&input), expected, "{input:?}");
    }
}

#[test]
fn a_newer_unsent_version_reads_as_draft_whatever_the_older_one_carries() {
    let input = StatusFacts { latest_is_sent: false, change_requested_latest: true, client_signed_latest: true, ..facts() };
    assert_eq!(status_from(&input), DocumentStatus::Draft);
}

#[test]
fn nothing_in_the_documents_module_writes_a_status() {
    let sources = [
        include_str!("model.rs"), include_str!("kinds.rs"), include_str!("status.rs"),
    ];
    for source in sources {
        assert!(!source.contains(concat!("SET ", "status")), "a status write");
    }
}

#[test]
fn the_documents_table_has_no_status_column() {
    let (_d, conn) = crate::documents::testing::test_conn();
    let has: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('documents') WHERE name = 'status'",
        [], |r| r.get(0)).unwrap();
    assert!(!has);
}

#[test]
fn the_facts_come_from_the_rows() {
    let (_d, conn) = crate::documents::testing::test_conn();
    conn.execute_batch(
        "INSERT INTO clients (name) VALUES ('Cedar Systems');
         INSERT INTO documents (client_id, kind_id, title, token, created_at, updated_at)
             VALUES (1, 1, 'Proposal', 'tok', '2026-10-01', '2026-10-01');
         INSERT INTO document_versions (document_id, number, file_path, checksum, sent_at, created_at)
             VALUES (1, 1, 'documents/1/v1.pdf', 'sha256:01', '2026-10-02', '2026-10-01');
         INSERT INTO document_change_requests (version_id, name, method, requested_at, note, checksum)
             VALUES (1, 'Sam Example', 'manual', '2026-10-03', 'Fix the dates', 'sha256:01');",
    ).unwrap();
    assert_eq!(document_status(&conn, 1).unwrap(), DocumentStatus::ChangesRequested);
    conn.execute_batch(
        "INSERT INTO document_versions (document_id, number, file_path, checksum, created_at)
             VALUES (1, 2, 'documents/1/v2.pdf', 'sha256:02', '2026-10-04');",
    ).unwrap();
    assert_eq!(document_status(&conn, 1).unwrap(), DocumentStatus::Draft);
}
```

Extend the `no_rust_enum_mirrors_the_kind_list` source list in `kinds.rs` with `include_str!("status.rs")`.

- [ ] **Step 2: Run** `cargo test -p nigel-core documents::status -- --test-threads=1` — Expected: FAIL (missing module).

- [ ] **Step 3: Implement**

```rust
pub fn status_from(f: &StatusFacts) -> DocumentStatus {
    use DocumentStatus::*;
    if f.withdrawn { return Withdrawn; }
    if f.countersigned { return Executed; }
    if f.declined { return Declined; }
    if !f.latest_is_sent { return Draft; }
    if f.client_signed_latest { return Accepted; }
    if f.change_requested_latest { return ChangesRequested; }
    Sent
}

const FACTS_SQL: &str = "
    SELECT d.withdrawn_at IS NOT NULL,
           d.declined_at IS NOT NULL,
           EXISTS(SELECT 1 FROM document_signatures s
                    JOIN document_versions v ON v.id = s.version_id
                   WHERE v.document_id = d.id AND s.role = 'countersign'),
           lv.sent_at IS NOT NULL,
           EXISTS(SELECT 1 FROM document_signatures s
                   WHERE s.version_id = lv.id AND s.role = 'client'),
           EXISTS(SELECT 1 FROM document_change_requests c WHERE c.version_id = lv.id)
      FROM documents d
      JOIN document_versions lv
        ON lv.document_id = d.id
       AND lv.number = (SELECT MAX(number) FROM document_versions WHERE document_id = d.id)
     WHERE d.id = ?1";

pub fn status_facts(conn: &Connection, document_id: i64) -> Result<StatusFacts> {
    conn.query_row(FACTS_SQL, [document_id], |r| Ok(StatusFacts {
        withdrawn: r.get(0)?, declined: r.get(1)?, countersigned: r.get(2)?,
        latest_is_sent: r.get(3)?, client_signed_latest: r.get(4)?, change_requested_latest: r.get(5)?,
    }))
    .optional()?
    .ok_or_else(|| NigelError::NotFound(format!("Document not found: id {document_id}")))
}

pub fn document_status(conn: &Connection, document_id: i64) -> Result<DocumentStatus> {
    Ok(status_from(&status_facts(conn, document_id)?))
}
```

- [ ] **Step 4: Run** — Expected: PASS.

- [ ] **Step 5: Commit** `git commit -am "Derive document status from the rows, never write it (TASK-109.1)"` (after `git add crates/nigel-core/src/documents/status.rs`).

---

### Task 4: Guards, archived clients, and the client delete block

**Files:**
- Create: `crates/nigel-core/src/documents/guards.rs`
- Modify: `crates/nigel-core/src/documents/mod.rs`, `crates/nigel-core/src/invoicing/clients.rs`, `crates/nigel-core/src/error.rs`

**Interfaces — Produces:**

```rust
// guards.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action { Edit, Send, Revise, Accept, RequestChanges, Decline, Countersign, Withdraw }
impl Action {
    pub const ALL: [Action; 8];
    pub fn allowed_from(self) -> &'static [DocumentStatus];
    pub fn describe(self) -> &'static str;   // "edited", "sent", "revised", "accepted", "sent back for changes", "declined", "countersigned", "withdrawn"
}
pub fn can(status: DocumentStatus, action: Action) -> bool;
pub fn ensure_allowed(id: i64, status: DocumentStatus, action: Action) -> Result<()>;
pub fn ensure_client_active_for_documents(conn: &Connection, client_id: i64) -> Result<()>;

// invoicing/clients.rs
pub fn ensure_client_active_for(conn: &Connection, id: i64, purpose: &str) -> Result<()>;
// ensure_client_active(conn, id) == ensure_client_active_for(conn, id, "invoicing")

// error.rs
BlockReason::HasDocuments(i64)            // reason_code "has_documents", count Some(n)
DeleteBlock::documents(subject, count)    // "Cannot delete: client has 2 documents"
```

The guard table (the data layer's only copy):

| Action | `allowed_from` |
|---|---|
| Edit | Draft |
| Send | Draft |
| Revise | Sent, ChangesRequested |
| Accept | Sent, ChangesRequested |
| RequestChanges | Sent |
| Decline | Sent, ChangesRequested |
| Countersign | Accepted |
| Withdraw | Draft, Sent, ChangesRequested |

Refusal codes: terminal status → `document_terminal` ("Document #3 is executed — nothing more can be done to it."); `Accepted` and action ≠ Countersign → `document_accepted` ("Document #3 is accepted — the only step left is to countersign it."); otherwise `document_wrong_state` ("Document #3 is sent and cannot be edited; that needs it to be draft.").

- [ ] **Step 1: Failing tests**

```rust
// guards.rs
#[test]
fn the_guard_table_matches_the_spec() {
    use Action::*; use DocumentStatus::*;
    let expected: [(Action, &[DocumentStatus]); 8] = [
        (Edit, &[Draft]), (Send, &[Draft]), (Revise, &[Sent, ChangesRequested]),
        (Accept, &[Sent, ChangesRequested]), (RequestChanges, &[Sent]),
        (Decline, &[Sent, ChangesRequested]), (Countersign, &[Accepted]),
        (Withdraw, &[Draft, Sent, ChangesRequested]),
    ];
    for (action, allowed) in expected {
        for status in DocumentStatus::ALL {
            assert_eq!(can(status, action), allowed.contains(&status), "{action:?} from {status:?}");
        }
    }
}

#[test]
fn terminal_states_refuse_everything_with_one_code() {
    for status in [DocumentStatus::Withdrawn, DocumentStatus::Executed, DocumentStatus::Declined] {
        for action in Action::ALL {
            let err = ensure_allowed(3, status, action).unwrap_err();
            assert!(matches!(err, NigelError::Conflict { code: "document_terminal", .. }), "{status:?} {action:?}");
        }
    }
}

#[test]
fn accepted_admits_countersign_and_nothing_else() {
    for action in Action::ALL {
        let result = ensure_allowed(3, DocumentStatus::Accepted, action);
        match action {
            Action::Countersign => assert!(result.is_ok()),
            _ => assert!(matches!(result, Err(NigelError::Conflict { code: "document_accepted", .. }))),
        }
    }
}

#[test]
fn only_a_draft_is_editable() {
    let err = ensure_allowed(3, DocumentStatus::Sent, Action::Edit).unwrap_err();
    assert_eq!(err.to_string(), "Document #3 is sent and cannot be edited; that needs it to be draft.");
}

#[test]
fn an_archived_client_refuses_documents_with_its_own_sentence() {
    let (_d, conn) = crate::documents::testing::test_conn();
    let id = crate::invoicing::clients::add_client(&conn, "Cedar Systems", Some("ap@cedar.test"), None, None).unwrap();
    crate::invoicing::clients::archive_client(&conn, id, "2026-10-01").unwrap();
    let err = ensure_client_active_for_documents(&conn, id).unwrap_err();
    assert!(matches!(err, NigelError::Conflict { code: "client_archived", .. }));
    assert_eq!(err.to_string(), "client 'Cedar Systems' is archived — unarchive it before filing or sending documents");
}
```

```rust
// clients.rs tests
#[test]
fn a_client_with_documents_cannot_be_deleted_and_the_block_counts_them() {
    let (_d, conn) = test_conn();
    let id = seed_client(&conn);
    conn.execute_batch(&format!(
        "INSERT INTO documents (client_id, kind_id, title, token, created_at, updated_at)
             VALUES ({id}, 1, 'A', 't1', '2026-10-01', '2026-10-01'),
                    ({id}, 1, 'B', 't2', '2026-10-01', '2026-10-01');"
    )).unwrap();
    let block = delete_blocker(&conn, id).unwrap().unwrap();
    assert_eq!(block.reason_code(), "has_documents");
    assert_eq!(block.count(), Some(2));
    assert_eq!(block.to_string(), "Cannot delete: client has 2 documents");
}
```

Add `(DeleteBlock::documents("client", 1), "Cannot delete: client has 1 document")` and `(…, 2) → "…2 documents"` to `delete_blocks_read_exactly_as_they_always_have`, `has_documents` to `block_reasons_have_stable_wire_codes`, and `Some(2)` to `only_the_counting_reasons_carry_a_count`.

- [ ] **Step 2: Run** `cargo test -p nigel-core -- --test-threads=1 guards clients error` — Expected: FAIL.

- [ ] **Step 3: Implement.** `ensure_client_active_for` carries the existing body with `"unarchive it before {purpose}"`; `ensure_client_active` delegates with `"invoicing"` so its sentence is unchanged. `delete_blocker` returns the invoice block first, then `SELECT COUNT(*) FROM documents WHERE client_id = ?1` → `DeleteBlock::documents("client", n)`. `ensure_client_active_for_documents` is `ensure_client_active_for(conn, client_id, "filing or sending documents")`. In `ensure_allowed`, build the "needs it to be" list from `allowed_from()` joined with " or ".

- [ ] **Step 4: Run** `cargo test -p nigel-core -- --test-threads=1` — Expected: PASS (existing `ensure_client_active_refuses_an_archived_client_by_name` unchanged).

- [ ] **Step 5: Commit**

```bash
git add crates/nigel-core/src/documents crates/nigel-core/src/invoicing/clients.rs crates/nigel-core/src/error.rs
git commit -m "Document guards in the data layer; client delete counts documents (TASK-109.1)"
```

---

### Task 5: Filing, storage, reads and the list

**Files:**
- Create: `crates/nigel-core/src/documents/store.rs`
- Modify: `crates/nigel-core/src/documents/mod.rs`, `crates/nigel-core/src/documents/testing.rs`

**Interfaces — Produces:**

```rust
pub const MIN_PDF_BYTES: usize = 8;
pub fn ensure_pdf(bytes: &[u8]) -> Result<()>;                 // Invalid("This file is not a PDF …")
pub fn version_file_rel(document_id: i64, number: i64) -> String; // "documents/{id}/v{n}.pdf"
pub fn write_version_file(data_dir: &Path, rel: &str, pdf: &[u8]) -> Result<()>; // 0700 dirs, 0600 file
pub fn read_version_pdf(data_dir: &Path, version: &DocumentVersion) -> Result<Vec<u8>>; // Conflict "file_changed"

pub struct NewDocument<'a> { pub client_id: i64, pub kind: &'a str, pub title: &'a str }
pub fn file_document(conn: &Connection, data_dir: &Path, new: &NewDocument<'_>, pdf: &[u8], today: &str) -> Result<i64>;
pub fn get_document(conn: &Connection, id: i64) -> Result<Document>;        // status filled by status::document_status
pub fn versions(conn: &Connection, document_id: i64) -> Result<Vec<DocumentVersion>>; // ascending
pub fn latest_version(conn: &Connection, document_id: i64) -> Result<DocumentVersion>;
pub fn latest_sent_version(conn: &Connection, document_id: i64) -> Result<Option<DocumentVersion>>;
pub fn recipients(conn: &Connection, version_id: i64) -> Result<Vec<Recipient>>;   // by position, id
pub fn signatures(conn: &Connection, version_id: i64) -> Result<Vec<Signature>>;
pub fn change_requests(conn: &Connection, version_id: i64) -> Result<Vec<ChangeRequest>>;
pub fn document_record(conn: &Connection, id: i64) -> Result<DocumentRecord>;
#[derive(Debug, Default, Clone)]
pub struct DocumentFilter { pub client_id: Option<i64>, pub status: Option<DocumentStatus>, pub kind: Option<String> }
pub fn list_documents(conn: &Connection, filter: &DocumentFilter) -> Result<Vec<DocumentListRow>>; // newest updated first
#[derive(Debug, Default, Clone)]
pub struct DocumentUpdate { pub title: Option<String>, pub kind: Option<String> }
pub fn update_document(conn: &Connection, id: i64, update: &DocumentUpdate, today: &str) -> Result<()>;

// testing.rs additions
pub fn fixture_pdf(seed: &str) -> Vec<u8>;    // b"%PDF-1.4\n% fixture {seed}\n%%EOF\n"
pub fn seed_client(conn: &Connection, name: &str) -> i64; // address pat@{first word of name, lowercased}.test; billing contact named "Pat Example" through set_contacts
pub fn seed_document(conn: &Connection, data_dir: &Path, client_id: i64, title: &str) -> i64; // fixture_pdf(&format!("{client_id}:{title}")), kind "Proposal"
```

Rules: `file_document` checks `ensure_client_active_for_documents`, `active_kind_by_name`, title trimmed 1–200 chars, `ensure_pdf`, then refuses a checksum already filed for that client on any version of any document with `Conflict { code: "duplicate_document", message: "This PDF is already filed for Cedar Systems as document #4 (\"Proposal\")." }`. One transaction inserts the document (`token = gen_document_token()`, `created_at = updated_at = validate_date(today)`) and version 1 (`file_path = version_file_rel(id, 1)`, `checksum = checksum_of(pdf)`); the file is written before commit and removed if the commit fails. `file_path` is stored relative to the data directory so a moved data directory keeps working.

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn ensure_pdf_refuses_content_that_only_claims_to_be_a_pdf() {
    assert!(ensure_pdf(&fixture_pdf("a")).is_ok());
    for bytes in [
        &b"<!doctype html><p>%PDF-1.7</p>"[..],
        &b"\x89PNG\r\n\x1a\n...."[..],
        &b"   %PDF-1.4\n"[..],
        &b"%PDF"[..],
        &b""[..],
    ] {
        let err = ensure_pdf(bytes).unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "{bytes:?}");
    }
}

#[test]
fn filing_creates_a_draft_with_its_file_and_checksum() {
    let (dir, conn) = test_conn();
    let client = seed_client(&conn, "Cedar Systems");
    let pdf = fixture_pdf("proposal");
    let id = file_document(&conn, dir.path(), &NewDocument { client_id: client, kind: "proposal", title: "Website rebuild" }, &pdf, "2026-10-05").unwrap();
    let record = document_record(&conn, id).unwrap();
    assert_eq!(record.document.status, DocumentStatus::Draft);
    assert_eq!(record.document.kind, "Proposal");
    assert_eq!(record.versions.len(), 1);
    let v1 = &record.versions[0].version;
    assert_eq!(v1.checksum, checksum_of(&pdf));
    assert_eq!(std::fs::read(dir.path().join(&v1.file_path)).unwrap(), pdf);
    assert!(v1.sent_at.is_none());
}

#[test]
fn the_same_pdf_for_the_same_client_is_a_conflict_naming_the_document() {
    let (dir, conn) = test_conn();
    let client = seed_client(&conn, "Cedar Systems");
    let pdf = fixture_pdf("same");
    let first = file_document(&conn, dir.path(), &NewDocument { client_id: client, kind: "Proposal", title: "Website rebuild" }, &pdf, "2026-10-05").unwrap();
    let err = file_document(&conn, dir.path(), &NewDocument { client_id: client, kind: "Proposal", title: "Again" }, &pdf, "2026-10-05").unwrap_err();
    assert!(matches!(err, NigelError::Conflict { code: "duplicate_document", .. }));
    assert!(err.to_string().contains(&format!("#{first}")));
    let other = seed_client(&conn, "Juniper Labs");
    assert!(file_document(&conn, dir.path(), &NewDocument { client_id: other, kind: "Proposal", title: "Theirs" }, &pdf, "2026-10-05").is_ok());
}

#[test]
fn an_archived_client_refuses_a_new_document() {
    let (dir, conn) = test_conn();
    let client = seed_client(&conn, "Harbor & Vale");
    crate::invoicing::clients::archive_client(&conn, client, "2026-10-01").unwrap();
    let err = file_document(&conn, dir.path(), &NewDocument { client_id: client, kind: "Agreement", title: "MSA" }, &fixture_pdf("x"), "2026-10-05").unwrap_err();
    assert!(err.to_string().contains("unarchive it before filing or sending documents"));
    assert!(!dir.path().join("documents").exists());
}

#[test]
fn the_list_filters_by_client_status_and_kind() {
    let (dir, conn) = test_conn();
    let cedar = seed_client(&conn, "Cedar Systems");
    let juniper = seed_client(&conn, "Juniper Labs");
    seed_document(&conn, dir.path(), cedar, "One");
    seed_document(&conn, dir.path(), juniper, "Two");
    let all = list_documents(&conn, &DocumentFilter::default()).unwrap();
    assert_eq!(all.len(), 2);
    let cedar_only = list_documents(&conn, &DocumentFilter { client_id: Some(cedar), ..Default::default() }).unwrap();
    assert_eq!(cedar_only.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(), ["One"]);
    let sent = list_documents(&conn, &DocumentFilter { status: Some(DocumentStatus::Sent), ..Default::default() }).unwrap();
    assert!(sent.is_empty());
}

#[test]
fn only_a_draft_takes_a_new_title_or_kind() {
    let (dir, conn) = test_conn();
    let client = seed_client(&conn, "Cedar Systems");
    let id = seed_document(&conn, dir.path(), client, "Draft title");
    update_document(&conn, id, &DocumentUpdate { title: Some("Final title".into()), kind: Some("Agreement".into()) }, "2026-10-06").unwrap();
    let doc = get_document(&conn, id).unwrap();
    assert_eq!((doc.title.as_str(), doc.kind.as_str(), doc.updated_at.as_str()), ("Final title", "Agreement", "2026-10-06"));
    conn.execute("UPDATE document_versions SET sent_at = '2026-10-07' WHERE document_id = ?1", [id]).unwrap();
    let err = update_document(&conn, id, &DocumentUpdate { title: Some("Late".into()), kind: None }, "2026-10-08").unwrap_err();
    assert!(matches!(err, NigelError::Conflict { code: "document_wrong_state", .. }));
}

#[test]
fn a_pdf_changed_on_disk_is_refused_rather_than_sent() {
    let (dir, conn) = test_conn();
    let client = seed_client(&conn, "Cedar Systems");
    let id = seed_document(&conn, dir.path(), client, "Tampered");
    let v = latest_version(&conn, id).unwrap();
    std::fs::write(dir.path().join(&v.file_path), fixture_pdf("other")).unwrap();
    assert!(matches!(read_version_pdf(dir.path(), &v), Err(NigelError::Conflict { code: "file_changed", .. })));
}
```

- [ ] **Step 2: Run** `cargo test -p nigel-core documents::store -- --test-threads=1` — Expected: FAIL.

- [ ] **Step 3: Implement.**

```rust
pub fn ensure_pdf(bytes: &[u8]) -> Result<()> {
    if bytes.len() >= MIN_PDF_BYTES && bytes.starts_with(b"%PDF-") {
        return Ok(());
    }
    Err(NigelError::Invalid(
        "This file is not a PDF: its content does not start with the %PDF- header, whatever its name says.".into(),
    ))
}
```

`get_document` selects the row joined to `document_kinds.name`, then sets `status: status::document_status(conn, id)?`; `NotFound(format!("Document not found: id {id}"))` on no row. `list_documents` builds `WHERE` from `client_id` and `kind` (`= ?n COLLATE NOCASE`), orders `updated_at DESC, id DESC`, derives each row's status through `status_facts`, and filters on status after derivation. `write_version_file` uses `settings::restrict_dir_permissions` / `restrict_file_permissions`.

- [ ] **Step 4: Run** — Expected: PASS.

- [ ] **Step 5: Commit** `git add crates/nigel-core/src/documents && git commit -m "File a PDF as a document draft; reads, list and draft edits (TASK-109.1)"`

---

### Task 6: Transitions — versions, recipients, responses, decline, countersign, withdraw

**Files:**
- Create: `crates/nigel-core/src/documents/record.rs`
- Modify: `crates/nigel-core/src/documents/mod.rs`, `crates/nigel-core/src/documents/status.rs`

**Interfaces — Produces:**

```rust
pub fn add_version(conn: &Connection, data_dir: &Path, id: i64, pdf: &[u8], today: &str) -> Result<i64>; // returns new number
pub fn freeze_recipients(conn: &Connection, version_id: i64, recipients: &[(NewRecipient, String)]) -> Result<Vec<Recipient>>;
pub fn unfreeze(conn: &Connection, version_id: i64) -> Result<()>;
pub fn mark_sent(conn: &Connection, version_id: i64, today: &str) -> Result<()>;
pub fn record_manual_accept(conn: &Connection, id: i64, name: &str, date: &str) -> Result<()>;
pub fn record_manual_change_request(conn: &Connection, id: i64, name: &str, note: &str, date: &str) -> Result<()>;
pub fn record_decline(conn: &Connection, id: i64, note: Option<&str>, date: &str) -> Result<()>;
pub fn record_countersign(conn: &Connection, id: i64, name: &str, date: &str) -> Result<()>;
pub fn record_withdrawal(conn: &Connection, id: i64, date: &str) -> Result<()>;
pub struct OnlineResponse<'a> {
    pub version_id: i64, pub recipient_id: i64, pub kind: ResponseKind<'a>,
    pub received_at: &'a str, pub ip: Option<&'a str>, pub user_agent: Option<&'a str>, pub checksum: &'a str,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordOutcome { Recorded, AlreadyRecorded }
pub fn record_online_response(conn: &Connection, response: &OnlineResponse<'_>) -> Result<RecordOutcome>;
```

Rules:
- `add_version`: guard `Revise`, archived client refuses, `ensure_pdf`, refuses a checksum equal to any earlier version of the same document (`Conflict "unchanged_revision"`), writes `v{n+1}.pdf`, bumps `updated_at`.
- `freeze_recipients`: version must be the latest and unsent (`Conflict "version_sent"`); exactly one `Signer` (`Conflict { code: "signer_count", message: "A document is sent to exactly one signer; this send names 0." }`); every name and email passes `validate_header_value`; no address twice (case-insensitive, `Invalid`); replaces any recipients already on the version; positions 0.. in the given order with the signer first.
- `unfreeze`: deletes recipients of an unsent version only.
- `mark_sent`: sets `sent_at`, bumps the document's `updated_at`.
- Manual verbs: each loads the document, calls `ensure_allowed` with its `Action`, binds to `latest_sent_version` and its checksum, writes `method = 'manual'`, `recipient_id`, `ip`, `user_agent` NULL, normalizes the date with `validate_moment`, trims names (empty → `Invalid`). Countersign writes `role = 'countersign'`. Decline writes `declined_at`, `decline_note` (`validate_note` when given). Withdraw writes `withdrawn_at`.
- `record_online_response` runs in one transaction: recipient must belong to `version_id` (`Invalid`); existing online row for `(version_id, recipient_id)` in **either** table → `AlreadyRecorded` (checked before any guard so a rerun is a no-op); `version_id` must be the document's latest sent version (`Conflict "stale_version"`); `checksum` must equal the version's (`Conflict "checksum_mismatch"`); `Accept` requires role `Signer` (`Conflict "role_not_allowed"`) and the `Accept` guard; `RequestChanges` requires the `RequestChanges` guard and `validate_note`; `received_at` through `validate_moment`. The typed name is stored in `typed_name` and the recipient's own name in `name`.

- [ ] **Step 1: Failing tests** (helpers `sent_document(conn, dir) -> (doc_id, version_id, signer_id, collaborator_id)` built from `seed_document` + `freeze_recipients` + `mark_sent`, defined in the test module)

```rust
#[test]
fn a_send_needs_exactly_one_signer() {
    let (dir, conn) = test_conn();
    let id = seed_document(&conn, dir.path(), seed_client(&conn, "Cedar Systems"), "SOW");
    let v = latest_version(&conn, id).unwrap();
    let collab = |e: &str| (NewRecipient { role: RecipientRole::Collaborator, name: e.into(), email: e.into() }, gen_document_token());
    let signer = |e: &str| (NewRecipient { role: RecipientRole::Signer, name: "Pat Example".into(), email: e.into() }, gen_document_token());
    for set in [vec![collab("sam@cedar.test")], vec![signer("a@cedar.test"), signer("b@cedar.test")]] {
        let err = freeze_recipients(&conn, v.id, &set).unwrap_err();
        assert!(matches!(err, NigelError::Conflict { code: "signer_count", .. }));
    }
    assert_eq!(freeze_recipients(&conn, v.id, &[signer("pat@cedar.test"), collab("sam@cedar.test")]).unwrap().len(), 2);
}

#[test]
fn a_second_response_for_one_recipient_and_version_is_a_no_op_across_both_tables() {
    let (dir, conn) = test_conn();
    let (id, v, signer, _) = sent_document(&conn, dir.path());
    let checksum = latest_version(&conn, id).unwrap().checksum;
    let first = OnlineResponse { version_id: v, recipient_id: signer, kind: ResponseKind::RequestChanges { note: "Fix the dates" }, received_at: "2026-10-05T17:04:11Z", ip: Some("203.0.113.7"), user_agent: Some("UA"), checksum: &checksum };
    assert_eq!(record_online_response(&conn, &first).unwrap(), RecordOutcome::Recorded);
    let accept = OnlineResponse { kind: ResponseKind::Accept { typed_name: "Pat Example" }, ..first };
    assert_eq!(record_online_response(&conn, &accept).unwrap(), RecordOutcome::AlreadyRecorded);
    assert!(signatures(&conn, v).unwrap().is_empty());
}

#[test]
fn a_response_must_carry_the_checksum_it_was_shown() {
    let (dir, conn) = test_conn();
    let (_, v, signer, _) = sent_document(&conn, dir.path());
    let r = OnlineResponse { version_id: v, recipient_id: signer, kind: ResponseKind::Accept { typed_name: "Pat Example" }, received_at: "2026-10-05T17:04:11Z", ip: None, user_agent: None, checksum: "sha256:00" };
    assert!(matches!(record_online_response(&conn, &r), Err(NigelError::Conflict { code: "checksum_mismatch", .. })));
}

#[test]
fn a_collaborator_cannot_accept() {
    let (dir, conn) = test_conn();
    let (id, v, _, collab) = sent_document(&conn, dir.path());
    let checksum = latest_version(&conn, id).unwrap().checksum;
    let r = OnlineResponse { version_id: v, recipient_id: collab, kind: ResponseKind::Accept { typed_name: "Sam Example" }, received_at: "2026-10-05T17:04:11Z", ip: None, user_agent: None, checksum: &checksum };
    assert!(matches!(record_online_response(&conn, &r), Err(NigelError::Conflict { code: "role_not_allowed", .. })));
}

#[test]
fn manual_responses_bind_to_the_latest_sent_version_and_carry_no_evidence() {
    let (dir, conn) = test_conn();
    let (id, v, _, _) = sent_document(&conn, dir.path());
    record_manual_accept(&conn, id, "Pat Example", "2026-10-06").unwrap();
    let sig = &signatures(&conn, v).unwrap()[0];
    assert_eq!((sig.method, sig.recipient_id, sig.ip.as_deref()), (Method::Manual, None, None));
    assert_eq!(sig.checksum, latest_version(&conn, id).unwrap().checksum);
    assert_eq!(get_document(&conn, id).unwrap().status, DocumentStatus::Accepted);
    record_countersign(&conn, id, "Sam Example", "2026-10-07").unwrap();
    assert_eq!(get_document(&conn, id).unwrap().status, DocumentStatus::Executed);
    assert!(matches!(record_withdrawal(&conn, id, "2026-10-08"), Err(NigelError::Conflict { code: "document_terminal", .. })));
}

#[test]
fn revise_after_a_change_request_makes_a_new_draft_version() {
    let (dir, conn) = test_conn();
    let (id, _, _, _) = sent_document(&conn, dir.path());
    record_manual_change_request(&conn, id, "Sam Example", "Fix the dates", "2026-10-06").unwrap();
    assert_eq!(get_document(&conn, id).unwrap().status, DocumentStatus::ChangesRequested);
    assert_eq!(add_version(&conn, dir.path(), id, &fixture_pdf("v2"), "2026-10-07").unwrap(), 2);
    assert_eq!(get_document(&conn, id).unwrap().status, DocumentStatus::Draft);
    assert!(matches!(add_version(&conn, dir.path(), id, &fixture_pdf("v3"), "2026-10-07"), Err(NigelError::Conflict { code: "document_wrong_state", .. })));
}

#[test]
fn a_revision_with_the_same_bytes_is_refused() {
    let (dir, conn) = test_conn();
    let (id, _, _, _) = sent_document(&conn, dir.path());
    let same = std::fs::read(dir.path().join(latest_version(&conn, id).unwrap().file_path)).unwrap();
    assert!(matches!(add_version(&conn, dir.path(), id, &same, "2026-10-07"), Err(NigelError::Conflict { code: "unchanged_revision", .. })));
}

#[test]
fn a_note_with_markup_is_stored_verbatim() {
    let (dir, conn) = test_conn();
    let (id, v, _, _) = sent_document(&conn, dir.path());
    let note = "<script>alert(1)</script> & <b>bold</b>";
    record_manual_change_request(&conn, id, "Sam Example", note, "2026-10-06").unwrap();
    assert_eq!(change_requests(&conn, v).unwrap()[0].note, note);
}

#[test]
fn request_changes_is_refused_once_changes_were_requested() {
    let (dir, conn) = test_conn();
    let (id, _, _, _) = sent_document(&conn, dir.path());
    record_manual_change_request(&conn, id, "Sam Example", "One", "2026-10-06").unwrap();
    assert!(record_manual_change_request(&conn, id, "Sam Example", "Two", "2026-10-06").is_err());
    record_decline(&conn, id, Some("Budget went elsewhere"), "2026-10-07").unwrap();
    assert_eq!(get_document(&conn, id).unwrap().status, DocumentStatus::Declined);
}
```

- [ ] **Step 2: Run** `cargo test -p nigel-core documents::record -- --test-threads=1` — Expected: FAIL.

- [ ] **Step 3: Implement** per the rules. The online transaction:

```rust
pub fn record_online_response(conn: &Connection, r: &OnlineResponse<'_>) -> Result<RecordOutcome> {
    let tx = conn.unchecked_transaction()?;
    let (document_id, role, name, email): (i64, String, String, String) = tx
        .query_row(
            "SELECT v.document_id, rc.role, rc.name, rc.email
               FROM document_recipients rc JOIN document_versions v ON v.id = rc.version_id
              WHERE rc.id = ?1 AND rc.version_id = ?2",
            rusqlite::params![r.recipient_id, r.version_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?
        .ok_or_else(|| NigelError::Invalid(format!("Recipient {} is not on version {}.", r.recipient_id, r.version_id)))?;
    let already: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM document_signatures WHERE version_id = ?1 AND recipient_id = ?2)
             OR EXISTS(SELECT 1 FROM document_change_requests WHERE version_id = ?1 AND recipient_id = ?2)",
        rusqlite::params![r.version_id, r.recipient_id], |row| row.get(0))?;
    if already { return Ok(RecordOutcome::AlreadyRecorded); }
    let latest = latest_sent_version(&tx, document_id)?;
    let version = match latest { Some(v) if v.id == r.version_id => v, _ => return Err(NigelError::Conflict {
        code: "stale_version", message: format!("This response is for a version of document #{document_id} that is no longer the latest sent."),
    })};
    if r.checksum != version.checksum { return Err(NigelError::Conflict {
        code: "checksum_mismatch", message: format!("The response was given on {} but version {} is {}.", r.checksum, version.number, version.checksum),
    })}
    let status = document_status(&tx, document_id)?;
    let at = validate_moment(r.received_at, "response")?;
    match r.kind {
        ResponseKind::Accept { typed_name } => {
            if role != RecipientRole::Signer.as_str() { return Err(NigelError::Conflict {
                code: "role_not_allowed", message: format!("{name} is a collaborator on document #{document_id} and cannot accept it."),
            })}
            ensure_allowed(document_id, status, Action::Accept)?;
            tx.execute("INSERT INTO document_signatures (version_id, recipient_id, role, name, email, method, signed_at, typed_name, ip, user_agent, checksum)
                        VALUES (?1, ?2, 'client', ?3, ?4, 'online', ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![r.version_id, r.recipient_id, name, email, at, typed_name, r.ip, r.user_agent, r.checksum])?;
        }
        ResponseKind::RequestChanges { note } => {
            ensure_allowed(document_id, status, Action::RequestChanges)?;
            let note = validate_note(note)?;
            tx.execute("INSERT INTO document_change_requests (version_id, recipient_id, name, email, method, requested_at, note, ip, user_agent, checksum)
                        VALUES (?1, ?2, ?3, ?4, 'online', ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![r.version_id, r.recipient_id, name, email, at, note, r.ip, r.user_agent, r.checksum])?;
        }
    }
    tx.execute("UPDATE documents SET updated_at = ?2 WHERE id = ?1", rusqlite::params![document_id, &at[..10]])?;
    tx.commit()?;
    Ok(RecordOutcome::Recorded)
}
```

(`latest_sent_version`, `document_status` and `validate_moment` accept `&Connection`; a `rusqlite::Transaction` derefs to it.)

Extend the `nothing_in_the_documents_module_writes_a_status` list with `include_str!("store.rs")` and `include_str!("record.rs")`.

- [ ] **Step 4: Run** `cargo test -p nigel-core -- --test-threads=1` — Expected: PASS.

- [ ] **Step 5: Commit** `git add crates/nigel-core/src/documents && git commit -m "Document transitions: versions, recipients, responses, decline, countersign, withdraw (TASK-109.1)"`

---

# Phase 2 — Track A: CLI filing (TASK-109.2) ∥ Track B: machinery

### Task 7 (Track A): `nigel document add`, `kinds`

**Files:**
- Create: `crates/nigel/src/cli/document.rs`
- Modify: `crates/nigel/src/cli/mod.rs`, `crates/nigel/src/main.rs`, `crates/nigel/tests/cli_dispatch.rs`

**Interfaces — Produces** (in `cli/mod.rs`):

```rust
/// File, send, and track documents for clients to accept.
Document { #[command(subcommand)] command: DocumentCommands },

#[derive(Subcommand)]
pub enum DocumentCommands {
    /// List, add, rename or deactivate document kinds.
    Kinds { #[command(subcommand)] command: Option<DocumentKindsCommands> },
    /// File a PDF as a new draft document.
    Add {
        #[arg(long)] client: i64,
        #[arg(long)] kind: String,
        #[arg(long)] title: String,
        #[arg(long)] file: std::path::PathBuf,
    },
}
#[derive(Subcommand)]
pub enum DocumentKindsCommands { List, Add { name: String }, Rename { id: i64, name: String }, Deactivate { id: i64 } }
```

```rust
// cli/document.rs
pub fn add(client: i64, kind: &str, title: &str, file: &Path, today: &str) -> Result<()>;
pub fn kinds(command: Option<DocumentKindsCommands>) -> Result<()>;
pub fn format_kind_list(kinds: &[DocumentKind]) -> String;
```

`add` reads the file, calls `store::file_document(&conn, &get_data_dir(), …)`, prints `Filed document #{id}: {title} ({kind}, draft, sha256:…)`.

- [ ] **Step 1: Failing tests** (`tests/cli_dispatch.rs`)

```rust
fn write_pdf(env: &TestEnv, name: &str, seed: &str) -> std::path::PathBuf {
    let path = env.home.path().join(name);
    std::fs::write(&path, format!("%PDF-1.4\n% fixture {seed}\n%%EOF\n")).unwrap();
    path
}

#[test]
fn document_add_files_a_pdf_as_a_draft() {
    let env = TestEnv::new();
    env.init_and_demo();
    env.cmd().args(["client", "add", "Cedar Systems", "--email", "pat@cedar.test"]).assert().success();
    let client = env.db().query_row("SELECT id FROM clients WHERE name = 'Cedar Systems'", [], |r| r.get::<_, i64>(0)).unwrap();
    let pdf = write_pdf(&env, "proposal.pdf", "a");
    env.cmd()
        .args(["document", "add", "--client", &client.to_string(), "--kind", "proposal", "--title", "Website rebuild", "--file", pdf.to_str().unwrap()])
        .assert().success()
        .stdout(predicate::str::contains("Filed document #1: Website rebuild (Proposal, draft"));
    assert!(env.data_dir().join("documents/1/v1.pdf").exists());
}

#[test]
fn document_add_refuses_a_non_pdf_named_pdf() {
    let env = TestEnv::new();
    env.init_and_demo();
    env.cmd().args(["client", "add", "Cedar Systems", "--email", "pat@cedar.test"]).assert().success();
    let client = env.db().query_row("SELECT id FROM clients WHERE name = 'Cedar Systems'", [], |r| r.get::<_, i64>(0)).unwrap().to_string();
    let path = env.home.path().join("fake.pdf");
    std::fs::write(&path, "<html>not a pdf</html>").unwrap();
    env.cmd()
        .args(["document", "add", "--client", &client, "--kind", "Proposal", "--title", "X", "--file", path.to_str().unwrap()])
        .assert().failure()
        .stderr(predicate::str::contains("not a PDF"));
}

#[test]
fn document_add_twice_names_the_existing_document() {
    let env = TestEnv::new();
    env.init_and_demo();
    env.cmd().args(["client", "add", "Cedar Systems", "--email", "pat@cedar.test"]).assert().success();
    let client = env.db().query_row("SELECT id FROM clients WHERE name = 'Cedar Systems'", [], |r| r.get::<_, i64>(0)).unwrap().to_string();
    let pdf = write_pdf(&env, "proposal.pdf", "twice");
    let add = |title: &str| env.cmd()
        .args(["document", "add", "--client", &client, "--kind", "Proposal", "--title", title, "--file", pdf.to_str().unwrap()])
        .assert();
    add("Website rebuild").success();
    add("Again").failure().stderr(predicate::str::contains("document #1"));
}

#[test]
fn document_kinds_add_rename_deactivate_round_trip() {
    let env = TestEnv::new();
    env.init_and_demo();
    env.cmd().args(["document", "kinds"]).assert().success()
        .stdout(predicate::str::contains("Proposal").and(predicate::str::contains("Agreement")));
    env.cmd().args(["document", "kinds", "add", "Statement of work"]).assert().success();
    env.cmd().args(["document", "kinds", "rename", "4", "SOW"]).assert().success();
    env.cmd().args(["document", "kinds", "deactivate", "4"]).assert().success();
    env.cmd().args(["document", "kinds", "list"]).assert().success()
        .stdout(predicate::str::contains("SOW").and(predicate::str::contains("inactive")));
}
```

Add a unit test in `cli/document.rs`:

```rust
#[test]
fn the_kind_list_marks_inactive_rows() {
    let kinds = vec![
        DocumentKind { id: 1, name: "Proposal".into(), active: true, position: 0 },
        DocumentKind { id: 4, name: "SOW".into(), active: false, position: 3 },
    ];
    let out = format_kind_list(&kinds);
    assert!(out.contains("Proposal") && out.contains("SOW") && out.contains("inactive"));
}
```

- [ ] **Step 2: Run** `cargo test -p nigel --test cli_dispatch document_ -- --test-threads=1` — Expected: FAIL (`unrecognized subcommand 'document'`).

- [ ] **Step 3: Implement** the enum, `pub mod document;`, the dispatch arm in `main.rs` (`Commands::Document { command } => match command { … }`), and the functions. Kinds list prints a `comfy_table` with `ID`, `Name`, `State` (`active`/`inactive`).

- [ ] **Step 4: Run** — Expected: PASS.

- [ ] **Step 5: Commit** `git add crates/nigel && git commit -m "nigel document add and kinds (TASK-109.2)"`

---

### Task 8 (Track A): `nigel document list` and `show`

**Files:**
- Modify: `crates/nigel/src/cli/document.rs`, `crates/nigel/src/cli/mod.rs`, `crates/nigel/src/main.rs`, `crates/nigel/tests/cli_dispatch.rs`

**Interfaces — Produces** (added to `DocumentCommands` in `cli/mod.rs`):

```rust
/// List documents.
List { #[arg(long)] client: Option<i64>, #[arg(long)] status: Option<String>, #[arg(long)] kind: Option<String> },
/// Show one document: every version, its recipients and every response.
Show { id: i64 },
```

```rust
// cli/document.rs
pub fn list(client: Option<i64>, status: Option<&str>, kind: Option<&str>) -> Result<()>;
pub fn show(id: i64) -> Result<()>;
pub fn format_document_list(rows: &[DocumentListRow]) -> String;
pub fn format_document_show(record: &DocumentRecord) -> String;
```

`list --status` parses through `DocumentStatus::parse`; an unknown word is `Invalid("Unknown status: x. Use one of: draft, sent, changes_requested, accepted, declined, executed, withdrawn.")`. `list --client` on a missing client is `NotFound` (via `ensure_client_exists`).

`format_document_show` layout:

```
Document #1  [changes_requested]  Proposal
Title:    Website rebuild
Client:   Cedar Systems
Filed:    2026-10-05

Version 1  sha256:…  sent 2026-10-05
  signer        Pat Example <pat@cedar.test>
  collaborator  Sam Example <sam@cedar.test>
  changes requested by Sam Example (online) at 2026-10-05T17:04:11Z, ip 203.0.113.7
    | Fix the dates
Version 2  sha256:…  draft
```

Signatures print `accepted by {name} ({method}) at {signed_at}` (+ `, typed "{typed_name}", ip {ip}` when online) and `countersigned by …`; a decline prints `Declined: {date}` and the note under `|`. Notes print line by line prefixed `    | ` — plain text, never interpreted.

- [ ] **Step 1: Failing tests** (unit tests in `cli/document.rs` building `DocumentRecord` literals with Cedar Systems / Pat Example / Sam Example; and `document_list_and_show_after_add` in `cli_dispatch.rs`):

```rust
#[test]
fn show_prints_every_version_with_its_checksum_recipients_and_responses() {
    let record = fixture_record(); // two versions; v1 sent with a signer, a collaborator and an online change request; v2 draft
    let out = format_document_show(&record);
    assert!(out.contains("Version 1  sha256:aa"));
    assert!(out.contains("signer        Pat Example <pat@cedar.test>"));
    assert!(out.contains("changes requested by Sam Example (online)"));
    assert!(out.contains("    | Fix the dates"));
    assert!(out.contains("Version 2  sha256:bb  draft"));
}

#[test]
fn a_note_prints_as_its_own_text() {
    let mut record = fixture_record();
    record.versions[0].change_requests[0].note = "<b>x</b>\nsecond line".into();
    let out = format_document_show(&record);
    assert!(out.contains("    | <b>x</b>\n    | second line"));
}

#[test]
fn the_list_shows_status_kind_client_and_version() {
    let rows = vec![DocumentListRow { id: 1, title: "Website rebuild".into(), kind: "Proposal".into(), client_id: 1, client_name: Some("Cedar Systems".into()), status: DocumentStatus::Sent, latest_version: 2, sent_at: Some("2026-10-05".into()), updated_at: "2026-10-05".into() }];
    let out = format_document_list(&rows);
    for needle in ["Website rebuild", "Proposal", "Cedar Systems", "sent", "v2"] { assert!(out.contains(needle), "{needle}"); }
}
```

Write `fixture_record()` in the test module as a full struct literal (no database).

- [ ] **Step 2: Run** `cargo test -p nigel document -- --test-threads=1` — Expected: FAIL.
- [ ] **Step 3: Implement**, adding the `List`/`Show` variants to `DocumentCommands`, their dispatch arms in `main.rs`, and the functions.
- [ ] **Step 4: Run** — Expected: PASS.
- [ ] **Step 5: Commit** `git commit -am "nigel document list and show through pure format functions (TASK-109.2)"`

---

### Task 9 (Track B): R2 key prefix, `DocumentPublisher`, and the document fakes

**Files:**
- Modify: `crates/nigel-core/src/invoicing/r2.rs`, `crates/nigel-core/src/invoicing/gateway.rs`, `crates/nigel-core/src/documents/testing.rs`

**Resolved ambiguity:** the spec says "`AssetPublisher` takes the prefix". The invoice trait's methods (`publish(token, html, pdf)`, `publish_page(token, html)`) do not fit per-recipient pages and versioned PDFs, and a dozen invoice fakes implement it. So the **key functions** take the prefix (`object_key(prefix, …)`), `R2Publisher`'s `AssetPublisher` impl passes `KeyPrefix::Invoices`, and a second trait, `DocumentPublisher`, implemented by the same `R2Publisher`, passes `KeyPrefix::Documents`. One struct, two layouts, no invoice fake changes.

**Interfaces — Produces:**

```rust
// r2.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPrefix { Invoices, Documents }
impl KeyPrefix { pub fn as_str(self) -> &'static str }   // "i" | "d"
pub fn object_key(prefix: KeyPrefix, token: &str, filename: &str) -> String;
pub const DOCUMENT_PDF_OBJECT: &str = "document.pdf";
pub fn pdf_object(prefix: KeyPrefix) -> &'static str;   // invoice.pdf | document.pdf
pub fn document_pdf_key(token: &str, version: i64) -> String;              // d/{token}/v{n}/document.pdf
pub fn document_page_key(token: &str, recipient_token: &str) -> String;    // d/{token}/{rt}/index.html
pub fn document_page_url(base: &str, token: &str, recipient_token: &str) -> String;
pub fn document_pdf_url(base: &str, token: &str, version: i64) -> String;
impl DocumentPublisher for R2Publisher { … }   // public_base_url is the documents base

// gateway.rs
pub trait DocumentPublisher {
    fn publish_pdf(&self, token: &str, version: i64, pdf: &[u8]) -> Result<String>;
    fn publish_page(&self, token: &str, recipient_token: &str, html: &[u8]) -> Result<String>;
    fn public_base(&self) -> &str;
}

// documents/testing.rs
#[derive(Default)]
pub struct FakeDocumentPublisher {
    pub objects: RefCell<Vec<(String, Vec<u8>)>>,   // key, bytes — in write order
    pub fail_when_key_contains: Option<String>,
}
impl DocumentPublisher for FakeDocumentPublisher { /* base "https://docs.example.test/d" */ }
impl FakeDocumentPublisher { pub fn page(&self, token: &str, rt: &str) -> Option<String>; pub fn keys(&self) -> Vec<String>; }
```

- [ ] **Step 1: Failing tests** (`r2.rs`)

```rust
#[test]
fn object_keys_carry_their_prefix() {
    assert_eq!(object_key(KeyPrefix::Invoices, "abc", PAGE_OBJECT), "i/abc/index.html");
    assert_eq!(object_key(KeyPrefix::Invoices, "abc", PDF_OBJECT), "i/abc/invoice.pdf");
    assert_eq!(document_pdf_key("abc", 2), "d/abc/v2/document.pdf");
    assert_eq!(document_page_key("abc", "r1"), "d/abc/r1/index.html");
}

#[test]
fn document_urls_name_the_object_not_the_directory() {
    assert_eq!(document_page_url("https://docs.example.test/d/", "abc", "r1"), "https://docs.example.test/d/abc/r1/index.html");
    assert_eq!(document_pdf_url("https://docs.example.test/d", "abc", 2), "https://docs.example.test/d/abc/v2/document.pdf");
}
```

Update `object_key_layout` to the new signature.

- [ ] **Step 2: Run** `cargo test -p nigel-core invoicing::r2 -- --test-threads=1` — Expected: FAIL (signature).
- [ ] **Step 3: Implement**; the `AssetPublisher` impl replaces `object_key(token, …)` with `object_key(KeyPrefix::Invoices, token, …)`; `logo_key` stays `i/…`.
- [ ] **Step 4: Run** `cargo test -p nigel-core -- --test-threads=1` — Expected: PASS.
- [ ] **Step 5: Commit** `git add crates/nigel-core/src && git commit -m "R2 keys take a prefix; DocumentPublisher for the d/ layout (TASK-109.3)"`

---

### Task 10 (Track B): A generic `Mailer::send` with an attachment filename

**Files:**
- Modify: `crates/nigel-core/src/invoicing/gateway.rs`, `crates/nigel-core/src/invoicing/mailgun.rs`, `crates/nigel-core/src/invoicing/send.rs` (tests), `crates/nigel-core/src/invoicing/schedules.rs` (tests), `crates/nigel-core/src/server/routes/invoices.rs` (tests), `crates/nigel/src/cli/invoice_manager.rs` (tests), `crates/nigel-core/src/documents/testing.rs`

**Interfaces — Produces:**

```rust
pub const INVOICE_ATTACHMENT: &str = "invoice.pdf";
#[derive(Debug, Clone, Copy)]
pub struct Attachment<'a> { pub filename: &'a str, pub content_type: &'a str, pub bytes: &'a [u8] }
#[derive(Debug, Clone, Copy)]
pub struct OutgoingMail<'a> { pub to: &'a str, pub cc: &'a [String], pub subject: &'a str, pub text: &'a str, pub attachment: Option<Attachment<'a>> }
pub trait Mailer {
    fn send(&self, mail: &OutgoingMail<'_>) -> Result<()>;
    fn send_invoice(&self, to: &str, cc: &[String], subject: &str, text: &str, pdf: &[u8]) -> Result<()> {
        self.send(&OutgoingMail { to, cc, subject, text, attachment: Some(Attachment { filename: INVOICE_ATTACHMENT, content_type: "application/pdf", bytes: pdf }) })
    }
}

// mailgun.rs
pub fn attachment_part(attachment: &Attachment<'_>) -> Result<reqwest::blocking::multipart::Part>;

// documents/testing.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedMail { pub to: String, pub subject: String, pub text: String, pub attachment: Option<(String, Vec<u8>)> }
#[derive(Default)]
pub struct FakeMailer { pub sent: RefCell<Vec<CapturedMail>>, pub fail_on_call: Option<usize> } // 0-based call index
impl Mailer for FakeMailer { … }
```

The five existing fakes (`send.rs` `FakeMail`/`FailMail`, `schedules.rs` `Post`, `routes/invoices.rs` `FakeMail`, `invoice_manager.rs` `FakeMail`) rename their `send_invoice` to `send` and read `mail.to`, `mail.cc`, `mail.subject`, `mail.text`, `mail.attachment.map(|a| a.bytes)` into the same captured fields. Their assertions do not change.

- [ ] **Step 1: Failing test** (`gateway.rs`)

```rust
#[test]
fn send_invoice_is_send_with_the_invoice_attachment_name() {
    use std::cell::RefCell;
    struct Capture(RefCell<Option<(String, String)>>);
    impl Mailer for Capture {
        fn send(&self, mail: &OutgoingMail<'_>) -> Result<()> {
            let a = mail.attachment.unwrap();
            *self.0.borrow_mut() = Some((a.filename.to_string(), a.content_type.to_string()));
            Ok(())
        }
    }
    let m = Capture(RefCell::new(None));
    m.send_invoice("to@example.test", &[], "s", "t", b"%PDF-").unwrap();
    assert_eq!(m.0.into_inner().unwrap(), ("invoice.pdf".into(), "application/pdf".into()));
}
```

and in `mailgun.rs`: `an_attachment_part_takes_the_given_filename` asserting `attachment_part(&Attachment { filename: "Website-rebuild-v2.pdf", … })` succeeds (the part's filename is not readable back from reqwest, so the test pins that construction succeeds for a non-ASCII-free name and fails for nothing; the filename is asserted end to end through `FakeMailer` in Task 16).

- [ ] **Step 2: Run** `cargo test -p nigel-core gateway -- --test-threads=1` — Expected: FAIL.
- [ ] **Step 3: Implement**; `MailgunClient` implements `send`, building the form from `message_fields(&self.envelope, mail.to, mail.cc, mail.subject, mail.text)` and adding `attachment_part(a)` when present.
- [ ] **Step 4: Run** `cargo test -- --test-threads=1` (root: covers `nigel` CLI fakes too) — Expected: PASS.
- [ ] **Step 5: Commit** `git add crates && git commit -m "Mailer gains a generic send with an attachment filename (TASK-109.3)"`

---

### Task 11 (Track B): Wire formats, `ResponseSource`, and the private R2 store

**Files:**
- Create: `crates/nigel-core/src/documents/wire.rs`
- Modify: `crates/nigel-core/src/documents/mod.rs`, `crates/nigel-core/src/invoicing/gateway.rs`, `crates/nigel-core/src/invoicing/r2.rs`, `crates/nigel-core/src/documents/testing.rs`

**Interfaces — Produces:**

```rust
// documents/wire.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManifestState { Open, Closed }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestRecipient { pub token: String, pub role: RecipientRole, pub name: String }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest { pub version: i64, pub checksum: String, pub state: ManifestState, pub recipients: Vec<ManifestRecipient> }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseAction { Accept, RequestChanges }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentResponse {
    pub action: ResponseAction, pub version: i64, pub checksum: String, pub recipient_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub typed_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub consent: Option<bool>,
    #[serde(default)] pub note: Option<String>,
    pub received_at: String,
    #[serde(default)] pub ip: Option<String>,
    #[serde(default)] pub user_agent: Option<String>,
}
pub fn manifest_key(token: &str) -> String;                                   // d/{token}/manifest.json
pub fn response_key(token: &str, version: i64, recipient_token: &str) -> String; // d/{token}/v{n}/{rt}.json

// invoicing/gateway.rs
pub trait ResponseSource {
    fn fetch(&self, token: &str, version: i64, recipient_token: &str) -> Result<Option<DocumentResponse>>;
    fn put_manifest(&self, token: &str, manifest: &Manifest) -> Result<()>;
}

// invoicing/r2.rs
pub struct R2PrivateStore { pub account_id: String, pub access_key: String, pub secret_key: String, pub bucket: String }
impl ResponseSource for R2PrivateStore { … }
fn object_body(status: reqwest::StatusCode, body: Vec<u8>) -> Result<Option<Vec<u8>>>; // 404 → None; 2xx → Some; else Err("r2 {status}: …")

// documents/testing.rs
#[derive(Default)]
pub struct FakeResponseSource {
    pub responses: RefCell<std::collections::HashMap<String, DocumentResponse>>, // keyed by response_key
    pub manifests: RefCell<Vec<(String, Manifest)>>,
    pub fail_fetch: bool,                 // every fetch fails
    pub fail_fetch_for: Option<String>,   // fetches for this document token fail
    pub fail_put: bool,
}
impl FakeResponseSource { pub fn put_response(&self, token: &str, version: i64, rt: &str, r: DocumentResponse); pub fn last_manifest(&self, token: &str) -> Option<Manifest>; }
```

`R2Publisher::put` and `R2PrivateStore` share two module-private functions, `put_object(account_id, access_key, secret_key, bucket, key, body, content_type)` and `get_object(…, key) -> Result<Option<Vec<u8>>>` (signed with `bucket.get_object(Some(&creds), key)`), both through `crate::invoicing::http_client()` so `no_invoicing_client_builds_its_own_unbounded_reqwest_client` holds. A body that is not valid `DocumentResponse` JSON is `Err(Other("response object {key} is not a valid response: …"))`.

- [ ] **Step 1: Failing tests**

```rust
// wire.rs
#[test]
fn the_manifest_matches_the_spec_shape() {
    let m = Manifest { version: 2, checksum: "sha256:ab".into(), state: ManifestState::Open,
        recipients: vec![ManifestRecipient { token: "t1".into(), role: RecipientRole::Signer, name: "Pat Example".into() }] };
    assert_eq!(serde_json::to_value(&m).unwrap(), serde_json::json!({
        "version": 2, "checksum": "sha256:ab", "state": "open",
        "recipients": [{ "token": "t1", "role": "signer", "name": "Pat Example" }]
    }));
}

#[test]
fn a_worker_response_parses_both_actions() {
    let accept: DocumentResponse = serde_json::from_value(serde_json::json!({
        "action": "accept", "version": 2, "checksum": "sha256:ab", "recipientToken": "t1",
        "typedName": "Pat Example", "consent": true, "note": null,
        "receivedAt": "2026-10-05T17:04:11Z", "ip": "203.0.113.7", "userAgent": "UA"
    })).unwrap();
    assert_eq!(accept.action, ResponseAction::Accept);
    let changes: DocumentResponse = serde_json::from_value(serde_json::json!({
        "action": "request_changes", "version": 2, "checksum": "sha256:ab", "recipientToken": "t2",
        "note": "Fix the dates", "receivedAt": "2026-10-05T17:04:11Z"
    })).unwrap();
    assert_eq!((changes.typed_name, changes.consent), (None, None));
}

#[test]
fn private_keys_sit_under_the_document_token() {
    assert_eq!(manifest_key("abc"), "d/abc/manifest.json");
    assert_eq!(response_key("abc", 2, "r1"), "d/abc/v2/r1.json");
}

// r2.rs
#[test]
fn a_missing_object_is_none_and_a_refusal_is_an_error() {
    assert_eq!(object_body(reqwest::StatusCode::NOT_FOUND, b"".to_vec()).unwrap(), None);
    assert_eq!(object_body(reqwest::StatusCode::OK, b"{}".to_vec()).unwrap(), Some(b"{}".to_vec()));
    assert!(object_body(reqwest::StatusCode::FORBIDDEN, b"denied".to_vec()).unwrap_err().to_string().contains("r2 403"));
}
```

- [ ] **Step 2: Run** `cargo test -p nigel-core -- --test-threads=1 wire r2` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -p nigel-core -- --test-threads=1` — Expected: PASS.
- [ ] **Step 5: Commit** `git add crates/nigel-core/src && git commit -m "Manifest and response formats; ResponseSource backed by a private R2 bucket (TASK-109.4)"`

---

### Task 12 (Track B): Settings keys and the Stripe-free client builder

**Files:**
- Modify: `crates/nigel-core/src/settings.rs`, `crates/nigel-core/src/invoicing/wiring.rs`, `crates/nigel/tests/common/mod.rs`

**Resolved ordering:** the brief lists "settings" in Phase 7; the send in Phase 3 needs `r2_private_bucket` and `document_response_url`, so the keys land here and Phase 7 documents them.

**Interfaces — Produces:**

```rust
// settings.rs — Settings gains three #[serde(default)] Option<String> fields:
pub r2_private_bucket: Option<String>,
pub documents_base_url: Option<String>,
pub document_response_url: Option<String>,

#[derive(Default)]
pub struct DocumentsConfig {
    pub invoicing: InvoicingConfig,
    pub r2_private_bucket: Option<String>,
    pub documents_base_url: Option<String>,
    pub document_response_url: Option<String>,
}
pub fn documents_config_from(s: &Settings) -> DocumentsConfig;   // NIGEL_* env wins; suppressed under TempConfigDir
pub fn documents_config() -> DocumentsConfig;
pub fn derive_documents_base(documents_base_url: Option<&str>, public_base_url: Option<&str>) -> Option<String>;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)] #[serde(rename_all = "camelCase")]
pub struct DocumentsStatus { pub send_configured: bool, pub sync_configured: bool, pub response_form: bool, pub missing: Vec<&'static str> }
pub fn documents_status(cfg: &DocumentsConfig) -> DocumentsStatus;

// wiring.rs
pub struct DocumentClients { /* private: publisher: R2Publisher, source: R2PrivateStore, mail: MailgunClient, response_url: Option<String>, warnings: Vec<String> */ }
impl DocumentClients {
    pub fn publisher(&self) -> &R2Publisher; pub fn source(&self) -> &R2PrivateStore; pub fn mail(&self) -> &MailgunClient;
    pub fn response_url(&self) -> Option<&str>; pub fn warnings(&self) -> &[String];
}
pub fn build_document_clients(cfg: DocumentsConfig, company: &str) -> Result<DocumentClients>;
pub fn optional_document_publisher(cfg: &DocumentsConfig) -> Option<R2Publisher>;
pub fn optional_response_source(cfg: &DocumentsConfig) -> Option<R2PrivateStore>;
```

Send requires, in this order: `mailgun_api_key`, `mailgun_domain`, `from_email`, `r2_account_id`, `r2_access_key`, `r2_secret_key`, `r2_bucket`, `r2_private_bucket`, and a documents base (reported missing as `documents_base_url` when neither it nor a `/i`-ending `public_base_url` gives one). `stripe_secret_key` is never required. Sync requires the four private-store keys (`r2_account_id`, `r2_access_key`, `r2_secret_key`, `r2_private_bucket`). `response_form` is `document_response_url.is_some()`. The documents base is validated with `r2::validate_public_base_url`. The mail half of `build_clients` moves into `fn build_mailer(api_key: Option<String>, domain: Option<String>, from_email: Option<String>, from_name: Option<String>, reply_to: Option<String>, company: &str) -> Result<(MailgunClient, Vec<String>)>` used by both builders, so the header-injection refusals are identical.

`tests/common/mod.rs`: `INVOICING_ENV_VARS` becomes `[&str; 15]` with the three new names.

- [ ] **Step 1: Failing tests**

```rust
// settings.rs
#[test]
fn the_documents_base_is_the_override_or_the_invoice_base_with_d() {
    assert_eq!(derive_documents_base(None, Some("https://billing.example.com/i")), Some("https://billing.example.com/d".into()));
    assert_eq!(derive_documents_base(None, Some("https://billing.example.com/i/")), Some("https://billing.example.com/d".into()));
    assert_eq!(derive_documents_base(Some("https://docs.example.com/d"), Some("https://billing.example.com/i")), Some("https://docs.example.com/d".into()));
    assert_eq!(derive_documents_base(None, Some("https://billing.example.com")), None);
    assert_eq!(derive_documents_base(None, None), None);
}

#[test]
fn documents_need_no_stripe_key_and_name_what_is_missing() {
    let cfg = DocumentsConfig { invoicing: InvoicingConfig { stripe_secret_key: None, ..fully_configured() }, r2_private_bucket: None, ..Default::default() };
    let status = documents_status(&cfg);
    assert!(!status.send_configured);
    assert_eq!(status.missing, vec!["r2_private_bucket"]);
    assert!(!status.response_form);
}

#[test]
fn the_document_keys_resolve_from_the_environment_first() {
    let file = Settings { r2_private_bucket: Some("file-private".into()), ..Settings::default() };
    let cfg = documents_config_with(&file, |name| match name {
        "NIGEL_R2_PRIVATE_BUCKET" => Some("env-private".into()),
        "NIGEL_DOCUMENT_RESPONSE_URL" => Some("https://docs.example.com/d/respond".into()),
        _ => None,
    });
    assert_eq!(cfg.r2_private_bucket.as_deref(), Some("env-private"));
    assert_eq!(cfg.document_response_url.as_deref(), Some("https://docs.example.com/d/respond"));
}

// wiring.rs
#[test]
fn the_document_builder_refuses_a_display_name_with_a_line_break() {
    let cfg = DocumentsConfig { invoicing: InvoicingConfig { from_name: Some("Cedar\r\nBcc: x@y.test".into()), ..configured_for_documents() }, r2_private_bucket: Some("private".into()), ..Default::default() };
    assert!(build_document_clients(cfg, "").is_err());
}
```

(`documents_config_with` is the env-injected private twin of `documents_config_from`, the `invoicing_config_with` shape. `configured_for_documents()` is a test helper returning every invoicing key except Stripe.)

- [ ] **Step 2: Run** `cargo test -p nigel-core -- --test-threads=1 settings wiring` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -- --test-threads=1` — Expected: PASS.
- [ ] **Step 5: Commit** `git add crates && git commit -m "Document settings keys and a client builder that needs no Stripe (TASK-109.3)"`

---

### Task 13 (Track B): Per-area upload allow-list and PDF magic

**Files:**
- Modify: `crates/nigel-core/src/server/uploads.rs`, `crates/nigel-core/src/server/routes/imports.rs`

**Interfaces — Produces:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadArea { Statement, Document }
impl UploadArea { pub fn allowed_extensions(self) -> &'static [&'static str]; } // ["csv","xlsx","xls"] | ["pdf"]
pub fn sanitize_filename_for(raw: &str, area: UploadArea) -> std::result::Result<String, String>;
pub fn sanitize_filename(raw: &str) -> std::result::Result<String, String>; // = sanitize_filename_for(raw, UploadArea::Statement)
pub fn check_content(area: UploadArea, bytes: &[u8]) -> std::result::Result<(), String>; // Document → documents::store::ensure_pdf
```

`ALLOWED_EXTENSIONS` is replaced by `UploadArea::Statement.allowed_extensions()`; `imports.rs` keeps calling `sanitize_filename`.

- [ ] **Step 1: Failing tests** (`uploads.rs`)

```rust
#[test]
fn each_area_has_its_own_extensions() {
    assert!(sanitize_filename_for("sow.pdf", UploadArea::Document).is_ok());
    assert!(sanitize_filename_for("sow.pdf", UploadArea::Statement).is_err());
    assert!(sanitize_filename_for("march.csv", UploadArea::Document).is_err());
    assert_eq!(sanitize_filename("march.CSV").unwrap(), "march.csv");
}

#[test]
fn a_document_upload_named_pdf_holding_html_is_refused_by_content() {
    assert!(check_content(UploadArea::Document, b"<html>%PDF-1.7</html>").is_err());
    assert!(check_content(UploadArea::Document, b"%PDF-1.7\n%%EOF\n").is_ok());
    assert!(check_content(UploadArea::Statement, b"date,amount\n").is_ok());
}
```

(The route-level twin, `a_document_upload_named_pdf_holding_html_is_a_400`, lands in Task 25.)

- [ ] **Step 2: Run** `cargo test -p nigel-core uploads -- --test-threads=1` — Expected: FAIL.
- [ ] **Step 3: Implement.** `unsupported_extension(name, area)` lists that area's extensions.
- [ ] **Step 4: Run** `cargo test -p nigel-core -- --test-threads=1` — Expected: PASS.
- [ ] **Step 5: Commit** `git add crates/nigel-core/src/server && git commit -m "Upload areas with their own allow-lists and a PDF content check (TASK-109.5)"`

---

# Phase 3 — Send, preview, revise, withdraw (TASK-109.3)

### Task 14: Page rendering — one seam for preview and send

**Files:**
- Create: `crates/nigel-core/src/documents/render.rs`
- Modify: `crates/nigel-core/src/documents/mod.rs`, `crates/nigel-core/src/invoicing/render_html.rs` (`fn esc` → `pub(crate) fn esc`)

**Interfaces — Produces:**

```rust
pub struct PageContext<'a> {
    pub company: &'a str, pub client_name: &'a str, pub kind: &'a str, pub title: &'a str,
    pub token: &'a str, pub version: i64, pub checksum: &'a str,
    pub pdf_href: &'a str,        // "../v{n}/document.pdf" when published or previewed to disk; the API preview passes an absolute path
}
#[derive(Debug, Clone, Copy)]
pub struct PageRecipient<'a> { pub token: &'a str, pub role: RecipientRole, pub name: &'a str }
#[derive(Debug, Clone, Copy)]
pub enum PageState<'a> {
    Open { response_url: Option<&'a str> },
    Revising,
    ChangesRequested,
    Accepted { name: &'a str, date: &'a str },
    Executed { client: (&'a str, &'a str), countersign: (&'a str, &'a str) },
    Declined,
}
pub fn relative_pdf_href(version: i64) -> String;
pub fn render_recipient_page(ctx: &PageContext<'_>, recipient: &PageRecipient<'_>, state: &PageState<'_>) -> String;
pub fn render_document_pages(ctx: &PageContext<'_>, recipients: &[PageRecipient<'_>], state: &PageState<'_>) -> Vec<(String, String)>; // (recipient token, html)
pub fn withdrawn_page_html(company: &str, title: &str) -> String;
pub fn email_subject(company: &str, title: &str, role: RecipientRole) -> String;
pub fn render_document_email_text(company: &str, ctx: &PageContext<'_>, recipient: &PageRecipient<'_>, url: &str) -> String;
pub fn attachment_name(title: &str, version: i64) -> String;
```

Page content: `<meta name="robots" content="noindex">`, the company, client, kind, title, "Version {n}", the checksum, the PDF as `<object data=… type="application/pdf">` with a download link fallback, a recorded-assent sentence ("Accepting records your typed name, the time, your IP address and browser against this exact file (checksum …). It is a record of your agreement, not a certified electronic signature."), then by state:
- `Open { response_url: Some(url) }` — for a signer: an accept form (typed name, consent checkbox, submit) and a request-changes form (textarea `maxlength="4000"`); for a collaborator: the request-changes form only. Both carry `data-*` attributes (endpoint, token, recipient token, version, checksum) and one inline script that reads them from `form.dataset`, posts JSON with `fetch`, and replaces the form with "Received: thank you" on 200 or the server's message otherwise. `<noscript>` and the no-form case both say "To respond, reply to the email this link came in."
- `Open { response_url: None }` — no form, the email sentence.
- `Revising` — "This document is being revised. A new version will be sent to you." no form.
- `ChangesRequested` — "Changes requested: a revised version is on its way." no form.
- `Accepted { name, date }` — "Accepted by {name} on {date}." no form.
- `Executed { client, countersign }` — both lines, no form.
- `Declined` — "This document was declined." no form.
Every interpolated string goes through `esc`; nothing user-supplied is ever written into the script.

Subjects: signer `"{company}: {title}: please review and sign"`, collaborator `"{company}: {title}: for your review"`; an empty company drops its `"{company}: "` prefix. `attachment_name("Website rebuild / Phase 2", 2)` is `"Website-rebuild-Phase-2-v2.pdf"` (ASCII alphanumerics kept, runs of anything else become one `-`, trimmed, at most 80 characters before `-v{n}.pdf`, `"document"` when nothing is left).

- [ ] **Step 1: Failing tests**

```rust
fn ctx() -> PageContext<'static> {
    PageContext { company: "Initech", client_name: "Cedar Systems", kind: "Proposal", title: "Website rebuild",
        token: "doc", version: 2, checksum: "sha256:ab", pdf_href: "../v2/document.pdf" }
}
const SIGNER: PageRecipient<'static> = PageRecipient { token: "rs", role: RecipientRole::Signer, name: "Pat Example" };
const COLLAB: PageRecipient<'static> = PageRecipient { token: "rc", role: RecipientRole::Collaborator, name: "Sam Example" };
const URL: Option<&str> = Some("https://docs.example.test/d/respond");

#[test]
fn a_signer_page_carries_both_forms_and_a_collaborator_page_one() {
    let signer = render_recipient_page(&ctx(), &SIGNER, &PageState::Open { response_url: URL });
    assert!(signer.contains("data-action=\"accept\"") && signer.contains("data-action=\"request_changes\""));
    let collab = render_recipient_page(&ctx(), &COLLAB, &PageState::Open { response_url: URL });
    assert!(!collab.contains("data-action=\"accept\"") && collab.contains("data-action=\"request_changes\""));
    for page in [&signer, &collab] {
        assert!(page.contains("<meta name=\"robots\" content=\"noindex\">"));
        assert!(page.contains("sha256:ab") && page.contains("../v2/document.pdf"));
    }
}

#[test]
fn without_a_response_endpoint_there_is_no_form() {
    let page = render_recipient_page(&ctx(), &SIGNER, &PageState::Open { response_url: None });
    assert!(!page.contains("<form"));
    assert!(page.contains("reply to the email this link came in"));
}

#[test]
fn closed_states_carry_no_form() {
    for state in [PageState::Revising, PageState::ChangesRequested, PageState::Declined,
                  PageState::Accepted { name: "Pat Example", date: "2026-10-06" },
                  PageState::Executed { client: ("Pat Example", "2026-10-06"), countersign: ("Sam Example", "2026-10-07") }] {
        assert!(!render_recipient_page(&ctx(), &SIGNER, &state).contains("<form"), "{state:?}");
    }
    let accepted = render_recipient_page(&ctx(), &SIGNER, &PageState::Accepted { name: "Pat Example", date: "2026-10-06" });
    assert!(accepted.contains("Accepted by Pat Example on 2026-10-06"));
}

#[test]
fn a_note_or_name_with_markup_is_escaped_on_the_page() {
    let evil = PageRecipient { name: "<img src=x onerror=alert(1)>", ..SIGNER };
    let page = render_recipient_page(&PageContext { title: "<script>x</script>", ..ctx() }, &evil, &PageState::Accepted { name: "<b>Pat</b>", date: "2026-10-06" });
    assert!(!page.contains("<img src=x") && !page.contains("<script>x") && !page.contains("<b>Pat"));
    assert!(page.contains("&lt;b&gt;Pat&lt;/b&gt;"));
}

#[test]
fn subjects_and_attachment_names() {
    assert_eq!(email_subject("Initech", "Website rebuild", RecipientRole::Signer), "Initech: Website rebuild: please review and sign");
    assert_eq!(email_subject("", "Website rebuild", RecipientRole::Collaborator), "Website rebuild: for your review");
    assert_eq!(attachment_name("Website rebuild / Phase 2", 2), "Website-rebuild-Phase-2-v2.pdf");
    assert_eq!(attachment_name("—", 1), "document-v1.pdf");
}

#[test]
fn the_email_body_is_plain_text_with_the_personal_link() {
    let text = render_document_email_text("Initech", &ctx(), &SIGNER, "https://docs.example.test/d/doc/rs/index.html");
    assert!(text.contains("https://docs.example.test/d/doc/rs/index.html"));
    assert!(!text.contains('<'));
}
```

- [ ] **Step 2: Run** `cargo test -p nigel-core documents::render -- --test-threads=1` — Expected: FAIL.
- [ ] **Step 3: Implement.** The inline script (fixed text, no interpolation):

```js
document.querySelectorAll('form[data-endpoint]').forEach(function (form) {
  form.addEventListener('submit', function (event) {
    event.preventDefault();
    var d = form.dataset, f = new FormData(form), status = form.querySelector('[data-status]');
    var body = { token: d.token, recipientToken: d.recipient, version: Number(d.version),
                 checksum: d.checksum, action: d.action };
    if (d.action === 'accept') { body.typedName = f.get('typedName'); body.consent = f.get('consent') === 'on'; }
    else { body.note = f.get('note'); }
    form.querySelectorAll('button').forEach(function (b) { b.disabled = true; });
    fetch(d.endpoint, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) })
      .then(function (r) { return r.json().catch(function () { return {}; }).then(function (j) { return { ok: r.ok, j: j }; }); })
      .then(function (res) {
        if (res.ok) { document.querySelectorAll('form[data-endpoint]').forEach(function (x) { x.hidden = true; });
                      document.getElementById('received').hidden = false; }
        else { status.textContent = (res.j && res.j.message) || 'This response could not be recorded.';
               form.querySelectorAll('button').forEach(function (b) { b.disabled = false; }); }
      })
      .catch(function () { status.textContent = 'This response could not be sent. Check your connection and try again.';
                           form.querySelectorAll('button').forEach(function (b) { b.disabled = false; }); });
  });
});
```

- [ ] **Step 4: Run** — Expected: PASS.
- [ ] **Step 5: Commit** `git add crates/nigel-core/src && git commit -m "Recipient pages, notices and email text through one render seam (TASK-109.3)"`

---

### Task 15: `nigel document preview`

**Files:**
- Create: `crates/nigel-core/src/documents/send.rs` (preview half; the send half lands in Task 16)
- Modify: `crates/nigel-core/src/documents/mod.rs`, `crates/nigel/src/cli/document.rs`, `crates/nigel/src/cli/mod.rs`, `crates/nigel/src/main.rs`, `crates/nigel/tests/cli_dispatch.rs`

**Interfaces — Produces:**

```rust
// documents/send.rs
pub struct PreviewFiles { pub dir: PathBuf, pub pages: Vec<PathBuf>, pub pdf: PathBuf }
pub fn write_preview(conn: &Connection, data_dir: &Path, id: i64, company: &str, response_url: Option<&str>, out_dir: &Path) -> Result<PreviewFiles>;
pub fn preview_recipients(conn: &Connection, client_id: i64) -> Vec<(String /*label*/, RecipientRole, String /*name*/)>;
// ("signer", Signer, billing contact name or "Signer"), ("collaborator", Collaborator, "Collaborator")

// cli/mod.rs
/// Render the recipient pages and the PDF to local files, with no network.
Preview { id: i64, #[arg(long)] output_dir: Option<String> },

// cli/mod.rs, called from main.rs
pub fn launch_sync_allowed(command: &Commands) -> bool;   // the existing skip list, plus Document Preview
```

`write_preview` renders the latest version through `render_document_pages` with `PageState::Open { response_url }` and `relative_pdf_href(n)`, writing `<out>/document-{id}/{label}/index.html` and `<out>/document-{id}/v{n}/document.pdf`. The CLI passes `documents_config().document_response_url` (read, never required) and `company_name(conn)`; output defaults to `<data_dir>/previews`. It prints each path.

- [ ] **Step 1: Failing tests**

```rust
// documents/send.rs
#[test]
fn preview_writes_pages_and_the_pdf_with_no_network_and_no_configuration() {
    let (dir, conn) = test_conn();
    let id = seed_document(&conn, dir.path(), seed_client(&conn, "Cedar Systems"), "Website rebuild");
    let out = tempfile::tempdir().unwrap();
    let files = write_preview(&conn, dir.path(), id, "", None, out.path()).unwrap();
    assert_eq!(files.pages.len(), 2);
    let signer = std::fs::read_to_string(&files.pages[0]).unwrap();
    assert!(signer.contains("../v1/document.pdf") && !signer.contains("<form"));
    assert!(files.pdf.ends_with("document-1/v1/document.pdf"));
}
```

```rust
// cli_dispatch.rs
fn filed_document(env: &TestEnv) {
    env.init_and_demo();
    env.cmd().args(["client", "add", "Cedar Systems", "--contact", "pat@cedar.test:Pat Example"]).assert().success();
    let client = env.db().query_row("SELECT id FROM clients WHERE name = 'Cedar Systems'", [], |r| r.get::<_, i64>(0)).unwrap().to_string();
    let pdf = write_pdf(env, "proposal.pdf", "filed");
    env.cmd()
        .args(["document", "add", "--client", &client, "--kind", "Proposal", "--title", "Website rebuild", "--file", pdf.to_str().unwrap()])
        .assert().success();
}

#[test]
fn document_preview_writes_files_without_any_configuration() {
    let env = TestEnv::new();
    filed_document(&env);
    env.cmd().args(["document", "preview", "1"]).assert().success()
        .stdout(predicate::str::contains("document-1/signer/index.html"));
    assert!(env.data_dir().join("previews/document-1/v1/document.pdf").exists());
    assert!(env.data_dir().join("previews/document-1/collaborator/index.html").exists());
}
```

`filed_document` is the shared helper every later dispatch test in this plan starts from.

- [ ] **Step 2: Run** `cargo test document_preview -- --test-threads=1` — Expected: FAIL.
- [ ] **Step 3: Implement**; replace the inline `matches!` in `main.rs` with `if cli::launch_sync_allowed(&command) { sync_invoice_payments(); }`. The predicate lives in `cli/mod.rs` (where a test can reach it) and is the existing list plus `Commands::Document { command: DocumentCommands::Preview { .. } }`, with a test `previews_never_reach_the_network` asserting it is `false` for `Invoice Preview`, `Invoice Template` and `Document Preview`, and `true` for `Document List`.
- [ ] **Step 4: Run** — Expected: PASS.
- [ ] **Step 5: Commit** `git add crates && git commit -m "nigel document preview, offline and unconfigured (TASK-109.3)"`

---

### Task 16: The traced send

**Files:**
- Modify: `crates/nigel-core/src/documents/send.rs`, `crates/nigel-core/src/documents/testing.rs`

**Interfaces — Produces:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentSendStep { Config, Load, Render, Freeze, Publish, Manifest, Email, Record }
impl DocumentSendStep { pub const ALL: [Self; 8]; pub fn as_str(self) -> &'static str; } // config load render freeze publish manifest email record
impl Serialize for DocumentSendStep  // via as_str
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct RecipientLink { pub role: RecipientRole, pub name: String, pub email: String, pub url: String }
#[derive(Debug, Clone)]
pub struct DocumentSendOutcome { pub version: i64, pub links: Vec<RecipientLink>, pub steps: Vec<(DocumentSendStep, StepOutcome)>, pub warnings: Vec<String> }
#[derive(Debug)]
pub struct DocumentSendFailure {
    pub step: DocumentSendStep, pub completed: Vec<DocumentSendStep>,
    pub emailed: Vec<String>,                 // addresses already mailed
    pub document_status: Option<DocumentStatus>,
    pub cleanup_warnings: Vec<String>,
    pub source: NigelError,
}
pub struct SendContext<'a> { pub data_dir: &'a Path, pub company: &'a str, pub response_url: Option<&'a str>, pub today: &'a str }
pub fn default_signer(conn: &Connection, client_id: i64) -> Result<NewRecipient>; // billing contact; Conflict "signer_name_required" when it has no name; "client_missing_email" when none
pub fn send_document_traced<P: DocumentPublisher, M: Mailer, R: ResponseSource>(
    conn: &Connection, document_id: i64, recipients: &[NewRecipient], ctx: &SendContext<'_>,
    publisher: &P, mailer: &M, source: &R,
) -> std::result::Result<DocumentSendOutcome, DocumentSendFailure>;
pub fn send_document<P, M, R>(…same…) -> Result<DocumentSendOutcome>; // .map_err(|f| f.source)
```

(`StepOutcome` is `crate::invoicing::send::StepOutcome`.)

```rust
// documents/testing.rs additions
pub fn pat() -> NewRecipient;   // Signer, "Pat Example", pat@cedar.test
pub fn sam() -> NewRecipient;   // Collaborator, "Sam Example", sam@cedar.test
```

Orchestration (each fallible call tagged with its step; `Config` belongs to the caller):
1. **Load** — `get_document`, `ensure_allowed(Send)`, `ensure_client_active_for_documents`, `latest_version` (unsent by the guard), `read_version_pdf` (refuses a changed file), client name.
2. **Render** — mint one `gen_document_token()` per recipient, build `PageRecipient`s, `render_document_pages(… PageState::Open { response_url: ctx.response_url })`.
3. **Freeze** — `freeze_recipients(version.id, …)` (exactly one signer enforced here).
4. **Publish** — `publish_pdf(token, n, pdf)`, then `publish_page` per recipient; the returned URLs become `links`.
5. **Manifest** — `put_manifest(token, Manifest { version: n, checksum, state: Open, recipients })`.
6. **Email** — per recipient, signer first: `mailer.send(OutgoingMail { to: format_address(Some(name), email), cc: &[], subject: email_subject(…), text: render_document_email_text(…), attachment: Some(Attachment { filename: &attachment_name(title, n), content_type: "application/pdf", bytes: &pdf }) })`; each success pushes the address onto `emailed`.
7. **Record** — `mark_sent(version.id, ctx.today)`.

Rollback (any failure from Freeze through Record): `unfreeze(version.id)`; if the Manifest step had completed, `put_manifest` again with `state: Closed` — a failure there adds `"Warning: the response manifest for this send could not be closed ({e}); responses to the links already emailed will be refused by Nigel but may reach the Worker."` to `cleanup_warnings`; `document_status` is re-read.

- [ ] **Step 1: Failing tests**

```rust
fn ctx(dir: &Path) -> SendContext<'_> { SendContext { data_dir: dir, company: "Initech", response_url: Some("https://docs.example.test/d/respond"), today: "2026-10-05" } }

#[test]
fn a_send_publishes_one_page_per_recipient_writes_the_manifest_and_mails_each_their_link() {
    let (dir, conn) = test_conn();
    let id = seed_document(&conn, dir.path(), seed_client(&conn, "Cedar Systems"), "Website rebuild");
    let (p, m, s) = (FakeDocumentPublisher::default(), FakeMailer::default(), FakeResponseSource::default());
    let out = send_document_traced(&conn, id, &[pat(), sam()], &ctx(dir.path()), &p, &m, &s).unwrap();
    assert_eq!(out.steps.iter().map(|(s, _)| *s).collect::<Vec<_>>(), &DocumentSendStep::ALL[1..]);
    assert_eq!(get_document(&conn, id).unwrap().status, DocumentStatus::Sent);
    let doc = get_document(&conn, id).unwrap();
    assert!(p.keys().contains(&format!("d/{}/v1/document.pdf", doc.token)));
    assert_eq!(p.keys().iter().filter(|k| k.ends_with("/index.html")).count(), 2);
    let manifest = s.last_manifest(&doc.token).unwrap();
    assert_eq!((manifest.state, manifest.recipients.len()), (ManifestState::Open, 2));
    let mails = m.sent.borrow();
    assert_eq!(mails[0].subject, "Initech: Website rebuild: please review and sign");
    assert_eq!(mails[1].subject, "Initech: Website rebuild: for your review");
    assert_eq!(mails[0].attachment.as_ref().unwrap().0, "Website-rebuild-v1.pdf");
    for (mail, link) in mails.iter().zip(&out.links) {
        assert!(mail.text.contains(&link.url) && link.url.ends_with("/index.html"));
    }
}

#[test]
fn a_mail_failure_after_the_first_recipient_rolls_back_and_closes_the_manifest() {
    let (dir, conn) = test_conn();
    let id = seed_document(&conn, dir.path(), seed_client(&conn, "Cedar Systems"), "Website rebuild");
    let (p, s) = (FakeDocumentPublisher::default(), FakeResponseSource::default());
    let m = FakeMailer { fail_on_call: Some(1), ..Default::default() };
    let failure = send_document_traced(&conn, id, &[pat(), sam()], &ctx(dir.path()), &p, &m, &s).unwrap_err();
    assert_eq!(failure.step, DocumentSendStep::Email);
    assert_eq!(failure.emailed, vec!["pat@cedar.test".to_string()]);
    assert_eq!(failure.document_status, Some(DocumentStatus::Draft));
    let v = latest_version(&conn, id).unwrap();
    assert!(v.sent_at.is_none() && recipients(&conn, v.id).unwrap().is_empty());
    let token = get_document(&conn, id).unwrap().token;
    assert_eq!(s.last_manifest(&token).unwrap().state, ManifestState::Closed);
}

fn assert_rolled_back(conn: &Connection, id: i64) {
    let v = latest_version(conn, id).unwrap();
    assert!(v.sent_at.is_none());
    assert!(recipients(conn, v.id).unwrap().is_empty());
    assert_eq!(get_document(conn, id).unwrap().status, DocumentStatus::Draft);
}

#[test]
fn every_step_before_record_rolls_back_to_a_draft_with_no_recipients() {
    let (dir, conn) = test_conn();
    let id = seed_document(&conn, dir.path(), seed_client(&conn, "Cedar Systems"), "Website rebuild");
    let cases: [(FakeDocumentPublisher, FakeResponseSource, DocumentSendStep); 2] = [
        (FakeDocumentPublisher { fail_when_key_contains: Some("document.pdf".into()), ..Default::default() }, FakeResponseSource::default(), DocumentSendStep::Publish),
        (FakeDocumentPublisher::default(), FakeResponseSource { fail_put: true, ..Default::default() }, DocumentSendStep::Manifest),
    ];
    for (p, s, step) in cases {
        let m = FakeMailer::default();
        let failure = send_document_traced(&conn, id, &[pat(), sam()], &ctx(dir.path()), &p, &m, &s).unwrap_err();
        assert_eq!(failure.step, step);
        assert!(failure.emailed.is_empty() && m.sent.borrow().is_empty());
        assert_rolled_back(&conn, id);
    }
}

#[test]
fn a_retry_after_a_failure_reuses_the_document_token_with_new_recipient_tokens() {
    let (dir, conn) = test_conn();
    let id = seed_document(&conn, dir.path(), seed_client(&conn, "Cedar Systems"), "Website rebuild");
    let token = get_document(&conn, id).unwrap().token;
    let failing = FakeDocumentPublisher { fail_when_key_contains: Some("index.html".into()), ..Default::default() };
    let (m, s) = (FakeMailer::default(), FakeResponseSource::default());
    send_document_traced(&conn, id, &[pat(), sam()], &ctx(dir.path()), &failing, &m, &s).unwrap_err();
    let first_keys = failing.keys();
    let p = FakeDocumentPublisher::default();
    send_document_traced(&conn, id, &[pat(), sam()], &ctx(dir.path()), &p, &m, &s).unwrap();
    assert_eq!(get_document(&conn, id).unwrap().token, token);
    let v = latest_version(&conn, id).unwrap();
    for r in recipients(&conn, v.id).unwrap() {
        assert!(!first_keys.iter().any(|k| k.contains(&r.token)), "a recipient token was reused");
        assert!(p.page(&token, &r.token).is_some());
    }
}

#[test]
fn a_send_with_two_signers_stops_at_freeze_and_mails_nobody() {
    let (dir, conn) = test_conn();
    let id = seed_document(&conn, dir.path(), seed_client(&conn, "Cedar Systems"), "Website rebuild");
    let (p, m, s) = (FakeDocumentPublisher::default(), FakeMailer::default(), FakeResponseSource::default());
    let two = [pat(), NewRecipient { role: RecipientRole::Signer, ..sam() }];
    let failure = send_document_traced(&conn, id, &two, &ctx(dir.path()), &p, &m, &s).unwrap_err();
    assert_eq!(failure.step, DocumentSendStep::Freeze);
    assert!(matches!(failure.source, NigelError::Conflict { code: "signer_count", .. }));
    assert!(p.keys().is_empty() && m.sent.borrow().is_empty());
}

#[test]
fn the_default_signer_is_the_named_billing_contact() {
    let (_dir, conn) = test_conn();
    let client = seed_client(&conn, "Cedar Systems");
    assert_eq!(default_signer(&conn, client).unwrap(), pat());
    let unnamed = crate::invoicing::clients::add_client(&conn, "Juniper Labs", Some("ap@juniper.test"), None, None).unwrap();
    assert!(matches!(default_signer(&conn, unnamed), Err(NigelError::Conflict { code: "signer_name_required", .. })));
    let nobody = crate::invoicing::clients::add_client(&conn, "Globex", None, None, None).unwrap();
    assert!(matches!(default_signer(&conn, nobody), Err(NigelError::Conflict { code: "client_missing_email", .. })));
}
```

`pat()` and `sam()` are written once in `documents::testing` (a plain `NewRecipient` literal each); the CLI crate reaches them through the `testutil` feature. `seed_client(conn, "Cedar Systems")` gives the billing contact "Pat Example" and `pat@cedar.test`, so `pat()` matches it.

- [ ] **Step 2: Run** `cargo test -p nigel-core documents::send -- --test-threads=1` — Expected: FAIL.
- [ ] **Step 3: Implement** with a private `Trace` (the `invoicing/send.rs` shape) and `fn run(…) -> std::result::Result<DocumentSendOutcome, (DocumentSendStep, NigelError)>`.
- [ ] **Step 4: Run** — Expected: PASS.
- [ ] **Step 5: Commit** `git commit -am "Traced document send with rollback before mark-sent (TASK-109.3)"`

---

### Task 17: `nigel document send`

**Files:**
- Modify: `crates/nigel/src/cli/document.rs`, `crates/nigel/src/cli/mod.rs`, `crates/nigel/src/main.rs`, `crates/nigel/tests/cli_dispatch.rs`

**Interfaces — Produces:**

```rust
// cli/mod.rs
/// Publish a document and email the signer and any collaborators their own links.
Send {
    id: i64,
    /// "Name <email>" (default: the client's billing contact)
    #[arg(long)] signer: Option<String>,
    /// "Name <email>", repeatable
    #[arg(long = "collaborator")] collaborators: Vec<String>,
    /// Send without confirmation (required when stdin is not a TTY)
    #[arg(long)] yes: bool,
},

// cli/document.rs
pub fn send(id: i64, signer: Option<&str>, collaborators: &[String], yes: bool, today: &str) -> Result<()>;
pub(crate) fn resolve_recipients(conn: &Connection, client_id: i64, signer: Option<&str>, collaborators: &[String]) -> Result<Vec<NewRecipient>>;
pub fn format_send_summary(title: &str, client: &str, version: i64, recipients: &[NewRecipient], publish_host: Option<&str>, response_form: bool) -> String;
pub(crate) fn send_with<P: DocumentPublisher, M: Mailer, R: ResponseSource>(conn: &Connection, id: i64, recipients: &[NewRecipient], ctx: &SendContext<'_>, publisher: &P, mailer: &M, source: &R) -> Result<String>;
pub fn format_send_failure(failure: &DocumentSendFailure) -> String;
```

Flow: open the DB; load the document and run the `Send` guard and archived check first (a refusal costs nothing); resolve recipients; without `--yes` refuse on a non-TTY (`"Refusing to send document #{id} without confirmation. Pass --yes."`), else print the summary and ask `Send it? [y/N]`; check `documents_status(&documents_config())` and refuse with every missing key: `"Sending documents is not configured: missing {keys} (set each one in settings.json or the matching NIGEL_ env var)"`; then `build_document_clients(documents_config(), &company_name)`; print client warnings as `notice:`; `send_with` prints `Sent document #{id} v{n}:` then one line per link (`  signer        Pat Example <pat@cedar.test>  https://…/index.html`). A `DocumentSendFailure` prints `format_send_failure` (step, emailed addresses, cleanup warnings) to stderr and returns its source.

- [ ] **Step 1: Failing tests**

```rust
// cli/document.rs tests (fakes from nigel_core::documents::testing)
#[test]
fn send_with_prints_each_recipients_link() {
    let _config = nigel_core::settings::TempConfigDir::new();
    let (dir, conn) = test_conn();
    let id = seed_document(&conn, dir.path(), seed_client(&conn, "Cedar Systems"), "Website rebuild");
    let ctx = SendContext { data_dir: dir.path(), company: "Initech", response_url: None, today: "2026-10-05" };
    let (p, m, s) = (FakeDocumentPublisher::default(), FakeMailer::default(), FakeResponseSource::default());
    let out = send_with(&conn, id, &[pat(), sam()], &ctx, &p, &m, &s).unwrap();
    assert!(out.starts_with("Sent document #1 v1:"));
    assert_eq!(out.matches("/index.html").count(), 2);
    assert!(out.contains("signer") && out.contains("collaborator"));
}

#[test]
fn recipients_default_to_the_billing_contact_and_take_collaborators() {
    let (_dir, conn) = test_conn();
    let client = seed_client(&conn, "Cedar Systems");
    assert_eq!(resolve_recipients(&conn, client, None, &["Sam Example <sam@cedar.test>".into()]).unwrap(), vec![pat(), sam()]);
    let lee = resolve_recipients(&conn, client, Some("Lee Example <lee@cedar.test>"), &[]).unwrap();
    assert_eq!((lee.len(), lee[0].name.as_str(), lee[0].role), (1, "Lee Example", RecipientRole::Signer));
}

#[test]
fn the_summary_says_what_sending_will_do() {
    let out = format_send_summary("Website rebuild", "Cedar Systems", 1, &[pat(), sam()], Some("docs.example.test"), false);
    assert!(out.contains("Pat Example <pat@cedar.test> (signer)") && out.contains("docs.example.test"));
    assert!(out.contains("no response form"));
}
```

```rust
// cli_dispatch.rs
#[test]
fn document_send_without_yes_on_a_pipe_refuses_and_sends_nothing() {
    let env = TestEnv::new();
    filed_document(&env);
    env.cmd().args(["document", "send", "1"]).assert().failure()
        .stderr(predicate::str::contains("Pass --yes"));
    let sent: Option<String> = env.db().query_row("SELECT sent_at FROM document_versions WHERE document_id = 1", [], |r| r.get(0)).unwrap();
    assert!(sent.is_none());
}

#[test]
fn document_send_with_nothing_configured_names_every_missing_key() {
    let env = TestEnv::new();
    filed_document(&env);
    env.cmd().args(["document", "send", "1", "--yes"]).assert().failure()
        .stderr(predicate::str::contains("Sending documents is not configured: missing")
            .and(predicate::str::contains("mailgun_api_key"))
            .and(predicate::str::contains("r2_private_bucket")));
}
```

- [ ] **Step 2: Run** `cargo test document_send -- --test-threads=1` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** — Expected: PASS.
- [ ] **Step 5: Commit** `git add crates/nigel && git commit -m "nigel document send with recipients and confirmation (TASK-109.3)"`

---

### Task 18: Revise and withdraw with best-effort republish

**Files:**
- Create: `crates/nigel-core/src/documents/lifecycle.rs`
- Modify: `crates/nigel-core/src/documents/mod.rs`, `crates/nigel-core/src/documents/testing.rs`, `crates/nigel/src/cli/document.rs`, `crates/nigel/src/cli/mod.rs`, `crates/nigel/src/main.rs`, `crates/nigel/tests/cli_dispatch.rs`

**Interfaces — Produces:**

```rust
// documents/lifecycle.rs
#[derive(Debug, Clone)]
pub struct ReviseOutcome { pub version: i64, pub warnings: Vec<String> }
pub fn revise_with_republish<P: DocumentPublisher, R: ResponseSource>(
    conn: &Connection, data_dir: &Path, id: i64, pdf: &[u8], today: &str, company: &str,
    publisher: Option<&P>, source: Option<&R>,
) -> Result<ReviseOutcome>;
pub fn withdraw_with_teardown<P: DocumentPublisher, R: ResponseSource>(
    conn: &Connection, id: i64, today: &str, company: &str, publisher: Option<&P>, source: Option<&R>,
) -> Result<Vec<String>>;
pub(crate) fn close_and_republish<P: DocumentPublisher, R: ResponseSource>(
    conn: &Connection, document: &Document, version: &DocumentVersion, company: &str,
    state: &PageState<'_>, publisher: Option<&P>, source: Option<&R>,
) -> Vec<String>;

// documents/testing.rs
pub fn sent_document_with_fakes(conn: &Connection, dir: &Path) -> (i64, FakeDocumentPublisher, FakeResponseSource);

// cli/mod.rs
/// Replace the PDF with a new version: live pages show "being revised" until it is sent.
Revise { id: i64, #[arg(long)] file: std::path::PathBuf },
/// Withdraw a document. Terminal — its pages are replaced with a withdrawn notice.
Withdraw { id: i64, #[arg(long)] yes: bool },
```

`close_and_republish` rewrites the manifest for `version` with `state: Closed` (same recipients) and every recipient page of `version` with `state`. A version with no recipients was never published, so there is nothing to close or rewrite and no warning. Otherwise a missing collaborator is a warning, never an error:
- publisher `None` → `"Warning: the change is recorded, but the R2 publisher is not configured, so the published pages for version {n} still show the old state."`
- source `None` → `"Warning: r2_private_bucket is not configured, so version {n}'s response manifest is still open."`
- a failed write → `"Warning: could not republish {name}'s page ({e})."` / `"Warning: could not close the response manifest ({e})."`

Revise commits `add_version` first, then `close_and_republish(latest sent version, PageState::Revising)`. Withdraw commits `record_withdrawal` first, then for **every sent version** closes its manifest-entry (only the latest version's manifest exists; closing writes the latest) and replaces each recipient page with `withdrawn_page_html`; an unsent document has nothing to tear down and returns no warnings.

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn revise_closes_the_manifest_and_republishes_every_live_page_as_being_revised() {
    let (dir, conn) = test_conn();
    let (id, p, s) = sent_document_with_fakes(&conn, dir.path());
    record_manual_change_request(&conn, id, "Sam Example", "Fix the dates", "2026-10-06").unwrap();
    let out = revise_with_republish(&conn, dir.path(), id, &fixture_pdf("v2"), "2026-10-07", "Initech", Some(&p), Some(&s)).unwrap();
    assert_eq!((out.version, out.warnings.len()), (2, 0));
    let doc = get_document(&conn, id).unwrap();
    assert_eq!(s.last_manifest(&doc.token).unwrap().state, ManifestState::Closed);
    let v1 = &versions(&conn, id).unwrap()[0];
    for r in recipients(&conn, v1.id).unwrap() {
        let page = p.page(&doc.token, &r.token).unwrap();
        assert!(page.contains("being revised") && !page.contains("<form"));
    }
}

#[test]
fn the_next_send_after_a_revise_reuses_the_document_token_with_new_recipient_tokens() {
    let (dir, conn) = test_conn();
    let (id, p, s) = sent_document_with_fakes(&conn, dir.path());
    record_manual_change_request(&conn, id, "Sam Example", "Fix the dates", "2026-10-06").unwrap();
    revise_with_republish(&conn, dir.path(), id, &fixture_pdf("v2"), "2026-10-07", "Initech", Some(&p), Some(&s)).unwrap();
    let token = get_document(&conn, id).unwrap().token;
    let ctx = SendContext { data_dir: dir.path(), company: "Initech", response_url: None, today: "2026-10-08" };
    send_document(&conn, id, &[pat(), sam()], &ctx, &p, &FakeMailer::default(), &s).unwrap();
    let all = versions(&conn, id).unwrap();
    let old: Vec<String> = recipients(&conn, all[0].id).unwrap().into_iter().map(|r| r.token).collect();
    let new: Vec<String> = recipients(&conn, all[1].id).unwrap().into_iter().map(|r| r.token).collect();
    assert!(new.iter().all(|t| !old.contains(t)));
    assert_eq!(get_document(&conn, id).unwrap().token, token);
    assert_eq!(s.last_manifest(&token).unwrap().version, 2);
}

#[test]
fn withdraw_commits_first_and_reports_what_it_could_not_reach() {
    let (dir, conn) = test_conn();
    let (id, _, _) = sent_document_with_fakes(&conn, dir.path());
    let warnings = withdraw_with_teardown::<FakeDocumentPublisher, FakeResponseSource>(&conn, id, "2026-10-06", "Initech", None, None).unwrap();
    assert_eq!(warnings.len(), 2);
    assert_eq!(get_document(&conn, id).unwrap().status, DocumentStatus::Withdrawn);
}

#[test]
fn a_partial_republish_failure_is_a_warning_not_an_error() {
    let (dir, conn) = test_conn();
    let (id, _, s) = sent_document_with_fakes(&conn, dir.path());
    let v = latest_version(&conn, id).unwrap();
    let sam_token = recipients(&conn, v.id).unwrap().into_iter().find(|r| r.role == RecipientRole::Collaborator).unwrap().token;
    let p = FakeDocumentPublisher { fail_when_key_contains: Some(sam_token), ..Default::default() };
    let warnings = withdraw_with_teardown(&conn, id, "2026-10-06", "Initech", Some(&p), Some(&s)).unwrap();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("Sam Example"));
    assert_eq!(get_document(&conn, id).unwrap().status, DocumentStatus::Withdrawn);
}

#[test]
fn withdrawing_a_draft_touches_nothing() {
    let (dir, conn) = test_conn();
    let id = seed_document(&conn, dir.path(), seed_client(&conn, "Cedar Systems"), "Website rebuild");
    let (p, s) = (FakeDocumentPublisher::default(), FakeResponseSource::default());
    assert!(withdraw_with_teardown(&conn, id, "2026-10-06", "Initech", Some(&p), Some(&s)).unwrap().is_empty());
    assert!(p.keys().is_empty() && s.manifests.borrow().is_empty());
}
```

`sent_document_with_fakes` (in `documents::testing`, shared by Tasks 19–21 and 24–27) files a document for the first client name from the fixture cast (Cedar Systems, Juniper Labs, Harbor & Vale, Acme, Globex, Initech) that is not yet in `clients` (`seed_client` for it), titles it "Website rebuild", sends it to `pat()` and `sam()` through the fakes with company "Initech", `response_url: Some("https://docs.example.test/d/respond")` and `today: "2026-10-05"`, and returns the document id with the publisher and response source. Calling it twice in one test therefore files two documents for two clients, so neither the duplicate-name rule nor the duplicate-PDF rule is weakened. CLI tests in `cli_dispatch.rs`: `document_withdraw_without_yes_on_a_pipe_refuses`; `document_revise_of_a_draft_is_refused` (asserts `document_wrong_state`); and `document_revise_files_version_two` (marks v1 sent with `UPDATE document_versions SET sent_at = '2026-10-05' …`, runs `document revise 1 --file …`, asserts exit 0 and stdout "version 2 is a draft", with no warnings since there are no recipients and nothing was published). `revise_with` / `withdraw_with` seams in `cli/document.rs` are tested with fakes.

- [ ] **Step 2: Run** `cargo test -- --test-threads=1 lifecycle document_revise document_withdraw` — Expected: FAIL.
- [ ] **Step 3: Implement.** The CLI builds `optional_document_publisher` / `optional_response_source` from `documents_config()` (neither is required), prints `Revised document #{id}: version {n} is a draft.` / `Withdrew document #{id}.` and each warning on stderr.
- [ ] **Step 4: Run** `cargo test -- --test-threads=1` — Expected: PASS.
- [ ] **Step 5: Commit** `git add crates && git commit -m "Revise and withdraw: commit first, republish best-effort (TASK-109.3)"`

---

# Phase 4 — Track A: sync and manual verbs (TASK-109.4) ∥ Track B: the Worker

### Task 19 (Track A): Manual verbs with republish after a status change

**Files:**
- Modify: `crates/nigel-core/src/documents/lifecycle.rs`, `crates/nigel/src/cli/document.rs`, `crates/nigel/src/cli/mod.rs`, `crates/nigel/src/main.rs`, `crates/nigel/tests/cli_dispatch.rs`

**Interfaces — Produces:**

```rust
// lifecycle.rs
pub fn republish_after_change<P: DocumentPublisher, R: ResponseSource>(
    conn: &Connection, id: i64, company: &str, publisher: Option<&P>, source: Option<&R>,
) -> Vec<String>;
// status → PageState: Accepted → Accepted { client signature name, date }; ChangesRequested → ChangesRequested;
// Executed → Executed { client, countersign }; Declined → Declined; anything else → no-op.

// cli/mod.rs
Accept { id: i64, #[arg(long)] name: String, #[arg(long)] date: Option<String> },
#[command(name = "request-changes")]
RequestChanges { id: i64, #[arg(long)] name: String, #[arg(long)] note: String, #[arg(long)] date: Option<String> },
Decline { id: i64, #[arg(long)] note: Option<String>, #[arg(long)] date: Option<String> },
Countersign { id: i64, #[arg(long)] name: String, #[arg(long)] date: Option<String> },

// cli/document.rs
pub fn accept(id: i64, name: &str, date: &str) -> Result<()>;
pub fn request_changes(id: i64, name: &str, note: &str, date: &str) -> Result<()>;
pub fn decline(id: i64, note: Option<&str>, date: &str) -> Result<()>;
pub fn countersign(id: i64, name: &str, date: &str) -> Result<()>;
```

`--date` defaults to `cli::today()`. Each verb records through `record.rs`, prints `Recorded: document #{id} is {status}.`, then prints `republish_after_change` warnings. The date on stamped pages is the first ten characters of the moment.

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn accepting_republishes_every_page_stamped_and_without_a_form() {
    let (dir, conn) = test_conn();
    let (id, p, s) = sent_document_with_fakes(&conn, dir.path());
    record_manual_accept(&conn, id, "Pat Example", "2026-10-06").unwrap();
    assert!(republish_after_change(&conn, id, "Initech", Some(&p), Some(&s)).is_empty());
    let doc = get_document(&conn, id).unwrap();
    assert_eq!(s.last_manifest(&doc.token).unwrap().state, ManifestState::Closed);
    for r in recipients(&conn, latest_version(&conn, id).unwrap().id).unwrap() {
        let page = p.page(&doc.token, &r.token).unwrap();
        assert!(page.contains("Accepted by Pat Example on 2026-10-06") && !page.contains("<form"));
    }
}

#[test]
fn countersigning_stamps_both_signatures() {
    let (dir, conn) = test_conn();
    let (id, p, s) = sent_document_with_fakes(&conn, dir.path());
    record_manual_accept(&conn, id, "Pat Example", "2026-10-06").unwrap();
    record_countersign(&conn, id, "Sam Example", "2026-10-07").unwrap();
    republish_after_change(&conn, id, "Initech", Some(&p), Some(&s));
    let doc = get_document(&conn, id).unwrap();
    let signer = &recipients(&conn, latest_version(&conn, id).unwrap().id).unwrap()[0];
    let page = p.page(&doc.token, &signer.token).unwrap();
    assert!(page.contains("Pat Example") && page.contains("Sam Example") && page.contains("2026-10-07"));
}

#[test]
fn a_failed_republish_never_loses_the_signature() {
    let (dir, conn) = test_conn();
    let (id, _, s) = sent_document_with_fakes(&conn, dir.path());
    record_manual_accept(&conn, id, "Pat Example", "2026-10-06").unwrap();
    let failing = FakeDocumentPublisher { fail_when_key_contains: Some("index.html".into()), ..Default::default() };
    assert_eq!(republish_after_change(&conn, id, "Initech", Some(&failing), Some(&s)).len(), 2);
    assert_eq!(get_document(&conn, id).unwrap().status, DocumentStatus::Accepted);
    assert_eq!(signatures(&conn, latest_version(&conn, id).unwrap().id).unwrap().len(), 1);
}
```

```rust
// cli_dispatch.rs — the manual path with nothing configured
#[test]
fn manual_accept_and_countersign_carry_a_document_to_executed() {
    let env = TestEnv::new();
    filed_document(&env);
    env.db().execute_batch("UPDATE document_versions SET sent_at = '2026-10-05' WHERE document_id = 1").unwrap();
    env.cmd().args(["document", "accept", "1", "--name", "Pat Example"]).assert().success()
        .stdout(predicate::str::contains("is accepted"));
    env.cmd().args(["document", "decline", "1"]).assert().failure()
        .stderr(predicate::str::contains("only step left is to countersign"));
    env.cmd().args(["document", "countersign", "1", "--name", "Sam Example"]).assert().success()
        .stdout(predicate::str::contains("is executed"));
    env.cmd().args(["document", "withdraw", "1", "--yes"]).assert().failure()
        .stderr(predicate::str::contains("nothing more can be done"));
    env.cmd().args(["document", "show", "1"]).assert().success()
        .stdout(predicate::str::contains("accepted by Pat Example (manual)")
            .and(predicate::str::contains("countersigned by Sam Example (manual)")));
}

#[test]
fn request_changes_needs_a_note() {
    let env = TestEnv::new();
    filed_document(&env);
    env.db().execute_batch("UPDATE document_versions SET sent_at = '2026-10-05' WHERE document_id = 1").unwrap();
    env.cmd().args(["document", "request-changes", "1", "--name", "Sam Example", "--note", ""]).assert().failure()
        .stderr(predicate::str::contains("1 to 4000"));
}
```

No publish ever happened in these, so the republish step is silent (`republish_after_change` returns no warnings when the version's recipients list is empty).

- [ ] **Step 2: Run** `cargo test -- --test-threads=1 republish_after_change manual_ request_changes` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** — Expected: PASS.
- [ ] **Step 5: Commit** `git add crates && git commit -m "Manual accept, request-changes, decline and countersign with republish (TASK-109.4)"`

---

### Task 20 (Track A): Sync core

**Files:**
- Create: `crates/nigel-core/src/documents/sync.rs`
- Modify: `crates/nigel-core/src/documents/mod.rs`

**Interfaces — Produces:**

```rust
pub const DOCUMENT_BUDGET_EXHAUSTED: &str = "not checked: the sync time budget was used up before this document was reached";
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct DocumentSyncLine {
    pub document_id: i64, pub title: String,
    pub recorded: Vec<String>,   // "Pat Example accepted version 2", "Sam Example requested changes on version 1"
    pub refused: Vec<String>,    // why a response was not recorded
    pub warnings: Vec<String>,   // republish warnings
    pub status: DocumentStatus,
}
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct DocumentSyncFailure { pub document_id: i64, pub message: String }
#[derive(Debug, Clone, Default, Serialize)] #[serde(rename_all = "camelCase")]
pub struct DocumentSyncReport { pub documents_checked: u32, pub recorded: u32, pub lines: Vec<DocumentSyncLine>, pub failures: Vec<DocumentSyncFailure> }
pub fn sync_documents<R: ResponseSource, P: DocumentPublisher>(
    conn: &Connection, company: &str, source: &R, publisher: Option<&P>, deadline: Option<std::time::Instant>,
) -> Result<DocumentSyncReport>;
```

Per document whose status is `sent` or `changes_requested` (oldest `updated_at` first):
1. Latest sent version `v`; recipients without an online row on `v`; `source.fetch(token, v.number, rt)` each. A fetch error is that document's failure; the run continues.
2. Sort fetched responses by `received_at`.
3. Refuse (a `refused` line, nothing recorded) when: `response.version != v.number`, `response.checksum != v.checksum`, `response.recipient_token != rt`, `Accept` without `consent == Some(true)` or without `typed_name`, `RequestChanges` without a note; and any `Conflict`/`Invalid` from `record_online_response` (e.g. a collaborator's request arriving after the signer accepted).
4. Record through `record_online_response`; `AlreadyRecorded` is silent.
5. If the status changed, `republish_after_change(…, publisher, Some(source))` and keep its warnings.
`Err` only when every checked document failed (`invoice sync`'s rule); an empty run is `Ok` with zero counts.

- [ ] **Step 1: Failing tests**

```rust
struct Sent { token: String, version: DocumentVersion, signer: Recipient, collaborator: Recipient }

fn sent_state(conn: &Connection, id: i64) -> Sent {
    let doc = get_document(conn, id).unwrap();
    let version = latest_version(conn, id).unwrap();
    let rs = recipients(conn, version.id).unwrap();
    Sent { token: doc.token, signer: rs[0].clone(), collaborator: rs[1].clone(), version }
}

fn accept_from(s: &Sent, at: &str) -> DocumentResponse {
    DocumentResponse { action: ResponseAction::Accept, version: s.version.number, checksum: s.version.checksum.clone(),
        recipient_token: s.signer.token.clone(), typed_name: Some("Pat Example".into()), consent: Some(true), note: None,
        received_at: at.into(), ip: Some("203.0.113.7".into()), user_agent: Some("UA".into()) }
}

fn changes_from(s: &Sent, at: &str) -> DocumentResponse {
    DocumentResponse { action: ResponseAction::RequestChanges, recipient_token: s.collaborator.token.clone(),
        typed_name: None, consent: None, note: Some("Fix the dates".into()), ..accept_from(s, at) }
}

#[test]
fn an_online_accept_is_recorded_once_and_rerunning_records_nothing() {
    let (dir, conn) = test_conn();
    let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
    let s = sent_state(&conn, id);
    src.put_response(&s.token, 1, &s.signer.token, accept_from(&s, "2026-10-05T17:04:11Z"));
    let first = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
    assert_eq!((first.documents_checked, first.recorded), (1, 1));
    assert_eq!(first.lines[0].status, DocumentStatus::Accepted);
    let again = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
    assert_eq!((again.documents_checked, again.recorded), (0, 0));
    let sigs = signatures(&conn, s.version.id).unwrap();
    assert_eq!((sigs.len(), sigs[0].method, sigs[0].ip.as_deref()), (1, Method::Online, Some("203.0.113.7")));
}

#[test]
fn a_response_for_an_earlier_version_is_refused_and_records_nothing() {
    let (dir, conn) = test_conn();
    let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
    let v1 = sent_state(&conn, id);
    record_manual_change_request(&conn, id, "Sam Example", "Fix the dates", "2026-10-06").unwrap();
    revise_with_republish(&conn, dir.path(), id, &fixture_pdf("v2"), "2026-10-07", "Initech", Some(&p), Some(&src)).unwrap();
    let ctx = SendContext { data_dir: dir.path(), company: "Initech", response_url: None, today: "2026-10-08" };
    send_document(&conn, id, &[pat(), sam()], &ctx, &p, &FakeMailer::default(), &src).unwrap();
    let v2 = sent_state(&conn, id);
    src.put_response(&v1.token, 1, &v1.signer.token, accept_from(&v1, "2026-10-08T10:00:00Z"));
    src.put_response(&v2.token, 2, &v2.signer.token, DocumentResponse { recipient_token: v2.signer.token.clone(), ..accept_from(&v1, "2026-10-08T10:01:00Z") });
    let report = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
    assert_eq!(report.recorded, 0);
    assert_eq!(report.lines[0].refused.len(), 1);
    assert!(report.lines[0].refused[0].contains("version 1"));
    assert!(signatures(&conn, v2.version.id).unwrap().is_empty());
    assert_eq!(get_document(&conn, id).unwrap().status, DocumentStatus::Sent);
}

#[test]
fn a_checksum_mismatch_is_refused() {
    let (dir, conn) = test_conn();
    let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
    let s = sent_state(&conn, id);
    src.put_response(&s.token, 1, &s.signer.token, DocumentResponse { checksum: "sha256:00".into(), ..accept_from(&s, "2026-10-05T17:04:11Z") });
    let report = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
    assert_eq!((report.recorded, report.lines[0].refused.len()), (0, 1));
}

#[test]
fn two_responses_on_one_version_apply_in_received_order() {
    let (dir, conn) = test_conn();
    let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
    let s = sent_state(&conn, id);
    src.put_response(&s.token, 1, &s.collaborator.token, changes_from(&s, "2026-10-05T17:05:00Z"));
    src.put_response(&s.token, 1, &s.signer.token, accept_from(&s, "2026-10-05T17:00:00Z"));
    let report = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
    assert_eq!(report.recorded, 1);
    assert_eq!(report.lines[0].refused.len(), 1);
    assert_eq!(get_document(&conn, id).unwrap().status, DocumentStatus::Accepted);
}

#[test]
fn a_change_request_before_an_accept_records_both() {
    let (dir, conn) = test_conn();
    let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
    let s = sent_state(&conn, id);
    src.put_response(&s.token, 1, &s.collaborator.token, changes_from(&s, "2026-10-05T17:00:00Z"));
    src.put_response(&s.token, 1, &s.signer.token, accept_from(&s, "2026-10-05T17:05:00Z"));
    let report = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
    assert_eq!(report.recorded, 2);
    assert_eq!(get_document(&conn, id).unwrap().status, DocumentStatus::Accepted);
}

#[test]
fn an_accept_without_consent_is_refused() {
    let (dir, conn) = test_conn();
    let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
    let s = sent_state(&conn, id);
    src.put_response(&s.token, 1, &s.signer.token, DocumentResponse { consent: None, ..accept_from(&s, "2026-10-05T17:04:11Z") });
    let report = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
    assert_eq!((report.recorded, report.lines[0].refused.len()), (0, 1));
}

#[test]
fn a_status_change_closes_the_manifest_and_republishes() {
    let (dir, conn) = test_conn();
    let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
    let s = sent_state(&conn, id);
    src.put_response(&s.token, 1, &s.collaborator.token, changes_from(&s, "2026-10-05T17:04:11Z"));
    sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
    assert_eq!(src.last_manifest(&s.token).unwrap().state, ManifestState::Closed);
    assert!(p.page(&s.token, &s.signer.token).unwrap().contains("Changes requested"));
}

#[test]
fn one_failing_fetch_does_not_stop_the_run() {
    let (dir, conn) = test_conn();
    let (first, p, _) = sent_document_with_fakes(&conn, dir.path());
    let (second, _, _) = sent_document_with_fakes(&conn, dir.path());
    let (a, b) = (sent_state(&conn, first), sent_state(&conn, second));
    let src = FakeResponseSource { fail_fetch_for: Some(a.token.clone()), ..Default::default() };
    src.put_response(&b.token, 1, &b.signer.token, accept_from(&b, "2026-10-05T17:04:11Z"));
    let report = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
    assert_eq!((report.documents_checked, report.recorded), (2, 1));
    assert_eq!(report.failures.len(), 1);
}

#[test]
fn every_document_failing_is_an_err() {
    let (dir, conn) = test_conn();
    let (_, p, _) = sent_document_with_fakes(&conn, dir.path());
    sent_document_with_fakes(&conn, dir.path());
    let failing = FakeResponseSource { fail_fetch: true, ..Default::default() };
    assert!(sync_documents(&conn, "Initech", &failing, Some(&p), None).is_err());
}

#[test]
fn a_spent_budget_reports_the_rest_as_failures() {
    let (dir, conn) = test_conn();
    let (_, p, src) = sent_document_with_fakes(&conn, dir.path());
    let report = sync_documents(&conn, "Initech", &src, Some(&p), Some(std::time::Instant::now())).unwrap();
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].message, DOCUMENT_BUDGET_EXHAUSTED);
}
```

`sent_document_with_fakes` (Task 18) files each call's document for a client name not yet taken, and `seed_document` seeds its PDF from the client id and title, so two documents in one test collide on neither the client-name rule nor the duplicate-PDF rule. A budget-exhausted run is `Ok` even though every document is a failure: nothing failed at the far end.

- [ ] **Step 2: Run** `cargo test -p nigel-core documents::sync -- --test-threads=1` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** — Expected: PASS.
- [ ] **Step 5: Commit** `git add crates/nigel-core/src/documents && git commit -m "Sync document responses through ResponseSource, once each (TASK-109.4)"`

---

### Task 21 (Track A): `nigel document sync` and the launch sync

**Files:**
- Modify: `crates/nigel/src/cli/document.rs`, `crates/nigel/src/cli/mod.rs`, `crates/nigel/src/main.rs`, `crates/nigel/tests/cli_dispatch.rs`

**Interfaces — Produces:**

```rust
// cli/mod.rs
/// Pull online responses and record them. Run from cron beside `invoice schedule run`.
Sync,
// launch_sync_allowed also returns false for Document Sync.

// cli/document.rs
pub fn sync() -> Result<()>;
pub fn format_sync_report(report: &DocumentSyncReport) -> String;
pub(crate) fn sync_with<R: ResponseSource, P: DocumentPublisher>(conn: &Connection, company: &str, source: &R, publisher: Option<&P>) -> Result<String>;

// main.rs
fn sync_document_responses();   // best-effort; does nothing without optional_response_source; notices on stderr
```

`sync` requires the private store (`build`-style refusal naming `r2_account_id`, `r2_access_key`, `r2_secret_key`, `r2_private_bucket` when missing). Report format: one line per document — `#3 Website rebuild: Pat Example accepted version 2 → accepted` — refusals as `  refused: …`, warnings as `  warning: …`, failures as `notice: document sync failed for #3: …` on stderr, then `Recorded {n} new response(s)`. The launch sync runs after `sync_invoice_payments()` under the same `launch_sync_allowed` predicate.

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn the_sync_report_prints_one_line_per_document() {
    let report = DocumentSyncReport { documents_checked: 2, recorded: 1,
        lines: vec![DocumentSyncLine { document_id: 3, title: "Website rebuild".into(), recorded: vec!["Pat Example accepted version 2".into()],
            refused: vec!["a response for version 1 does not match version 2".into()], warnings: vec![], status: DocumentStatus::Accepted }],
        failures: vec![DocumentSyncFailure { document_id: 4, message: "r2 403: denied".into() }] };
    let out = format_sync_report(&report);
    assert!(out.contains("#3 Website rebuild: Pat Example accepted version 2 → accepted"));
    assert!(out.contains("  refused: a response for version 1"));
    assert!(out.contains("Recorded 1 new response(s)"));
}

#[test]
fn sync_with_records_an_online_accept() {
    let (dir, conn) = test_conn();
    let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
    let doc = get_document(&conn, id).unwrap();
    let v = latest_version(&conn, id).unwrap();
    let signer = recipients(&conn, v.id).unwrap().remove(0);
    src.put_response(&doc.token, 1, &signer.token, DocumentResponse { action: ResponseAction::Accept, version: 1, checksum: v.checksum.clone(),
        recipient_token: signer.token.clone(), typed_name: Some("Pat Example".into()), consent: Some(true), note: None,
        received_at: "2026-10-05T17:04:11Z".into(), ip: None, user_agent: None });
    assert!(sync_with(&conn, "Initech", &src, Some(&p)).unwrap().contains("→ accepted"));
}

#[test]
fn launch_sync_skips_document_sync_and_both_previews() {
    use crate::cli::{Commands, DocumentCommands, InvoiceCommands};
    assert!(!crate::cli::launch_sync_allowed(&Commands::Document { command: DocumentCommands::Sync }));
    assert!(!crate::cli::launch_sync_allowed(&Commands::Document { command: DocumentCommands::Preview { id: 1, output_dir: None } }));
    assert!(!crate::cli::launch_sync_allowed(&Commands::Invoice { command: InvoiceCommands::Preview { number: 1, output_dir: None } }));
    assert!(crate::cli::launch_sync_allowed(&Commands::Document { command: DocumentCommands::Show { id: 1 } }));
}
```

```rust
// cli_dispatch.rs
#[test]
fn document_sync_with_nothing_configured_names_the_private_store_keys() {
    let env = TestEnv::new();
    env.init_and_demo();
    env.cmd().args(["document", "sync"]).assert().failure()
        .stderr(predicate::str::contains("r2_private_bucket"));
}

#[test]
fn a_document_command_runs_with_no_document_configuration() {
    let env = TestEnv::new();
    filed_document(&env);
    env.cmd().args(["document", "list"]).assert().success()
        .stderr(predicate::str::contains("document sync").not());
}
```

- [ ] **Step 2: Run** `cargo test -- --test-threads=1 document_sync launch_sync` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -- --test-threads=1` — Expected: PASS.
- [ ] **Step 5: Commit** `git add crates/nigel && git commit -m "nigel document sync, and document responses in the launch sync (TASK-109.4)"`

---

### Task 22 (Track B): Worker scaffold and the validation rules

**Files:**
- Create: `workers/document-response/{package.json,tsconfig.json,vitest.config.ts,.gitignore,wrangler.toml}`, `workers/document-response/src/env.ts`, `workers/document-response/src/validate.ts`, `workers/document-response/test/validate.test.ts`

**Interfaces — Produces:**

```ts
// src/env.ts
export interface StoredObject { json<T>(): Promise<T> }
export interface PutOptions { onlyIf?: { etagDoesNotMatch?: string }; httpMetadata?: { contentType?: string } }
export interface PrivateBucket {
  get(key: string): Promise<StoredObject | null>;
  put(key: string, value: string, options?: PutOptions): Promise<unknown | null>;
}
export interface Limiter { limit(options: { key: string }): Promise<{ success: boolean }> }
export interface Env { PRIVATE: PrivateBucket; RATE_LIMITER: Limiter }

// src/validate.ts
export type Role = 'signer' | 'collaborator';
export type Action = 'accept' | 'request_changes';
export interface Manifest { version: number; checksum: string; state: 'open' | 'closed'; recipients: { token: string; role: Role; name: string }[] }
export interface ResponseRequest { token: string; recipientToken: string; version: number; checksum: string; action: Action; typedName?: string; consent?: boolean; note?: string }
export interface Refusal { status: 400 | 403 | 404 | 405 | 409 | 422 | 429; code: string; message: string }
export const NOTE_MAX = 4000;
export function normalizeName(name: string): string;
export function parseRequest(body: unknown): ResponseRequest | Refusal;
export function checkRequest(request: ResponseRequest, manifest: Manifest | null): Refusal | null;
export function isRefusal(value: unknown): value is Refusal;
```

`package.json`:

```json
{
  "name": "nigel-document-response",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "description": "The Worker that records responses to documents sent from Nigel",
  "scripts": {
    "test": "vitest run",
    "typecheck": "tsc --noEmit",
    "deploy": "npx wrangler deploy"
  },
  "devDependencies": {
    "typescript": "^5.7.2",
    "vitest": "^2.1.8"
  }
}
```

`tsconfig.json`: `"target": "ES2022", "module": "ESNext", "moduleResolution": "Bundler", "strict": true, "lib": ["ES2022", "DOM"], "types": [], "noEmit": true, "include": ["src", "test"]`. `vitest.config.ts`: `export default defineConfig({ test: { environment: 'node', include: ['test/**/*.test.ts'] } })`. `.gitignore`: `node_modules/`, `.wrangler/`.

`wrangler.toml`:

```toml
name = "nigel-document-response"
main = "src/index.ts"
compatibility_date = "2026-09-01"

# Replace with the documents hostname that serves the public bucket.
routes = [{ pattern = "docs.example.com/d/respond", zone_name = "example.com" }]

[[r2_buckets]]
binding = "PRIVATE"
bucket_name = "nigel-documents-private"

[[ratelimits]]
name = "RATE_LIMITER"
namespace_id = "1001"

  [ratelimits.simple]
  limit = 10
  period = 60
```

Rules (`checkRequest`, in order): manifest `null` → 404 `not_found`; `state !== 'open'` → 409 `closed`; recipient token not in the manifest → 403 `forbidden`; collaborator with `accept` → 403 `forbidden`; `version`/`checksum` differ → 409 `version_mismatch`; `accept`: `consent !== true` → 422 `consent_required`, `normalizeName(typedName) !== normalizeName(recipient.name)` → 422 `name_mismatch`; `request_changes`: `note` not a string, blank after trim, longer than 4000 code points, or holding a control character other than `\n\r\t` → 422 `note_invalid`. `parseRequest` refuses a non-object, missing/ill-typed fields or an unknown action with 400 `bad_request`.

```ts
export function normalizeName(name: string): string {
  return name.normalize('NFC').trim().replace(/\s+/gu, ' ').toUpperCase().toLowerCase();
}
```

(Upper-then-lower is the case fold JavaScript offers: it folds `ß` to `ss` and the final sigma, which `toLowerCase` alone does not.)

- [ ] **Step 1: Failing tests** (`test/validate.test.ts`)

```ts
import { describe, it, expect } from 'vitest';
import { checkRequest, normalizeName, parseRequest, isRefusal, type Manifest, type ResponseRequest } from '../src/validate.js';

const manifest: Manifest = {
  version: 2, checksum: 'sha256:ab', state: 'open',
  recipients: [
    { token: 'rs', role: 'signer', name: 'Zoë Example' },
    { token: 'rc', role: 'collaborator', name: 'Sam Example' },
  ],
};
const accept: ResponseRequest = { token: 'doc', recipientToken: 'rs', version: 2, checksum: 'sha256:ab', action: 'accept', typedName: 'Zoë Example', consent: true };
const changes: ResponseRequest = { token: 'doc', recipientToken: 'rc', version: 2, checksum: 'sha256:ab', action: 'request_changes', note: 'Fix the dates' };

describe('the role matrix', () => {
  it('lets a signer accept or request changes', () => {
    expect(checkRequest(accept, manifest)).toBeNull();
    expect(checkRequest({ ...changes, recipientToken: 'rs' }, manifest)).toBeNull();
  });
  it('lets a collaborator only request changes', () => {
    expect(checkRequest(changes, manifest)).toBeNull();
    expect(checkRequest({ ...accept, recipientToken: 'rc', typedName: 'Sam Example' }, manifest)?.status).toBe(403);
  });
  it('refuses a token the manifest does not name', () => {
    expect(checkRequest({ ...accept, recipientToken: 'nobody' }, manifest)?.code).toBe('forbidden');
  });
});

describe('names', () => {
  it('names_match_across_case_whitespace_and_normalization_but_not_letters', () => {
    for (const typed of ['zoë example', '  ZOË   Example ', 'Zoë Example', 'Zoë\tExample']) {
      expect(checkRequest({ ...accept, typedName: typed }, manifest), typed).toBeNull();
    }
    for (const typed of ['Zoe Example', 'Zoë Exampl', '']) {
      expect(checkRequest({ ...accept, typedName: typed }, manifest)?.code, typed).toBe('name_mismatch');
    }
    expect(normalizeName('STRASSE')).toBe(normalizeName('straße'));
  });
});

describe('consent and notes', () => {
  it('requires consent to be exactly true', () => {
    for (const consent of [false, undefined]) {
      expect(checkRequest({ ...accept, consent }, manifest)?.code).toBe('consent_required');
    }
  });
  it('bounds the note at 1 to 4000 characters of text', () => {
    expect(checkRequest({ ...changes, note: '' }, manifest)?.code).toBe('note_invalid');
    expect(checkRequest({ ...changes, note: '   ' }, manifest)?.code).toBe('note_invalid');
    expect(checkRequest({ ...changes, note: '😀'.repeat(4000) }, manifest)).toBeNull();
    expect(checkRequest({ ...changes, note: 'x'.repeat(4001) }, manifest)?.code).toBe('note_invalid');
    expect(checkRequest({ ...changes, note: 'bell\u0007' }, manifest)?.code).toBe('note_invalid');
    expect(checkRequest({ ...changes, note: '<script>alert(1)</script>' }, manifest)).toBeNull();
  });
});

describe('the manifest', () => {
  it('is 404 when missing and 409 when closed', () => {
    expect(checkRequest(accept, null)?.status).toBe(404);
    expect(checkRequest(accept, { ...manifest, state: 'closed' })?.code).toBe('closed');
  });
  it('refuses a version or checksum it does not carry', () => {
    expect(checkRequest({ ...accept, version: 1 }, manifest)?.code).toBe('version_mismatch');
    expect(checkRequest({ ...accept, checksum: 'sha256:00' }, manifest)?.code).toBe('version_mismatch');
  });
});

describe('parsing', () => {
  it('refuses a malformed body with 400', () => {
    for (const body of [null, 'x', {}, { ...accept, version: '2' }, { ...accept, action: 'delete' }]) {
      const parsed = parseRequest(body);
      expect(isRefusal(parsed) && parsed.status, JSON.stringify(body)).toBe(400);
    }
  });
});
```

- [ ] **Step 2: Run** `cd workers/document-response && npm install && npm test` — Expected: FAIL (`Cannot find module '../src/validate.js'`).
- [ ] **Step 3: Implement** `env.ts` and `validate.ts` (code-point length via `[...note].length`).
- [ ] **Step 4: Run** `npm test && npm run typecheck` — Expected: PASS.
- [ ] **Step 5: Commit**

```bash
git add workers/document-response
git commit -m "Document response Worker: request rules and name normalization (TASK-109.4)"
```

---

### Task 23 (Track B): Worker handler, write-once, rate limit, and CI

**Files:**
- Create: `workers/document-response/src/index.ts`, `workers/document-response/test/memory-r2.ts`, `workers/document-response/test/index.test.ts`
- Modify: `.github/workflows/ci.yml`

**Interfaces — Produces:**

```ts
// src/index.ts
export const RESPOND_PATH = '/d/respond';
export function manifestKey(token: string): string;                                  // d/{token}/manifest.json
export function responseKey(token: string, version: number, recipientToken: string): string; // d/{token}/v{n}/{rt}.json
export async function handle(request: Request, env: Env, now?: () => Date): Promise<Response>;
export default { fetch: (request: Request, env: Env) => handle(request, env) };

// test/memory-r2.ts
export class MemoryBucket implements PrivateBucket {
  objects: Map<string, string>;
  get(key: string): Promise<StoredObject | null>;
  put(key: string, value: string, options?: PutOptions): Promise<object | null>; // null when onlyIf.etagDoesNotMatch === '*' and the key exists
}
export class CountingLimiter implements Limiter { constructor(limit: number); limit(o: { key: string }): Promise<{ success: boolean }> }
```

Handler:

```ts
export async function handle(request: Request, env: Env, now: () => Date = () => new Date()): Promise<Response> {
  const url = new URL(request.url);
  if (url.pathname !== RESPOND_PATH) return reply({ status: 404, code: 'not_found', message: 'Not found.' });
  if (request.method !== 'POST') return reply({ status: 405, code: 'method_not_allowed', message: 'POST only.' });
  let body: unknown;
  try { body = await request.json(); } catch { return reply({ status: 400, code: 'bad_request', message: 'The body is not JSON.' }); }
  const parsed = parseRequest(body);
  if (isRefusal(parsed)) return reply(parsed);
  const { success } = await env.RATE_LIMITER.limit({ key: parsed.recipientToken });
  if (!success) return reply({ status: 429, code: 'rate_limited', message: 'Too many attempts. Wait a minute and try again.' });
  const stored = await env.PRIVATE.get(manifestKey(parsed.token));
  const manifest = stored ? await stored.json<Manifest>() : null;
  const refusal = checkRequest(parsed, manifest);
  if (refusal) return reply(refusal);
  const record = {
    action: parsed.action, version: parsed.version, checksum: parsed.checksum, recipientToken: parsed.recipientToken,
    ...(parsed.action === 'accept' ? { typedName: parsed.typedName, consent: true } : {}),
    note: parsed.action === 'request_changes' ? parsed.note : null,
    receivedAt: now().toISOString().replace(/\.\d{3}Z$/, 'Z'),
    ip: request.headers.get('CF-Connecting-IP'),
    userAgent: request.headers.get('User-Agent'),
  };
  const written = await env.PRIVATE.put(
    responseKey(parsed.token, parsed.version, parsed.recipientToken),
    JSON.stringify(record),
    { onlyIf: { etagDoesNotMatch: '*' }, httpMetadata: { contentType: 'application/json' } },
  );
  if (written === null) return reply({ status: 409, code: 'already_responded', message: 'A response for this version is already recorded.' });
  return new Response(JSON.stringify({ ok: true, message: 'Received: thank you' }), { status: 200, headers: { 'Content-Type': 'application/json' } });
}
```

(`reply(refusal)` answers `{ code, message }` with the refusal's status and `Content-Type: application/json`.) The Worker reads only `PRIVATE` and never touches the public bucket.

**Resolved risk:** the spec names `onlyIf: { etagDoesNotMatch: "*" }`. R2 documents conditional puts and the `If-None-Match` header semantics; before the first deploy, confirm under `npx wrangler dev --remote` that a second `put` with this condition returns `null` (Task 39 puts this step in the deploy docs). If it does not, pass `onlyIf: new Headers({ 'If-None-Match': '*' })` instead; the in-memory fake models the documented behaviour either way.

- [ ] **Step 1: Failing tests** (`test/index.test.ts`)

```ts
import { describe, it, expect, beforeEach } from 'vitest';
import { handle, manifestKey, responseKey } from '../src/index.js';
import { MemoryBucket, CountingLimiter } from './memory-r2.js';

const MANIFEST = { version: 2, checksum: 'sha256:ab', state: 'open', recipients: [
  { token: 'rs', role: 'signer', name: 'Pat Example' }, { token: 'rc', role: 'collaborator', name: 'Sam Example' } ] };
const ACCEPT = { token: 'doc', recipientToken: 'rs', version: 2, checksum: 'sha256:ab', action: 'accept', typedName: 'pat example', consent: true };

let bucket: MemoryBucket;
let env: { PRIVATE: MemoryBucket; RATE_LIMITER: CountingLimiter };
const post = (body: unknown, path = '/d/respond') =>
  handle(new Request(`https://docs.example.test${path}`, { method: 'POST', body: JSON.stringify(body),
    headers: { 'CF-Connecting-IP': '203.0.113.7', 'User-Agent': 'UA' } }), env, () => new Date('2026-10-05T17:04:11.123Z'));

beforeEach(() => {
  bucket = new MemoryBucket();
  bucket.objects.set(manifestKey('doc'), JSON.stringify(MANIFEST));
  env = { PRIVATE: bucket, RATE_LIMITER: new CountingLimiter(10) };
});

describe('POST /d/respond', () => {
  it('writes the response the sync expects', async () => {
    const res = await post(ACCEPT);
    expect(res.status).toBe(200);
    expect(JSON.parse(bucket.objects.get(responseKey('doc', 2, 'rs'))!)).toEqual({
      action: 'accept', version: 2, checksum: 'sha256:ab', recipientToken: 'rs', typedName: 'pat example',
      consent: true, note: null, receivedAt: '2026-10-05T17:04:11Z', ip: '203.0.113.7', userAgent: 'UA' });
  });
  it('is write-once: the second post gets 409 already_responded', async () => {
    expect((await post(ACCEPT)).status).toBe(200);
    const again = await post({ ...ACCEPT, action: 'request_changes', note: 'x', typedName: undefined, consent: undefined });
    expect(again.status).toBe(409);
    expect((await again.json()).code).toBe('already_responded');
  });
  it('two_concurrent_posts_write_one_response', async () => {
    const [a, b] = await Promise.all([post(ACCEPT), post(ACCEPT)]);
    expect([a.status, b.status].sort()).toEqual([200, 409]);
    expect([...bucket.objects.keys()].filter((k) => k.endsWith('.json') && !k.endsWith('manifest.json'))).toHaveLength(1);
  });
  it('rate-limits per recipient token', async () => {
    env.RATE_LIMITER = new CountingLimiter(1);
    expect((await post({ ...ACCEPT, typedName: 'wrong' })).status).toBe(422);
    expect((await post(ACCEPT)).status).toBe(429);
    expect((await post({ ...ACCEPT, recipientToken: 'rc', action: 'request_changes', note: 'x', typedName: undefined, consent: undefined })).status).toBe(200);
  });
  it('refuses a version mismatch, a closed manifest, and a missing one', async () => {
    const mismatch = await post({ ...ACCEPT, version: 1 });
    expect(mismatch.status).toBe(409);
    expect((await mismatch.json()).code).toBe('version_mismatch');
    bucket.objects.set(manifestKey('doc'), JSON.stringify({ ...MANIFEST, state: 'closed' }));
    expect((await post(ACCEPT)).status).toBe(409);
    expect((await post({ ...ACCEPT, token: 'other' })).status).toBe(404);
  });
  it('answers 404 off the respond path and 400 for a body that is not JSON', async () => {
    expect((await post(ACCEPT, '/d/other')).status).toBe(404);
    const res = await handle(new Request('https://docs.example.test/d/respond', { method: 'POST', body: '{' }), env);
    expect(res.status).toBe(400);
  });
  it('never writes to anything but the private bucket', async () => {
    await post(ACCEPT);
    expect([...bucket.objects.keys()].every((k) => k.startsWith('d/doc/'))).toBe(true);
  });
});
```

`MemoryBucket.put` checks for the existing key and inserts **synchronously** before its first `await`, so two concurrent calls race the way R2's conditional write does: exactly one wins.

- [ ] **Step 2: Run** `cd workers/document-response && npm test` — Expected: FAIL.
- [ ] **Step 3: Implement** `index.ts`, `memory-r2.ts`; add to `.github/workflows/ci.yml` in the `check` job, after "Build web":

```yaml
      - name: Install Worker dependencies
        working-directory: workers/document-response
        run: npm ci

      - name: Typecheck Worker
        working-directory: workers/document-response
        run: npm run typecheck

      - name: Test Worker
        working-directory: workers/document-response
        run: npm test
```

A separate npm project outside `web/` is fine for CI: the `changes` filter counts `workers/**` as code, `actions/setup-node` is already set up in that job, and `npm ci` needs only the committed `workers/document-response/package-lock.json`.

- [ ] **Step 4: Run** `npm ci && npm run typecheck && npm test` — Expected: PASS.
- [ ] **Step 5: Commit**

```bash
git add workers/document-response .github/workflows/ci.yml
git commit -m "Document response Worker: write-once responses, rate limit, CI step (TASK-109.4)"
```

---

# Phase 5 — HTTP API (TASK-109.5)

**Wire DTOs** (in `server/routes/documents.rs`, all `#[serde(rename_all = "camelCase")]`):

```rust
#[derive(Serialize)] struct RecipientView { #[serde(flatten)] recipient: Recipient, page_url: Option<String> }
#[derive(Serialize)] struct VersionView { #[serde(flatten)] version: DocumentVersion, recipients: Vec<RecipientView>, signatures: Vec<Signature>, change_requests: Vec<ChangeRequest> }
#[derive(Serialize)] struct DocumentDetail {
    #[serde(flatten)] document: Document, client: Client, versions: Vec<VersionView>,
    can_edit: bool, can_send: bool, can_revise: bool, can_accept: bool, can_request_changes: bool,
    can_decline: bool, can_countersign: bool, can_withdraw: bool,
}
#[derive(Serialize)] struct ActionResult { #[serde(flatten)] document: DocumentDetail, warnings: Vec<String> }
#[derive(Serialize)] struct SendStepResult { step: DocumentSendStep, outcome: StepOutcome }
#[derive(Serialize)] struct SendResult { document: DocumentDetail, steps: Vec<SendStepResult>, links: Vec<RecipientLink>, config_warnings: Vec<String>, warnings: Vec<String> }
#[derive(Serialize)] struct SyncResult { #[serde(flatten)] report: DocumentSyncReport }
```

`can_*` = `guards::can(status, Action::…)`, and `can_send`/`can_revise` also require `ensure_client_active_for_documents(…).is_ok()` — the guards called, never re-derived. `page_url` is `document_page_url(base, token, rt)` only for sent versions when a documents base is configured. No `token` field is serialized; recipient page URLs, which embed the tokens, are returned only for sent versions, to the loopback, cookie-authenticated SPA.

### Task 24: Read routes — list, detail, kinds, preview

**Files:**
- Create: `crates/nigel-core/src/server/routes/documents.rs`
- Modify: `crates/nigel-core/src/server/routes/mod.rs`, `crates/nigel-core/src/server/testutil.rs`

**Interfaces — Produces:** `pub fn routes() -> Router<AppState>` with `GET /documents` (query `clientId`, `status`, `kind` as strings → 400 on a malformed one; unknown client → 404 `client_not_found`; unknown status word → 400), `GET /documents/{id}` (404 `document_not_found`), `GET /document-kinds` (active only), `GET /documents/{id}/preview` (signer page, `PageState::Open { response_url }`, `pdf_href = "/api/documents/{id}/preview.pdf"`, headers `Content-Type: text/html; charset=utf-8`, `Content-Security-Policy: sandbox`, `X-Frame-Options: SAMEORIGIN`), `GET /documents/{id}/preview.pdf` (the latest version's bytes, `application/pdf`, `Content-Disposition: inline; filename="<attachment_name>"`). No preview depends on the `pdf` feature, so neither answers 501. `testutil.rs`: `seeded_db()` files one draft document (id 1) right after `seed(&conn)` with `file_document(&conn, db_path.parent().unwrap(), &NewDocument { client_id: 1, kind: "Proposal", title: "Website rebuild" }, &fixture_pdf("seed"), "2026-10-05")` for the client the seed already creates as id 1 ("Acme Co") — no new client, so no existing count changes — writing the fixture PDF beside the database, which is the directory `AppState::data_dir()` derives; `DATA_ROUTES` gains `/api/documents`, `/api/documents/1`, `/api/document-kinds` (array length 27) so `unlocking_opens_every_data_route` proves each answers 200; `PREVIEW_ROUTES` gains the two document previews (length 4).

- [ ] **Step 1: Failing tests** — `list_filters_and_404s_an_unknown_client`, `detail_never_carries_a_token`, `detail_can_flags_are_the_guards_called` (a draft: `canEdit`, `canSend`, `canWithdraw` true and the rest false; after the client is archived `canSend` false), `kinds_lists_only_active_kinds`, `preview_is_sandboxed_and_names_the_pdf_route`, `preview_pdf_answers_the_filed_bytes`, `a_sent_document_carries_page_urls_but_no_token_field`, plus the existing locked-guard sweep in `server/mod.rs` covering the new `DATA_ROUTES`/`PREVIEW_ROUTES` without edits.

```rust
#[tokio::test]
async fn detail_never_carries_a_token() {
    let _config = TempConfig::new();
    let (_dir, db_path) = seeded_db();
    let (app, token) = app_for(&db_path);
    let body = ok_json(&app, "/api/documents/1", &token).await;
    let text = body.to_string();
    assert!(body.get("token").is_none());
    assert!(body["versions"][0].get("filePath").is_none());
    let doc_token: String = crate::db::open_connection(&db_path, None).unwrap()
        .query_row("SELECT token FROM documents WHERE id = 1", [], |r| r.get(0)).unwrap();
    assert!(!text.contains(&doc_token));
}

#[tokio::test]
async fn a_sent_document_carries_page_urls_but_no_token_field() {
    let _config = TempConfig::new();
    let mut settings = crate::settings::load_settings();
    settings.public_base_url = Some("https://billing.example.test/i".to_string());
    crate::settings::save_settings(&settings).expect("settings");
    let (_dir, db_path) = seeded_db();
    let conn = crate::db::open_connection(&db_path, None).unwrap();
    let (id, _, _) = sent_document_with_fakes(&conn, db_path.parent().unwrap());
    drop(conn);
    let (app, token) = app_for(&db_path);
    let body = ok_json(&app, &format!("/api/documents/{id}"), &token).await;
    let recipient = &body["versions"][0]["recipients"][0];
    assert!(body.get("token").is_none() && recipient.get("token").is_none());
    assert!(recipient["pageUrl"].as_str().unwrap().ends_with("/index.html"));
}
```

- [ ] **Step 2: Run** `cargo test -p nigel-core routes::documents -- --test-threads=1` — Expected: FAIL.
- [ ] **Step 3: Implement**; mount `.merge(documents::routes())` in `data_router`.
- [ ] **Step 4: Run** `cargo test -p nigel-core -- --test-threads=1` — Expected: PASS.
- [ ] **Step 5: Commit** `git add crates/nigel-core/src/server && git commit -m "GET /api/documents, detail with can* flags, kinds and preview (TASK-109.5)"`

---

### Task 25: Filing, edit and revise routes (multipart through the uploads spool)

**Files:**
- Modify: `crates/nigel-core/src/server/routes/documents.rs`, `crates/nigel-core/src/server/testutil.rs`

**Interfaces — Produces:**
- `POST /documents` — multipart fields `clientId`, `kind`, `title`, `file`; `DefaultBodyLimit::max(uploads::MAX_UPLOAD_BYTES)`; the file name passes `sanitize_filename_for(…, UploadArea::Document)`, the bytes `check_content(UploadArea::Document, …)` (400 on either); the bytes are parked with `uploads::store` (0600/0700, `purge_stale` first), read back, filed with `store::file_document(conn, &state.data_dir(), …)`, then `uploads::delete`; answers 201 with `DocumentDetail`.
- `PATCH /documents/{id}` — `{ title?, kind? }` (`deny_unknown_fields`), 409 with the data layer's sentence outside draft.
- `POST /documents/{id}/revise` — multipart `file`, same checks; calls `revise_with_republish` with `optional_document_publisher`/`optional_response_source` from `documents_config()`; answers `ActionResult`.
- `testutil.rs`: `pub fn multipart_form(fields: &[(&str, &str)], file: Option<(&str, &[u8])>) -> (String, Vec<u8>)` and `pub async fn post_multipart(app, uri, token, fields, file) -> (StatusCode, serde_json::Value)`; `WRITE_ROUTES` gains `("PATCH", "/api/documents/1", r#"{"title":"X"}"#)` (array length 36). The multipart routes are covered by a dedicated locked-guard test in this file because `WRITE_ROUTES` bodies are JSON.

- [ ] **Step 1: Failing tests** — `filing_a_pdf_answers_the_draft_detail`, `a_document_upload_named_pdf_holding_html_is_a_400`, `a_document_upload_with_a_csv_name_is_a_400`, `filing_the_same_pdf_twice_is_a_409_naming_the_document` (`details.reason == "duplicate_document"`), `filing_for_an_archived_client_is_a_409_client_archived`, `the_spool_is_empty_after_filing` (`uploads_dir(&db_path)` has no entries), `patch_outside_draft_is_a_409_with_the_data_layer_sentence`, `revise_from_draft_is_a_409_document_wrong_state`, `the_multipart_routes_are_locked_with_the_database`.

```rust
#[tokio::test]
async fn a_document_upload_named_pdf_holding_html_is_a_400() {
    let _config = TempConfig::new();
    let (_dir, db_path) = seeded_db();
    let (app, token) = app_for(&db_path);
    let (status, json) = post_multipart(&app, "/api/documents", &token,
        &[("clientId", "1"), ("kind", "Proposal"), ("title", "Fake")],
        Some(("fake.pdf", b"<html>%PDF-1.7</html>"))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    assert!(json["error"]["message"].as_str().unwrap().contains("not a PDF"));
}
```

- [ ] **Step 2: Run** — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -p nigel-core -- --test-threads=1` — Expected: PASS.
- [ ] **Step 5: Commit** `git commit -am "File, edit and revise documents over HTTP through the uploads spool (TASK-109.5)"`

---

### Task 26: Send route and the failure seam

**Files:**
- Modify: `crates/nigel-core/src/server/routes/documents.rs`, `crates/nigel-core/src/server/error.rs`, `crates/nigel-core/src/server/testutil.rs`

**Interfaces — Produces:**

```rust
#[derive(Deserialize)] #[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SendRecipient { name: String, email: String }
#[derive(Deserialize, Default)] #[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SendRequest { #[serde(default)] confirm: bool, signer: Option<SendRecipient>, #[serde(default)] collaborators: Vec<SendRecipient> }
fn send_with<P: DocumentPublisher, M: Mailer, R: ResponseSource>(conn: &Connection, data_dir: &Path, id: i64, request: SendRequest, ctx_company: &str, response_url: Option<&str>, today: &str, publisher: &P, mailer: &M, source: &R) -> ApiResult<SendResult>;

// server/error.rs
impl From<DocumentSendFailure> for ApiError { … }
```

Order in the handler: `confirm` first (400 `confirmation_required`, message `"Sending a document requires an explicit confirmation: post {\"confirm\": true}."`); then `documents_status` (409 `send_not_configured` with `step: "config"` and `missing`); then the documents base validity (409 `invalid_public_base_url`); then `build_document_clients` (`send_misconfigured` on `Invalid`); then `with_conn_api` → `send_with`. No signer in the body → `default_signer`. The failure mapping: `Publish`/`Manifest` → 502 `upstream_failed` with `service: "r2"`, `Email` → 502 with `service: "mailgun"`, `Render`/`Freeze`/`Record` → 500 unless the source is a `Conflict` (then the data layer's 409 with its reason and the step merged, e.g. `signer_count` at `freeze`), `Config`/`Load` keep their own code; details always carry `step`, `completed`, `emailed`, `documentStatus`, `cleanupWarnings`, and `reason: "send_failed"` when the code was replaced.

`testutil.rs`: `WRITE_ROUTES` gains `("POST", "/api/documents/1/send", r#"{"confirm":true}"#)` (array length 37), so the locked-database sweep covers the send route.

- [ ] **Step 1: Failing tests** — `send_without_confirmation_is_a_400_and_sends_nothing`, `send_with_no_configuration_is_a_409_naming_the_missing_keys`, `send_with_answers_the_detail_steps_and_links` (fakes; `steps` are the seven post-config words in order; `links[*].url` end with `/index.html`), `a_mail_failure_is_a_502_naming_mailgun_and_who_was_emailed`, and in `server/error.rs` `a_send_conflict_at_freeze_is_a_409_with_its_reason_and_the_step`. The route cannot post two signers (the body takes one `signer` and collaborators), so the several-signers refusal is pinned by the data-layer tests (Tasks 6 and 16) and by this mapping test:

```rust
#[test]
fn a_send_conflict_at_freeze_is_a_409_with_its_reason_and_the_step() {
    let failure = DocumentSendFailure {
        step: DocumentSendStep::Freeze,
        completed: vec![DocumentSendStep::Load, DocumentSendStep::Render],
        emailed: vec![],
        document_status: Some(DocumentStatus::Draft),
        cleanup_warnings: vec![],
        source: NigelError::Conflict { code: "signer_count", message: "A document is sent to exactly one signer.".into() },
    };
    let err = ApiError::from(failure);
    assert_eq!(err.code.status().as_u16(), 409);
    let details = err.details.unwrap();
    assert_eq!((details["reason"].as_str(), details["step"].as_str()), (Some("signer_count"), Some("freeze")));
}
```
- [ ] **Step 2: Run** — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** — Expected: PASS.
- [ ] **Step 5: Commit** `git add crates/nigel-core/src/server && git commit -m "POST /api/documents/{id}/send with confirmation and the step trace (TASK-109.5)"`

---

### Task 27: Manual verbs, withdraw and sync routes

**Files:**
- Modify: `crates/nigel-core/src/server/routes/documents.rs`, `crates/nigel-core/src/server/testutil.rs`

**Interfaces — Produces:**
- `POST /documents/{id}/accept` `{ name, date? }`, `…/request-changes` `{ name, note, date? }`, `…/decline` `{ note?, date? }`, `…/countersign` `{ name, date? }`, `…/withdraw` `{}` — each validates, records through `record.rs` (date defaults to `clock::today()`), then `republish_after_change` / `withdraw_with_teardown` with the optional collaborators; answers `ActionResult` (200, warnings as data — the void precedent).
- `POST /documents/sync` — 409 `sync_not_configured` naming the private-store keys when unset; `sync_documents(…, Some(Instant::now() + SYNC_BUDGET))` with `const SYNC_BUDGET: Duration = Duration::from_secs(60)`; an `Err` from a run where every document failed is 502 `upstream_failed` with `service: "r2"`.
- `WRITE_ROUTES` gains the six JSON routes (length 43).

- [ ] **Step 1: Failing tests** — `accept_then_countersign_reaches_executed_and_the_flags_follow` (`canCountersign` true only while accepted), `decline_from_accepted_is_a_409_document_accepted`, `request_changes_with_an_empty_note_is_a_400`, `withdraw_answers_its_teardown_warnings_as_data`, `sync_with_nothing_configured_is_a_409_naming_the_keys`, `sync_with_records_an_online_accept` (route-level `sync_with` seam over fakes).
- [ ] **Step 2: Run** — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -- --test-threads=1` — Expected: PASS.
- [ ] **Step 5: Commit** `git commit -am "Manual responses, withdraw and sync over HTTP (TASK-109.5)"`

---

# Phase 6 — Web (TASK-109.7)

UI track (Tasks 28–32) owns `web/packages/ui/**` and may start with Phase 5. App track (Tasks 33–37) owns `web/apps/app/**` and starts after Task 27 and Task 32. Every component: tokens from `@nigel/theme`, preview states declared once, `describePreviewA11y(preview)` in its test, export from `components/index.ts`.

### Task 28 (UI): `wc-document-status`

**Files:** Create `web/packages/ui/src/components/wc-document-status.ts`, `.preview.ts`, `.test.ts`; modify `components/index.ts`.

**Interfaces — Produces:**

```ts
export const DOCUMENT_STATUS_WORDS = ['draft', 'sent', 'changes_requested', 'accepted', 'declined', 'executed', 'withdrawn'] as const;
export type DocumentStatusWord = (typeof DOCUMENT_STATUS_WORDS)[number];
export function documentStatusLabel(status: string): string; // 'changes_requested' → 'changes requested'; unknown → as-is
@customElement('wc-document-status') export class WcDocumentStatus extends LitElement { @property({ type: String, reflect: true }) status = 'draft'; }
```

Icons (decorative; the word carries meaning): draft `wc-icon-status-draft`, sent `wc-icon-status-sent`, changes_requested `wc-icon-status-partial`, accepted `wc-icon-check`, declined `wc-icon-close`, executed `wc-icon-status-paid`, withdrawn `wc-icon-status-void`; unknown `wc-icon-dot`. Colours: sent `--wa-color-brand`, changes_requested `--nc-color-flagged`, accepted/executed `--nc-color-income`, declined `--nc-color-expense`, draft/withdrawn `--wa-color-muted` (withdrawn struck through). Lookup through a `Map` (the `wc-invoice-status` reason). No `wa-*` primitive, so no `controlsCss`.

- [ ] **Step 1: Failing test**

```ts
import { describe, it, expect, afterEach } from 'vitest';
import './wc-document-status.js';
import { documentStatusLabel } from './wc-document-status.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-document-status.preview.js';

describe('wc-document-status', () => {
  afterEach(() => { document.body.innerHTML = ''; });
  it('words changes_requested for a person', () => {
    expect(documentStatusLabel('changes_requested')).toBe('changes requested');
    expect(documentStatusLabel('constructor')).toBe('constructor');
  });
  it('renders the word beside the mark', async () => {
    const el = document.createElement('wc-document-status');
    el.status = 'accepted';
    document.body.appendChild(el);
    await el.updateComplete;
    expect(el.shadowRoot?.querySelector('.word')?.textContent).toBe('accepted');
    expect(el.shadowRoot?.querySelector('wc-icon-check')).toBeTruthy();
  });
});

describePreviewA11y(preview);
```

Preview: `id: 'wc-document-status'`, `group: 'Documents'`, one state per word plus `unknown`.

- [ ] **Step 2: Run** `cd web && npx vitest run -w @nigel/ui wc-document-status` (or `npm test -w @nigel/ui -- wc-document-status`) — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** — Expected: PASS.
- [ ] **Step 5: Commit** `git add web/packages/ui && git commit -m "wc-document-status (TASK-109.7)"`

---

### Task 29 (UI): `wc-document-table`

**Files:** Create `wc-document-table.ts`, `.preview.ts`, `.test.ts`; modify `index.ts`.

**Interfaces — Produces:**

```ts
export interface DocumentTableRow {
  id: number; title: string; kind: string; status: string;
  clientName: string | null; version: number; updatedAt: string; href?: string;
}
@customElement('wc-document-table') export class WcDocumentTable extends LitElement {
  @property({ attribute: false }) rows: DocumentTableRow[] = [];
  @property({ type: Boolean, reflect: true }) loading = false;
  @property({ type: String }) caption = 'Documents';
  @property({ type: String, attribute: 'empty-message' }) emptyMessage = 'No documents yet.';
}
```

Columns: Title (linked when `href`), Status (`wc-document-status`), Kind, Client (em dash when null), Version (`v2`), Updated. The `wc-invoice-table` shape: visually hidden caption, spinner while loading, `data-empty` paragraph.

- [ ] **Step 1: Failing test** — rows keyed by `data-row=id`; titles link only with `href`; a null client is an em dash; loading before empty; `describePreviewA11y(preview)`. Preview states: `list` (Cedar Systems / Juniper Labs / Harbor & Vale rows in every status), `orphaned-client`, `loading`, `empty`, `unlinked`.
- [ ] **Step 2–4:** run (FAIL), implement, run (PASS) with `npm test -w @nigel/ui`.
- [ ] **Step 5: Commit** `git commit -m "wc-document-table (TASK-109.7)"` after `git add web/packages/ui`.

---

### Task 30 (UI): `wc-document-timeline`

**Files:** Create `wc-document-timeline.ts`, `.preview.ts`, `.test.ts`; modify `index.ts`.

**Interfaces — Produces:**

```ts
export interface TimelineRecipient { role: 'signer' | 'collaborator'; name: string; email: string }
export interface TimelineEvent {
  kind: 'signature' | 'change_request';
  role: 'client' | 'countersign' | null;     // null for change requests
  name: string; email: string | null; method: 'online' | 'manual'; at: string;
  typedName: string | null; ip: string | null; userAgent: string | null; note: string | null;
}
export interface TimelineVersion { number: number; checksum: string; sentAt: string | null; createdAt: string; recipients: TimelineRecipient[]; events: TimelineEvent[] }
@customElement('wc-document-timeline') export class WcDocumentTimeline extends LitElement {
  @property({ attribute: false }) versions: TimelineVersion[] = [];   // newest first
}
```

One `<section aria-labelledby>` per version (each event an element with `data-event`, its evidence line `data-evidence`): heading "Version 2", the checksum in a `<code>`, "Sent 2026-10-05" or "Draft — not sent", a recipients list (role + name + email), then events: "Accepted by Pat Example — online, 2026-10-05T17:04:11Z", evidence line (typed name, IP, user agent) for online events, "Countersigned by …", "Changes requested by …" with the note in a `<p class="note">` bound as a **text** expression (`${event.note}`) with `white-space: pre-wrap`. Never `unsafeHTML`.

- [ ] **Step 1: Failing tests**

```ts
const V1: TimelineVersion = { number: 1, checksum: 'sha256:aa', sentAt: '2026-10-05', createdAt: '2026-10-05',
  recipients: [{ role: 'signer', name: 'Pat Example', email: 'pat@cedar.test' }, { role: 'collaborator', name: 'Sam Example', email: 'sam@cedar.test' }],
  events: [] };
const ACCEPT: TimelineEvent = { kind: 'signature', role: 'client', name: 'Pat Example', email: 'pat@cedar.test', method: 'manual',
  at: '2026-10-06', typedName: null, ip: null, userAgent: null, note: null };
const CHANGE: TimelineEvent = { kind: 'change_request', role: null, name: 'Sam Example', email: 'sam@cedar.test', method: 'online',
  at: '2026-10-05T17:04:11Z', typedName: null, ip: '203.0.113.7', userAgent: 'UA', note: 'Fix the dates' };
async function mount(versions: TimelineVersion[]): Promise<WcDocumentTimeline> {
  const el = document.createElement('wc-document-timeline');
  el.versions = versions;
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

it('a_note_with_markup_renders_as_text', async () => {
  const el = await mount([{ ...V1, events: [{ ...CHANGE, note: '<img src=x onerror="alert(1)">\nline two' }] }]);
  const note = el.shadowRoot!.querySelector('.note')!;
  expect(note.querySelector('img')).toBeNull();
  expect(note.textContent).toContain('<img src=x');
});
it('shows the evidence of an online acceptance and none for a manual one', async () => {
  const el = await mount([{ ...V1, events: [
    { ...ACCEPT, method: 'online', typedName: 'Pat Example', ip: '203.0.113.7', userAgent: 'UA' },
    { ...ACCEPT, role: 'countersign', name: 'Sam Example', method: 'manual', typedName: null, ip: null, userAgent: null },
  ] }]);
  const events = [...el.shadowRoot!.querySelectorAll('[data-event]')];
  expect(events[0].querySelector('[data-evidence]')?.textContent).toContain('203.0.113.7');
  expect(events[1].querySelector('[data-evidence]')).toBeNull();
});
it('marks an unsent version as a draft', async () => {
  const el = await mount([{ ...V1, sentAt: null, recipients: [], events: [] }]);
  expect(el.shadowRoot!.textContent).toContain('Draft — not sent');
});
```

Preview states: `draft-only`, `sent-with-change-request`, `revised-and-accepted` (two versions), `executed`, `manual-only`.

- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** `git commit -m "wc-document-timeline: one block per version, notes as text (TASK-109.7)"`

---

### Task 31 (UI): `wc-recipient-editor`

**Files:** Create `wc-recipient-editor.ts`, `.preview.ts`, `.test.ts`; modify `index.ts`.

**Interfaces — Produces:**

```ts
export interface RecipientValue { name: string; email: string }
export interface RecipientEditorValue { signer: RecipientValue; collaborators: RecipientValue[] }
export interface RecipientContactOption { name: string | null; email: string; isBilling: boolean }
export interface RecipientErrors { signerName?: string; signerEmail?: string; collaborators?: Record<number, string>; duplicate?: string }
export interface NcRecipientsChangeDetail { value: RecipientEditorValue }
export function defaultRecipients(contacts: RecipientContactOption[]): RecipientEditorValue; // signer = billing contact
export function validateRecipients(value: RecipientEditorValue): RecipientErrors;          // {} when valid
@customElement('wc-recipient-editor') export class WcRecipientEditor extends LitElement {
  static styles = [controlsCss, css`…`];
  @property({ attribute: false }) value: RecipientEditorValue = { signer: { name: '', email: '' }, collaborators: [] };
  @property({ attribute: false }) contacts: RecipientContactOption[] = [];
  @property({ attribute: false }) errors: RecipientErrors = {};
  @property({ type: Boolean, reflect: true }) disabled = false;
  // emits 'nc-recipients-change' (bubbles, composed) with NcRecipientsChangeDetail
}
```

Layout: a "Signer" fieldset (name + email `wa-input`s — exactly one, never removable), a "Collaborators" fieldset (rows of name + email + a remove `wa-button`), an "Add collaborator" `wa-button`, and one "Add {name or email}" `wa-button` per client contact not already listed. Validation: signer name and email required; every email contains `@`; no address twice (case-insensitive) → `duplicate`. Imports: `@awesome.me/webawesome/dist/components/input/input.js`, `…/button/button.js`.

- [ ] **Step 1: Failing tests** — `defaultRecipients` picks the billing contact as signer and leaves collaborators empty; `validateRecipients` refuses a blank signer name, a duplicate address and a collaborator with no `@`; adding a contact emits `nc-recipients-change` with it appended; removing a collaborator emits without it; there is never a control to remove the signer; `describePreviewA11y(preview)`. Preview states: `default`, `with-collaborators`, `errors`, `disabled`, `no-contacts`.
- [ ] **Step 2–4:** FAIL → implement → PASS (also `controls-adoption.test.ts`).
- [ ] **Step 5: Commit** `git commit -m "wc-recipient-editor: one signer, any collaborators (TASK-109.7)"`

---

### Task 32 (UI): `wc-send-dialog` document mode

**Files:** Modify `wc-send-dialog.ts`, `wc-send-dialog.preview.ts`, `wc-send-dialog.test.ts`.

**Interfaces — Produces:**

```ts
@property({ type: String, reflect: true }) mode: 'invoice' | 'document' = 'invoice';
@property({ type: String, attribute: false }) documentTitle = '';
@property({ type: Number, attribute: false }) recipientCount = 0;
@property({ type: Boolean, attribute: false }) responseForm = true;
// <slot name="recipients"> rendered in the confirm phase, document mode only
```

Document mode: label `Send “${documentTitle}”?`; consequences "publish the document{ to host}", "email each of the {n} recipients their own link with the PDF attached", and either "open it for online responses" or "with no response form — responses are recorded by hand"; no money line; sent outcome `“${documentTitle}” is on its way to ${n} recipient${n === 1 ? '' : 's'}.`; preview spinner label "Rendering the document". Invoice mode renders exactly what it does today (existing tests unchanged).

- [ ] **Step 1: Failing tests** — `document mode names the title and never mentions Stripe`, `the recipients slot renders in the confirm phase only`, `wa-hide is still prevented while sending in document mode`. Preview gains `document-confirm` (with a `wc-recipient-editor` in the slot), `document-sending`, `document-sent`, `document-failed`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** `git commit -m "wc-send-dialog hosts a document send with a recipients slot (TASK-109.7)"`

---

### Task 33 (App): API types, client, desktop client and fake

**Files:** Modify `web/apps/app/src/api/types.ts`, `api/client.ts`, `api/desktop-client.ts`, `api/client.test.ts`, `api/desktop-client.test.ts`, `__mocks__/fake-api-client.ts`.

**Interfaces — Produces:**

```ts
export type DocumentStatus = 'draft' | 'sent' | 'changes_requested' | 'accepted' | 'declined' | 'executed' | 'withdrawn';
export type RecipientRole = 'signer' | 'collaborator';
export interface DocumentKind { id: number; name: string; active: boolean; position: number }
export interface DocumentListRow { id: number; title: string; kind: string; clientId: number; clientName: string | null; status: DocumentStatus; latestVersion: number; sentAt: string | null; updatedAt: string }
export interface DocumentRecipient { id: number; versionId: number; role: RecipientRole; name: string; email: string; position: number; pageUrl: string | null }
export interface DocumentSignature { id: number; versionId: number; recipientId: number | null; role: 'client' | 'countersign'; name: string; email: string | null; method: 'online' | 'manual'; signedAt: string; typedName: string | null; ip: string | null; userAgent: string | null; checksum: string }
export interface DocumentChangeRequest { id: number; versionId: number; recipientId: number | null; name: string; email: string | null; method: 'online' | 'manual'; requestedAt: string; note: string; ip: string | null; userAgent: string | null; checksum: string }
export interface DocumentVersionDetail { id: number; documentId: number; number: number; checksum: string; sentAt: string | null; createdAt: string; recipients: DocumentRecipient[]; signatures: DocumentSignature[]; changeRequests: DocumentChangeRequest[] }
export interface DocumentDetail {
  id: number; clientId: number; kindId: number; kind: string; title: string;
  declinedAt: string | null; declineNote: string | null; withdrawnAt: string | null;
  createdAt: string; updatedAt: string; status: DocumentStatus; client: Client; versions: DocumentVersionDetail[];
  canEdit: boolean; canSend: boolean; canRevise: boolean; canAccept: boolean; canRequestChanges: boolean;
  canDecline: boolean; canCountersign: boolean; canWithdraw: boolean;
}
export interface DocumentListParams { clientId?: number; status?: string; kind?: string }
export interface NewDocumentRequest { clientId: number; kind: string; title: string; file: File }
export interface DocumentPatch { title?: string; kind?: string }
export interface DocumentRecipientInput { name: string; email: string }
export interface DocumentSendRequest { signer: DocumentRecipientInput; collaborators: DocumentRecipientInput[] }
export const DOCUMENT_SEND_STEPS = ['config', 'load', 'render', 'freeze', 'publish', 'manifest', 'email', 'record'] as const;
export type DocumentSendStep = (typeof DOCUMENT_SEND_STEPS)[number];
export interface RecipientLink { role: RecipientRole; name: string; email: string; url: string }
export interface DocumentSendResult { document: DocumentDetail; steps: { step: DocumentSendStep; outcome: SendStepOutcome }[]; links: RecipientLink[]; configWarnings: string[]; warnings: string[] }
export interface DocumentSendErrorDetails { reason?: string; step?: DocumentSendStep; completed?: DocumentSendStep[]; emailed?: string[]; documentStatus?: DocumentStatus; cleanupWarnings?: string[]; service?: 'r2' | 'mailgun'; missing?: string[] }
export interface ManualAcceptRequest { name: string; date?: string }
export interface ManualChangeRequestInput { name: string; note: string; date?: string }
export interface DeclineRequest { note?: string; date?: string }
export interface CountersignRequest { name: string; date?: string }
export interface DocumentActionResult extends DocumentDetail { warnings: string[] }
export interface DocumentSyncLine { documentId: number; title: string; recorded: string[]; refused: string[]; warnings: string[]; status: DocumentStatus }
export interface DocumentSyncResult { documentsChecked: number; recorded: number; lines: DocumentSyncLine[]; failures: { documentId: number; message: string }[] }
```

`CONFLICT_REASONS` gains `has_documents`, `document_terminal`, `document_accepted`, `document_wrong_state`, `duplicate_document`, `unchanged_revision`, `signer_count`, `signer_name_required`, `kind_inactive`, `file_changed`, `stale_version`, `checksum_mismatch`, `role_not_allowed`, `sync_not_configured`, `invalid_public_base_url`. `NOT_FOUND_REASONS` gains `document_not_found`.

`ApiClient` methods (each implemented on `FetchApiClient` against Task 24–27 routes; `createDocument` and `reviseDocument` send `FormData`; `sendDocument` adds `confirm: true`):

```ts
getDocuments(params?: DocumentListParams): Promise<DocumentListRow[]>;
getDocument(id: number): Promise<DocumentDetail>;
getDocumentKinds(): Promise<DocumentKind[]>;
createDocument(input: NewDocumentRequest): Promise<DocumentDetail>;
updateDocument(id: number, input: DocumentPatch): Promise<DocumentDetail>;
sendDocument(id: number, input: DocumentSendRequest): Promise<DocumentSendResult>;
reviseDocument(id: number, file: File): Promise<DocumentActionResult>;
acceptDocument(id: number, input: ManualAcceptRequest): Promise<DocumentActionResult>;
requestDocumentChanges(id: number, input: ManualChangeRequestInput): Promise<DocumentActionResult>;
declineDocument(id: number, input: DeclineRequest): Promise<DocumentActionResult>;
countersignDocument(id: number, input: CountersignRequest): Promise<DocumentActionResult>;
withdrawDocument(id: number): Promise<DocumentActionResult>;
syncDocuments(): Promise<DocumentSyncResult>;
documentPreviewUrl(id: number, format: 'html' | 'pdf'): string;
documentPreviewTarget(id: number): ExportTarget;
documentPreviewHtml(id: number): Promise<string>;
```

`DesktopApiClient` overrides `documentPreviewTarget` with `this.save(this.documentPreviewUrl(id, 'pdf'), \`document-${id}.pdf\`)`. Desktop drag-and-drop of a PDF onto the screen is not wired (the native drop channel stages statements only); the file picker inside `wc-dropzone` works in both shells and is the documented path.

`FakeApiClient`: `documents: DocumentListRow[]`, `documentDetails: Record<number, DocumentDetail>`, `documentKinds`, per-method `…Error` fields, `calls` entries (`sendDocument:{id}:{json}` etc.), and transitions mirroring the guard table (send → sent; `requestDocumentChanges` → changes_requested; revise → draft with a new version; accept → accepted; countersign → executed; withdraw → withdrawn), each recomputing the eight `can*` flags from one local copy of the table.

- [ ] **Step 1: Failing tests** (`client.test.ts`): `createDocument posts FormData with clientId kind title and file`, `sendDocument always carries confirm true`, `getDocuments omits absent filters`; (`desktop-client.test.ts`): `documentPreviewTarget saves through the native side`.
- [ ] **Step 2–4:** `cd web && npm test -w @nigel/app -- api` FAIL → implement → PASS; `npm run typecheck`.
- [ ] **Step 5: Commit** `git add web/apps/app/src/api web/apps/app/src/__mocks__ && git commit -m "Document API types, client methods and fakes (TASK-109.7)"`

---

### Task 34 (App): `documents-data.ts` and `documents-errors.ts`

**Files:** Create `web/apps/app/src/screens/documents-data.ts` (+ `.test.ts`), `documents-errors.ts` (+ `.test.ts`); modify `screens/invoicing-errors.ts` (+ test) for `has_documents`.

**Interfaces — Produces:**

```ts
// documents-data.ts
export const DOCUMENT_STATUS_FILTERS: { value: string; label: string }[]; // all + seven
export function documentListParams(params: URLSearchParams): DocumentListParams;
export function documentTableRows(rows: DocumentListRow[]): DocumentTableRow[];        // href '#/documents?id={id}'
export function timelineVersions(detail: DocumentDetail): TimelineVersion[];            // newest first; events sorted by time
export function recipientContacts(client: ClientDetail): RecipientContactOption[];
export function sendRequestFrom(value: RecipientEditorValue): DocumentSendRequest;      // trims
export const DOCUMENT_SEND_STEP_LABELS: Record<DocumentSendStep, string>;
export function documentSendStepViews(options: { running: boolean; result?: DocumentSendResult; error?: unknown }): SendStepView[];
export function latestSentVersion(detail: DocumentDetail): DocumentVersionDetail | null;

// documents-errors.ts
export function documentsGuardrailMessage(error: unknown): string;   // reason → sentence; 400 and unknown 409 → server message
export function documentSendFailureMessage(error: unknown, title: string): SendFailureView;
```

Sentences (one per reason; e.g. `document_accepted` → "This document is accepted. The only step left is to countersign it.", `duplicate_document` → "This PDF is already filed for this client.", `signer_count` → "A document is sent to exactly one signer.", `client_archived` → "This client is archived. Unarchive it before filing or sending documents.", `send_not_configured` → "Sending documents needs {keys}, which {is|are} not set."). The two fallbacks are deliberate: a 400 renders the server's message (it names the offending value), and an unrecognized 409 reason renders the server's message. A send failure after `emailed` is non-empty is not retryable and its note names who was emailed.

- [ ] **Step 1: Failing tests** — one per sentence; both fallbacks; `timelineVersions` keeps a note's markup as a plain string; `documentSendFailureMessage` marks a mailgun failure with one address emailed as not retryable; `invoicingGuardrailMessage(conflict('has_documents', { count: 2 }), 'client')` → "This client has 2 documents. …".
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** `git commit -m "Document data mappers and the reason-to-sentence table (TASK-109.7)"`

---

### Task 35 (App): Documents screen — list, filters and filing

**Files:** Create `web/apps/app/src/screens/documents.ts`, `documents.test.ts`; modify `screens/registry.ts` (+ `registry.test.ts` if it enumerates ids).

**Interfaces — Produces:**

```ts
type View = 'list' | 'detail';
@customElement('nigel-documents-screen') export class NigelDocumentsScreen extends SignalWatcher(LitElement) {
  @property({ attribute: false }) client!: ApiClient;
  @property({ attribute: false }) params = new URLSearchParams();
  @property({ attribute: false }) navigate!: ScreenContext['navigate'];
}
export function renderDocuments(ctx: ScreenContext): TemplateResult;
// registry: documents: { id: 'documents', title: 'Documents', navLabel: 'Documents', icon: 'wc-icon-edit', inNav: true, render: renderDocuments }
```

List view: status filters as links (`#/documents?status=sent`), a client filter, `wc-document-table`, a "Sync responses" button (`syncDocuments`, report lines shown in a `wc-notice-bar`), and a filing panel: `wc-dropzone accept=".pdf"` + client select (active clients), kind select (`getDocumentKinds`), title input; "File document" calls `createDocument` and navigates to the new detail. A failed load and a refused action are separate states (`error` vs `actionError`), as on the invoices screen.

- [ ] **Step 1: Failing tests** — `lists documents with their status`, `filters are links`, `a failed load is not a refused action`, `filing a PDF creates a draft and opens it`, `a duplicate filing shows the guardrail sentence`, `the dropzone accepts only .pdf`.
- [ ] **Step 2–4:** FAIL → implement → PASS (`screen-freshness`, `screen-layout`, `native-feel`, `api-seam`, `controls-adoption` stay green).
- [ ] **Step 5: Commit** `git commit -m "Documents screen: list, filters and filing (TASK-109.7)"`

---

### Task 36 (App): Detail, actions and send

**Files:** Modify `web/apps/app/src/screens/documents.ts`, `documents.test.ts`.

Detail view: header with title, kind, client and `wc-document-status`; a collapsed `wc-document-frame` previewing `documentPreviewHtml(id)` (srcdoc) with a "Download PDF" link from `documentPreviewTarget(id)`; `wc-document-timeline` from `timelineVersions(detail)`; actions shown only where the server's flag allows — Send (`canSend`), Revise (`canRevise`, a `wc-dropzone` in a `wc-manager-dialog` then `confirmDialog`), Accept/Request changes/Decline/Countersign (each `confirmDialog` collecting name/note/date as the action needs), Withdraw (`canWithdraw`, `confirmDialog`), Edit title/kind (`canEdit`). Every action's `warnings` render as dismissible `wc-notice-bar`s. Send: `wc-send-dialog mode="document"` with `wc-recipient-editor` in its `recipients` slot (defaults from `recipientContacts(await client.getClient(detail.clientId))`), confirm disabled while `validateRecipients` reports errors, the step trace from `documentSendStepViews`, `wa-hide` prevented mid-flight by the dialog, failures through `documentSendFailureMessage`, links listed on success.

- [ ] **Step 1: Failing tests** — `actions follow the can flags, never the status word` (a fake detail with `status: 'sent'` and every flag false shows no action), `send posts the recipient editor's value`, `a send failure after an email went out offers no retry`, `withdraw shows its teardown warnings`, `notes in the timeline render as text`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** `git commit -m "Document detail: timeline, guarded actions and the send dialog (TASK-109.7)"`

---

### Task 37 (App): The end-to-end flow in the browser

**Files:** Modify `web/apps/app/src/screens/documents.test.ts`.

- [ ] **Step 1: Write the flow test** — through `FakeApiClient` only: file "Website rebuild" for Cedar Systems → draft; send to Pat Example (signer) and Sam Example (collaborator) → sent; `fake.requestDocumentChanges` (standing in for a synced online response) then "Sync responses" → changes requested; revise with a new PDF → draft v2; send → sent; accept (manual, Pat Example) → accepted; countersign (Sam Example) → executed; at each step assert the status chip, which actions are visible, and the timeline's version count.
- [ ] **Step 2: Run** `cd web && npm test -w @nigel/app -- documents` — Expected: PASS once Tasks 35–36 hold; if it fails, fix the screen, not the test.
- [ ] **Step 3: Run the web gate** `cd web && npm run lint && npm run typecheck && npm test && npm run build` — Expected: PASS.
- [ ] **Step 4: Commit** `git commit -am "Documents flow from draft to executed in the browser (TASK-109.7)"`

---

# Phase 7 — Docs and the gate

### Task 38: API, architecture and design constraints

**Files:** Modify `docs/api.md`, `docs/architecture.md`, `docs/design-constraints.md`.

- [ ] **Step 1:** `docs/api.md` — a "Documents" subsection under both "Reading data" and "Changing data": every route from Tasks 24–27 with request and response shapes, `can*` flags as the guards called, no `token` field serialized and `pageUrl` (which embeds the tokens) returned only for sent versions to the loopback SPA, multipart filing through the uploads spool (PDF only, by content), `confirmation_required`, the send failure details (`step`, `completed`, `emailed`, `documentStatus`, `cleanupWarnings`, `service`), the sync budget, the new conflict reasons in "Conflict reasons" and `document_not_found` in "Not-found reasons"; state that kinds are managed from the CLI and the API serves documents only.
- [ ] **Step 2:** `docs/architecture.md` — the `documents/` module map (one line per file), the Worker, where documents meet `invoicing` (prefix, `DocumentPublisher`, `ResponseSource`, `Mailer::send`, `DocumentClients`), and the tree on disk (`<data_dir>/documents/<id>/v<n>.pdf`, `<data_dir>/previews/document-<id>/`).
- [ ] **Step 3:** `docs/design-constraints.md` — one entry each: status derived from rows (and why a newer unsent version reads as draft); guards live in the data layer and the `can*` flags call them; exactly one signer; one online response per recipient per version, enforced across both tables; checksums carry `sha256:` and bind every response; send rolls back before mark-sent and closes the manifest; withdraw/revise/accept commit first and republish best-effort; the Worker never touches the public bucket; recorded assent, not a legal e-signature.
- [ ] **Step 4:** `./scripts/check-no-real-data.sh; echo $?` — Expected: `0`.
- [ ] **Step 5: Commit** `git add docs/api.md docs/architecture.md docs/design-constraints.md && git commit -m "Document the documents API, module and constraints (TASK-109)"`

### Task 39: README, commands and the invoicing guide

**Files:** Modify `README.md`, `docs/commands.md`, `docs/invoicing.md`.

- [ ] **Step 1:** `README.md` Features — one **Documents** bullet (file a PDF, send to a signer and collaborators, online accept or change requests, revise, countersign; recorded assent, not a legal e-signature); Configuration — the three new keys.
- [ ] **Step 2:** `docs/commands.md` — every `nigel document …` line from the spec's CLI block with a one-line comment each.
- [ ] **Step 3:** `docs/invoicing.md` — a "Documents" section: configuration (`r2_private_bucket`, `documents_base_url`, `document_response_url`, their `NIGEL_*` overrides, what is required to send and to sync, no Stripe needed); the object layout in both buckets; the manifest and response formats; filing, preview, send (step trace, rollback), revise, manual verbs, withdraw; sync from cron beside `schedule run` (an example crontab line running `nigel document sync`) and at launch; Worker deploy: create the private bucket with no public domain, edit `wrangler.toml` (route on the documents hostname at `/d/respond`, bucket name, a unique rate-limit `namespace_id`), `cd workers/document-response && npm ci && npx wrangler deploy`, confirm write-once with two posts under `npx wrangler dev --remote`, then set `document_response_url`; what signing means (recorded assent, not a legal e-signature).
- [ ] **Step 4:** `./scripts/check-no-real-data.sh; echo $?` — Expected: `0`.
- [ ] **Step 5: Commit** `git add README.md docs/commands.md docs/invoicing.md && git commit -m "Documents in the README, command reference and invoicing guide (TASK-109)"`

### Task 40: Full gate (mirrors `.github/workflows/ci.yml`)

- [ ] **Step 1: Web**

```bash
cd web && npm ci && npm run lint && npm run typecheck && npm test && npm run build
```
Expected: all PASS.

- [ ] **Step 2: Worker**

```bash
cd workers/document-response && npm ci && npm run typecheck && npm test
```
Expected: PASS.

- [ ] **Step 3: Rust (root workspace)**

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo clippy --all-targets -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
cargo test --no-default-features --features serve -- --test-threads=1
cargo test -p nigel-core -- --test-threads=1
```
Expected: all PASS. (CI runs `cargo clippy -- -D warnings`; `--all-targets` is stricter and also run here.)

- [ ] **Step 4: Desktop shell** (it depends on `nigel-core`)

```bash
cd crates/nigel-desktop && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test -- --test-threads=1
```
Expected: PASS.

- [ ] **Step 5: Real-data check**

```bash
./scripts/check-no-real-data.sh; echo "exit=$?"
```
Expected: `exit=0`.

- [ ] **Step 6:** Close the ACs through the CLI only: `backlog task edit 109.1 --check-ac 1 …` for each AC each task discharged, then commit the task files on `main` per the backlog rule (or on this branch when closing as part of the work).

---

## Spec coverage map

| Spec section | Tasks |
|---|---|
| Data model, migration v15 | 1, 2, 5, 6 |
| Status | 3 |
| Transitions and guards | 4, 6 |
| Storage layout, manifest, response | 9, 11, 14, 16 |
| Sending, step trace, rollback | 16, 17 |
| Emails, preview, revise, withdraw | 14, 15, 18 |
| Generalizing the invoicing machinery | 9, 10, 11, 12, 13 |
| The Worker | 22, 23 |
| Sync | 20, 21 |
| CLI | 7, 8, 15, 17, 18, 19, 21 |
| HTTP API | 24, 25, 26, 27 |
| Web UI | 28–37 |
| Settings | 12, 39 |
| Testing | in every task; Review Focus pins in 5, 6, 8, 13, 14, 16, 20, 22, 23, 25, 30 |
| Build order | Phases 1–6 |
