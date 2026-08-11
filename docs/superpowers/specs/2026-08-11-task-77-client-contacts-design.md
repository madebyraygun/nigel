# A client needs more than one email address — `client_contacts`

Task: TASK-77 (epic TASK-86, Stream 3). **The design decision is the
deliverable**; the plumbing follows from it.

## Problem

`clients.email` is a single nullable `TEXT` column (migration v4). One client,
one address, one recipient. Real billing does not look like that: an
organisation has an AP contact and the person who commissioned the work, and
both want the invoice.

The measured shape, from the Harvest contact export used to backfill these
addresses:

- 123 clients, 187 contact emails.
- 41 clients (a third) have more than one; the largest has nine.
- 105 of the 187 addresses belong to a multi-contact client.
- Of the 23 clients imported from InvoiceShelf, 12 had two or three Harvest
  contacts and a person picked one by hand. The other picks were discarded and
  are unrecoverable: the column keeps the winner and nothing remembers there
  were others.
- Every one of the 187 rows carries a **name** and a **title**. None carries the
  billing-default flag, which is why the picking was manual.

## Decision

**A `client_contacts` child table, one row per address, each with a name and a
title, and an `is_billing` flag that exactly one row per client carries.**
`clients.email` is dropped by the migration; `Client.email` survives as a
*derived projection* of the billing contact, so every existing read path renders
what it renders today with no change.

That is Harvest's model, which is the model the source data was already in.

### Schema

```sql
CREATE TABLE client_contacts (
    id         INTEGER PRIMARY KEY,
    client_id  INTEGER NOT NULL,
    name       TEXT,
    email      TEXT NOT NULL,
    title      TEXT,
    is_billing INTEGER NOT NULL DEFAULT 0,
    position   INTEGER NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT (datetime('now')),
    FOREIGN KEY (client_id) REFERENCES clients(id) ON DELETE CASCADE
);

-- At most one billing contact per client, enforced by the database rather
-- than by remembering to check.
CREATE UNIQUE INDEX idx_client_contacts_billing
    ON client_contacts(client_id) WHERE is_billing = 1;

-- One address per client, case-insensitively: a cc that is also the To is a
-- duplicate delivery, not a second recipient.
CREATE UNIQUE INDEX idx_client_contacts_email
    ON client_contacts(client_id, lower(email));
```

`ON DELETE CASCADE` is live: `db.rs` line 612 opens every connection with
`PRAGMA foreign_keys=ON`. `delete_client` therefore needs no new statement, and
a test pins that the contacts go with the client.

The partial index gives *at most one* billing contact. *At least one, whenever
there is any contact at all* is the data layer's job, enforced at every write by
one normalize step (below). Between them, "exactly one address is identifiable
as the billing recipient" (AC #2) is true by construction rather than by
convention.

### `Client.email` becomes a projection

`clients.email` is dropped. `get_client` and `list_clients` select it instead as

```sql
(SELECT c.email FROM client_contacts c
  WHERE c.client_id = clients.id AND c.is_billing = 1) AS email
```

so `models::Client` is **unchanged**, and with it: `format_client_list`'s em
dash, `client_summary`, `{{CLIENT_EMAIL}}` in `render_html.rs`,
`require_email`, the `Client` serde shape on the wire, `wc-client-form`'s Email
field, the `clients.json` parity fixture, and the invoice detail's nested
client. The whole read surface keeps working because the field kept its meaning:
*the address an invoice is sent to.*

`list_clients` stays one query — the subquery is correlated, not an N+1 — which
is what keeps the clients list cheap on 123 rows and keeps the "a screen may not
fan out one request per row" rule satisfiable.

Writes keep working the same way. `add_client(conn, name, email, …)` creates the
billing contact when `email` is `Some`; `ClientUpdate.email` is
`Option<Option<String>>` and still means leave / set / clear, now implemented as
upsert / delete of the billing row. One private writer,
`set_billing_email(conn, client_id, Option<&str>)`, is the only place that
translates, and `add_client`, `update_client` and the InvoiceShelf importer all
go through it.

### Why not a comma-separated column (AC #4)

Rejected, for five reasons, in order of weight:

1. **`require_email` and the Mailgun `To` need to know which address is which.**
   A comma list can only answer "the first one", which makes an accidental
   reordering — a person tidying a form field — a change to who gets billed,
   with no schema able to notice.
2. **It would discard the data this task exists to recover.** Every one of the
   187 Harvest rows carries a name and a title. A column of addresses has
   nowhere to put them, so the migration that was supposed to stop losing
   contact information would lose it again.
3. **No uniqueness and no per-address identity.** `lower(email)` uniqueness, a
   stable id to edit or delete one row, and "which address failed" in an API
   error all require rows. Splitting a string in six call sites means six
   implementations of trimming, empty segments and case.
4. **Every reader re-parses.** `render_html.rs`, `mailgun.rs`, the CLI, the TUI,
   the SPA and the importer would each need the same split, and the column is
   not shape-checked today (deliberately — see `wc-client-form`'s doc comment),
   so the garbage would multiply rather than be caught.
5. **SQLite makes the child table free.** Partial and expression indexes, cascade
   delete, and a `position` column for ordering are all already available, and
   this codebase already models a child collection this way
   (`invoice_line_items`).

### Why the flag, and not "the first row is the billing one"

Position-as-billing was considered and is genuinely simpler: no flag, no partial
index, no normalize step, and the invariant is structural. It was rejected
because ordering would then carry a meaning nothing on screen suggests —
sorting your contacts alphabetically would change who gets the invoice. The flag
is what the source system uses, what the export carries a (empty) column for,
and what makes "make this the billing contact" an explicit action rather than a
side effect of a drag.

## The four open questions, settled

### 1. What does `require_email` mean now?

**Unchanged, in both its code and its sentence.** `require_email(&client)` still
reads `client.email` — which is now *the billing contact's address* — and still
answers

```rust
NigelError::Conflict { code: "client_missing_email",
                       message: format!("client '{}' has no email", client.name) }
```

A client with contacts but none flagged for billing would read as "no email",
which is honest, but the state is unreachable: every write path normalizes.

- Writing a contact list with no `isBilling` row flags the first.
- Writing a list with two flagged rows is refused as `Invalid` ("exactly one
  contact can be the billing recipient") — the database's partial index would
  refuse it anyway, and a named refusal beats a constraint violation.
- Clearing the billing email (`ClientUpdate.email = Some(None)`) deletes that
  contact row and promotes the next by `position`, if there is one. A client with
  cc contacts and no billing contact cannot be produced.
- An empty contact list is allowed and means the client has no address at all —
  today's `email IS NULL`, and `send` refuses it exactly as it does now.

Send resolves both recipients in one call at the existing `Precheck` step, so
the ordering property `send_invoice_traced` already has — no network call before
the client's address is known — is unchanged:

```rust
// src/invoicing/send.rs
pub struct Recipients { pub to: String, pub cc: Vec<String> }

/// The `To` and every `Cc`, already formatted as header values. Wraps
/// `require_email`, so the refusal and its code are the ones that already ship.
pub fn require_recipients(conn: &Connection, client: &Client) -> Result<Recipients>;
```

Each entry is `mailgun::format_address(contact.name, contact.email)` — TASK-80's
function, which is why **PR-3a must land before this one**.

### 2. Does a cc recipient see the same payment link?

**Yes, and it is intended.** One render, one message, one `To` plus a `Cc` list;
every recipient gets the identical HTML body and the identical PDF, including
the Pay button, and any of them can pay.

The alternative — a second, button-less render for the cc list — was rejected on
two grounds. It creates an artifact that is not what was published, so "a
preview cannot disagree with what a client receives" stops being true for the
cc recipients; and it would not achieve anything, because the published page at
`public_base_url/i/{token}/` carries the same button and the token URL is
already forwardable by design (`token` is the only access control, and CLAUDE.md
says so).

Documented in `docs/invoicing.md` as a property of adding a contact: *anyone you
add can pay the invoice.*

### 3. Does the published page name one contact or none?

**One: the billing contact, through the existing `{{CLIENT_EMAIL}}`
placeholder.** No new placeholder, no list.

The page is a static object on a public URL. Printing an organisation's whole
contact list onto it publishes internal addresses to anyone the link reaches.
The billing address is already on there today and is the one the document is
addressed to.

`render_html.rs` and `templates/invoice.html` are therefore **not touched by
this task** — which also keeps it out of the way of Stream 2's TASK-78, which is
editing exactly those files.

### 4. How do existing single emails migrate?

In the migration, with nobody re-entering anything (AC #5, AC #7):

```sql
INSERT INTO client_contacts (client_id, email, is_billing, position)
SELECT id, TRIM(email), 1, 0
  FROM clients
 WHERE email IS NOT NULL AND TRIM(email) <> '';

ALTER TABLE clients DROP COLUMN email;
```

Every client that had an address gets one billing contact carrying it; a client
that had none gets no rows and behaves exactly as before. No name and no title,
because the column never had them.

**`DROP COLUMN` needs SQLite 3.35+.** The bundled SQLCipher is well past that,
but the migration must not be the place we find out otherwise: the plan's first
step probes `sqlite_version()`, and the documented fallback if it is too old is
`ALTER TABLE clients RENAME COLUMN email TO legacy_email` (3.25+), leaving the
data in place and unread. Either way the column stops being a second source of
truth, which is the requirement.

The 12 InvoiceShelf clients whose other Harvest contacts were discarded are not
recovered by this migration — that data is not in the database. What this change
buys is that the *next* person to add them has somewhere to put them.

## Data layer

```rust
// src/invoicing/clients.rs

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientContact {
    pub id: i64,
    pub client_id: i64,
    pub name: Option<String>,
    pub email: String,
    pub title: Option<String>,
    pub is_billing: bool,
    pub position: i64,
}

/// A contact as a caller supplies it — no id, because the write is a
/// whole-list replacement.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewContact {
    pub email: String,
    pub name: Option<String>,
    pub title: Option<String>,
    #[serde(default)]
    pub is_billing: bool,
}

pub fn list_contacts(conn: &Connection, client_id: i64) -> Result<Vec<ClientContact>>;

/// Replace a client's whole contact list, in one transaction.
///
/// The shape `update_invoice` uses for `items`: the caller sends the list it
/// wants, validation runs before anything is deleted, and positions are the
/// order they arrived in.
pub fn set_contacts(conn: &Connection, client_id: i64, contacts: &[NewContact]) -> Result<()>;

/// At least one email, no blank or duplicate address, at most one billing row.
pub fn validate_contacts(contacts: &[NewContact]) -> Result<()>;
```

`validate_contacts` refuses: a blank or whitespace-only email; two addresses
that differ only in case; more than one `is_billing`; and any value carrying CR
or LF (TASK-80's `validate_header_value` — these strings become mail headers).
It does **not** shape-check an address, for the reason the rest of the codebase
does not: `nigel client add --email` never has, and a form that refused what the
CLI accepts would make the surfaces disagree about what a client is.

`ClientSummary` gains `contacts: Vec<ClientContact>`, so `nigel client show` and
`GET /api/clients/{id}` both answer them in the round trip they already make.
`Client` does not, so the list stays one query and one row shape.

## Surfaces

### Mailgun

```rust
pub fn message_fields(
    envelope: &EmailEnvelope,   // TASK-80
    to: &str,
    cc: &[String],
    subject: &str,
    html: &str,
) -> Vec<(String, String)>;
```

A non-empty `cc` becomes one `cc` field whose value is the addresses joined with
`", "` — the form Mailgun documents for a recipient list — and an empty one emits
no field at all. `Mailer::send_invoice` gains the `cc: &[String]` parameter; the
fakes in `send.rs`'s and `routes/invoices.rs`'s test modules record it, which is
how AC #3 is asserted without a network call.

### CLI

```bash
nigel client add "Acme Co" --email ap@acme.test
nigel client edit 1 --email billing@acme.test          # billing contact only, cc rows untouched
nigel client edit 1 --contact "ap@acme.test:Ada Payne:AP" \
                    --contact "dana@acme.test:Dana Chen:Design Lead"
nigel client show 1                                    # prints the contact table
```

`--contact "email[:name[:title]]"` is repeatable and **replaces the whole
list**, first row is the billing contact — the same shape and the same
whole-list semantics as `invoice new --item "desc:qty:unit"`, split with the
same bounded splitter. `--email` and `--contact` in one command is refused by
clap's `conflicts_with`: one sets one field, the other replaces the collection,
and applying both would make the order they were applied in visible.

`client show` grows a contacts table (Email, Name, Title, and a `billing`
marker) between the label block and the invoice history.

### TUI

`c` on the client manager list opens a contacts sub-screen for the selected
client — `Screen::Contacts`, a list with `a` add, `e` edit, `d` delete, `b` make
billing, `Esc` back. The add/edit form is the existing `ClientForm` machinery
with three fields (Email, Name, Title).

A sub-screen rather than repeatable rows inside the client form: the client form
is a plain four-field stack whose every printable key types into a field, and
bolting a row editor into it would need the invoice form's whole `Ins`/`Del`
apparatus for a collection that is usually empty. The manager idiom — a list
with single-key actions — is already what this screen is.

The billing contact is marked in the list, and `b` is refused with a status-line
sentence on a client with one contact (it is already billing).

### Web

- `POST /api/clients` and `PATCH /api/clients/{id}` accept `contacts:
  [NewContact]` — absent leaves the list alone, present replaces it whole. That
  is exactly `items` on `PATCH /api/invoices/{number}`. `email` remains
  accepted and means the billing address alone; `email` and `contacts` in one
  body is a 400, mirroring the CLI's `conflicts_with`.
- `GET /api/clients/{id}` (`ClientDetail`) carries `contacts`.
- `wc-client-form` gains a contacts repeater under the Email field: one row per
  contact (email, name, title), an add and a remove button per row, and a radio
  column choosing the billing recipient. Rows reorder with the up/down buttons
  `wc-line-items` already established, for the same keyboard-accessibility
  reason. New preview states — empty, one contact, several, one with a long
  name — and the axe run comes with them.
- **Opening Edit fetches `GET /api/clients/{id}` first**, because the list row is
  a bare `Client` and does not carry contacts. One request for one row, on a
  deliberate click; the dialog shows a loading state while it lands. The
  alternative — putting `contacts` on every list row — was rejected because the
  list is the one screen that must stay one query and one cheap payload.
- `invoicing-errors.ts` gains the `Invalid` cases only if they arrive as coded
  conflicts; validation failures here are 400s, which that module already
  renders verbatim by design.

### The InvoiceShelf importer (AC #7)

`import_invoiceshelf.rs` line 63 inserts `INSERT INTO clients (name, email)`.
It becomes an insert of the client row followed by `set_billing_email(conn, id,
email)` — the same private writer `add_client` uses — so the one address it has
lands as a billing contact and nothing is lost. It keeps its raw insert rather
than moving to `add_client`, because import data legitimately carries duplicate
and blank names that `add_client` refuses.

## Files this touches

| File | Change |
|---|---|
| `src/migrations.rs` | `client_contacts` + two indexes + backfill + drop `clients.email` |
| `src/invoicing/clients.rs` | `ClientContact`, `NewContact`, `list_contacts`, `set_contacts`, `validate_contacts`, `set_billing_email`; `get_client`/`list_clients` projection; `add_client`/`update_client` rewrites; `ClientSummary.contacts` |
| `src/invoicing/send.rs` | `Recipients`, `require_recipients`, the `Precheck` call, the `Mailer` call site |
| `src/invoicing/gateway.rs` | `Mailer::send_invoice` gains `cc` |
| `src/invoicing/mailgun.rs` | `message_fields` gains `cc`; the `Mailer` impl |
| `src/invoicing/import_invoiceshelf.rs` | one address → one billing contact |
| `src/cli/mod.rs`, `src/cli/client.rs` | `--contact`, the conflict, `client show`'s table |
| `src/cli/client_manager.rs` | the contacts sub-screen |
| `src/server/routes/clients.rs` | `contacts` on create and patch, on the detail response |
| `web/apps/app/src/api/{types,client}.ts`, `screens/{clients,invoice-data}.ts` | `ClientContact`, detail-on-edit, patch shape |
| `web/packages/ui/src/components/wc-client-form.ts` | the repeater, previews, axe |
| `docs/invoicing.md`, `CLAUDE.md`, `README.md` | the clients section, a "Who receives an invoice" block, the design constraint |

## Out of scope

- Contacts on anything but a client: no per-invoice recipient override, no
  "send this one to just the AP contact".
- `Bcc`, and a copy of every invoice to yourself.
- Contact-level opt-out, bounce handling, or anything that reads Mailgun's
  events API.
- Backfilling the 12 InvoiceShelf clients' discarded Harvest contacts — a Harvest
  CSV importer is its own task if it is wanted.
- Any change to `templates/invoice.html` or the PDF: the page names the billing
  contact and only the billing contact.

## Coordination

- **Depends on PR-3a (TASK-80)** for `format_address` and
  `validate_header_value`, and for `message_fields` already taking an envelope
  rather than a bare `from`.
- **Follows PR-3b (TASK-74/73)** in the same worktree, so `Client` already
  carries `archived_at` and `list_clients` already takes a `ClientScope`.
- **Migration numbering** follows the merge-order rule in the 74/73 spec: this
  migration is written as the next free number on its base and renumbered to
  last on rebase if another stream's landed first. It is always after the
  archive migration, since they share a worktree and a PR order.
- **Stream 2's TASK-78** edits `render_html.rs` and `templates/invoice.html`.
  This task deliberately edits neither, so the two should not collide — but if
  78 changes what `{{CLIENT_EMAIL}}` renders, it is the same field this design
  redefines the source of, and the orchestrator should read both diffs together.

## Open questions for the orchestrator

1. **The flag versus position.** Recommended: `is_billing` with a partial unique
   index, matching Harvest and the export. Position-as-billing is simpler and
   was rejected because reordering would silently rebill. This is the one choice
   that is annoying to reverse later, so it is the one worth confirming.
2. **`clients.email` dropped rather than kept in sync.** Dropping is what makes
   drift impossible; it also means a database rolled back to an older binary
   loses every address. Nigel has no down-migrations, so this is already true of
   v4 and v5, but it is worth saying out loud.
3. **`Client` does not carry `contacts`.** The clients list stays one query and
   the edit dialog fetches the detail. Say the word if you would rather every
   list row carried its contacts and the dialog opened instantly.
4. **Every cc can pay.** Settled as intended, and documented. If you want a cc
   who cannot pay, that is a second render and a second published page, and it
   should be its own task with its own reasoning.
5. **`--contact "email:name:title"` colon syntax.** Matches `--item`. A name
   containing a colon will need the bounded splitter to behave (the last field
   takes the remainder); confirm you are happy with the same ergonomics
   `--item` has rather than repeated single-purpose flags.
