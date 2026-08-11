# Client lifecycle — delete on every surface, and archive

Tasks: TASK-74 and TASK-73 (epic TASK-86, Stream 3). One design because they are
one decision: *what happens to a client you have stopped working with.* Delete
is for the row entered by mistake; archive is for the client you finished with.
They are complements, and the surfaces they touch are the same three lists.

## Where things stand

**Delete already exists, on one surface.** `invoicing::clients::delete_blocker`
counts every invoice naming the client, of any status, and returns
`DeleteBlock::invoices("client", count)`; `delete_client` turns that into
`NigelError::Blocked`, whose `Display` is `Cannot delete: client has 8
invoices`. `DELETE /api/clients/{id}` exposes it and the SPA clients screen
offers it per row, rendering the count from `details.reason = has_invoices` and
offering a "Show those invoices" action pointing at `#/invoices?clientId=N`
(`invoicing-errors.ts`, `invoicingGuardrailAction`).

`nigel client` has `add`, `show`, `edit`, `list` and no `delete`
(`cli/mod.rs::ClientCommands`). `cli/client_manager.rs` has `a` add and `e` edit
and no delete; the reason is recorded in CLAUDE.md — a delete would be a no-op
for every client anyone has ever billed. The web answered that objection by
stating the refusal plainly, so the same answer works in a terminal.

**Archive does not exist at all.** `clients` has `id, name, email,
billing_address, notes, created_at` (migration v4) and `list_clients` is
`SELECT … ORDER BY name` with no filter.

## TASK-74: delete on the CLI and the TUI

No new guard, no new sentence. Both surfaces call `clients::delete_blocker` and
`clients::delete_client` and print what those already say.

### `nigel client delete <id> [--yes]`

Modelled on `nigel invoice void`, which is the codebase's destructive-command
shape (`cli/invoice.rs::void`/`confirm_void`):

1. Load the client (`get_client`) — an unknown id is the data layer's
   `Client not found: id 99`.
2. Ask `delete_blocker` **before** prompting. A blocked client never sees a
   confirmation prompt, because there is nothing to confirm: print the block's
   sentence and the pointer, and exit non-zero.
3. On a clear client, print what is about to go and ask.

```
$ nigel client delete 7
Delete client #7 Globex? This cannot be undone.
Delete it? [y/N] y
Deleted client 7: Globex

$ nigel client delete 1
Cannot delete: client has 8 invoices
Run `nigel client show 1` to see them.
```

- `--yes` skips the prompt, exactly as `invoice void --yes` does.
- Not a TTY and no `--yes`: `Refusing to delete client #7 without confirmation.
  Pass --yes.` — `confirm_void`'s wording with the noun changed, produced by a
  shared helper rather than a second copy of the logic (see the plan's Task 1).
- The pointer names `nigel client show <id>`, **not** `nigel invoice list
  --client <id>`: `cli/invoice.rs::list` calls `list_invoices(&conn, None, None)`
  and the clap `List` variant takes no flags, so that command does not exist.
  `client show` prints the client's whole invoice history, which is the same
  place the web guardrail points.

### The TUI client manager

`d` on the list, following `cli/account_manager.rs` line for line:

- `handle_list_key` gains `KeyCode::Char('d')`, which calls `delete_blocker`
  first. A block sets the status line to the block's `to_string()` and stays on
  the list — the account manager's `Cannot delete: account has {count} {noun}`
  path, and the reason the screen never offers a confirmation it will not honour.
- Otherwise `screen = Screen::ConfirmDelete`, an inline overlay on the list:
  `Delete 'Globex'? (y/n)`, footer `y=confirm  n=cancel`.
- `y` calls `delete_client`, reloads, and sets `Deleted client: Globex`.
  A failure sets the error's `to_string()` on the status line.

The refusal sentence is the same string in all three front ends because all
three call `Display` on the same `NigelError::Blocked`.

## TASK-73: archive

### The column

Migration: `ALTER TABLE clients ADD COLUMN archived_at TEXT` — a nullable
timestamp, not a boolean flag.

This is v5's own reasoning applied again. `voided_at` and `published_at` are
timestamps from which a state is derived; nothing in this schema stores a
boolean where a date would do, and "when did we stop billing them" is worth
having for free. `is_archived()` is `archived_at.is_some()`.

The migration probes `pragma_table_info` before the `ALTER`, exactly as v5 does,
so a replay is harmless. There is no backfill: every existing client is active.

**Migration numbering is a merge hazard.** See "Coordination" at the end.

### Data layer

```rust
// src/invoicing/clients.rs

/// Which clients a list wants. An enum rather than a bool so a call site says
/// what it means: `list_clients(&conn, ClientScope::Active)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientScope { Active, All }

pub fn list_clients(conn: &Connection, scope: ClientScope) -> Result<Vec<Client>>;

/// Archive a client. Idempotent: archiving an archived client leaves the
/// original timestamp alone rather than restamping it.
pub fn archive_client(conn: &Connection, id: i64, on: &str) -> Result<()>;
pub fn unarchive_client(conn: &Connection, id: i64) -> Result<()>;

/// The refusal a new invoice for an archived client gets.
pub fn ensure_client_active(conn: &Connection, id: i64) -> Result<()>;
```

- `list_clients` keeps `ORDER BY name` in both scopes and does **not** sink
  archived rows to the bottom. Every surface marks them instead; re-sorting a
  list depending on a filter is the kind of thing that makes a screen's row
  order feel unstable.
- `archive_client` takes `on: &str` (the caller's `today`), like
  `void_invoice(conn, id, today)` — the data layer never reads the clock.
- Both refuse an unknown id with the module's own `Client not found: id 99`.
- `Client` gains `pub archived_at: Option<String>`, serialized as `archivedAt`.

**Archiving is not deletion** (AC #4): the only statement is `UPDATE clients SET
archived_at = ?`. No invoice, payment or history row is touched, and
`delete_blocker` is not consulted — a client with 8 invoices is exactly the
client archive exists for.

### A new invoice for an archived client (AC #5)

`invoices::create_invoice` already calls `ensure_client_exists(conn, client_id)`
at line 101. It gains `ensure_client_active` beside it, which answers:

```rust
NigelError::Conflict {
    code: "client_archived",
    message: format!("client '{name}' is archived — unarchive it before invoicing"),
}
```

A `Conflict` rather than an `Invalid` for the reason `client_missing_email` is
one: it is a fact about the client record that a screen can act on, so over HTTP
it is a 409 naming the client and carrying a reason a button can be built from,
not a 500 or an opaque 400.

`update_invoice` needs no guard — `InvoiceUpdate` cannot change `client_id`.

Existing invoices are untouched: `list_invoices` uses `LEFT JOIN clients`
(line 731) and `ar_aging_detail` uses `JOIN clients` (line 832), and neither
learns about `archived_at`. That is AC #3, and it is satisfied by *not* changing
those queries — the test that pins it asserts an archived client's name still
appears in both.

### CLI

```bash
nigel client archive 7            # Archived client 7: Globex
nigel client unarchive 7          # Restored client 7: Globex
nigel client list                 # active only
nigel client list --all           # active and archived, marked
```

`format_client_list` grows an `Archived` column **only when the slice it is
given contains an archived client**. The default list never does, so
`nigel client list`'s output — asserted byte-for-byte in
`format_client_list_prints_the_columns_it_always_has` and captured in the
`clients.txt` parity fixture — is unchanged. The column carries the date, since
`archived_at` is one.

`client show` prints an `Archived:` line when the client is archived, in the
existing label block.

### TUI

The client manager's list gains two keys:

| Key | Action |
|---|---|
| `d` | delete (TASK-74) |
| `x` | archive / unarchive the selected client |
| `A` | show or hide archived rows (default: hidden) |

Footer: `a=add  e=edit  d=delete  x=archive  A=show archived  Esc=back  q=quit`,
with `x=unarchive` when the selected row is archived and `A=hide archived` when
they are shown.

`x` is a toggle on one row, which is the register browser's `f` (flag) idiom;
`A` is a filter over the list. Neither is behind a confirmation — archiving is
reversible in one keystroke, and the confirmations in this app are for things
that are not. An archived row renders with the `(archived)` suffix after the
name in `client_row`, inside the existing 26-character name budget's neighbour
column rather than by adding a column to a fixed-width row.

The dashboard hands the screen bare `KeyCode`s, so `Char('A')` (Shift+a) is
reachable and no `Ctrl` chord is needed — the same constraint that shaped the
invoice form's `Ins`/`Del` bindings.

### Web

- `GET /api/clients?includeArchived=true` — a `RawQuery`-style optional
  parameter; absent or `false` means active only. An unrecognised value is a
  400, matching the strictness the report routes already have about parameters.
- `POST /api/clients/{id}/archive` and `POST /api/clients/{id}/unarchive`, each
  answering the refreshed `Client`.

  A state transition gets its own verb rather than a `PATCH` field, because
  that is what `POST /api/invoices/{number}/void` already established, and
  because `ClientPatch`'s fields are all column values where this one is an
  event with a timestamp the server writes.
- The clients screen gains a "Show archived" toggle above the table and an
  Archive/Unarchive row action beside Edit and Delete. An archived row carries a
  muted `Archived` badge in the name cell. The toggle writes
  `#/clients?archived=1`, so a filtered list is a URL — the invoice list's rule.
- `invoicing-errors.ts` gains `client_archived`.
- `wc-manager-table` already renders `ManagerAction[]`, so the new action is
  data, not a component change. The badge is the one visual addition and belongs
  in `@nigel/ui` with a preview state, per the component-first workflow.

## What is deliberately not built

- **No cascade between the two.** Deleting an archived client is still refused
  if it has invoices; archiving is never suggested as an alternative in the
  delete refusal. Two operations, two sentences.
- **No bulk archive**, though the InvoiceShelf import brought in 23 clients at
  once. One row at a time on every surface; a bulk selection model is a screen
  concept none of these three lists has.
- **No archive on the invoice side.** An archived client's invoices stay in
  every list, report and total, and archiving changes no figure anywhere. If it
  did, it would be a way to make money disappear from the aging report.
- **No `archived` filter on `GET /api/invoices`.**

## Files this touches

| File | Change |
|---|---|
| `src/migrations.rs` | one migration: `archived_at` on `clients` |
| `src/models.rs` | `Client.archived_at` |
| `src/invoicing/clients.rs` | `ClientScope`, `list_clients` signature, `archive_client`, `unarchive_client`, `ensure_client_active`, `get_client`/`list_clients` column lists |
| `src/invoicing/invoices.rs` | `ensure_client_active` in `create_invoice` |
| `src/cli/mod.rs` | `ClientCommands::{Delete, Archive, Unarchive}`, `List { all: bool }` |
| `src/cli/client.rs` | `delete`, `archive`, `unarchive`, `list(all)`, `format_client_list` column, `show`'s archived line |
| `src/cli/client_manager.rs` | `Screen::ConfirmDelete`, the three keys, footer, row marker |
| `src/main.rs` | dispatch for the three new subcommands |
| `src/server/routes/clients.rs` | the query parameter, the two POST routes |
| `src/server/testutil.rs`, `src/cli/demo.rs` | seeds compile against the new `list_clients` signature; one seeded client becomes archived so the fixtures cover it |
| `web/apps/app/src/api/{types,client}.ts` | `archivedAt`, `getClients(includeArchived)`, `archiveClient`, `unarchiveClient` |
| `web/apps/app/src/screens/clients.ts` | toggle, action, badge |
| `web/packages/ui/src/components/wc-manager-table.ts` or a small badge component | the archived marker plus its preview and axe test |
| `web/apps/app/src/__fixtures__/invoicing/clients.json` | recaptured: `Client` gained a field |
| `docs/invoicing.md`, `CLAUDE.md`, `README.md` | commands, the client section, the Client Manager architecture bullet |

`clients.txt` in the fixtures is unchanged by design (see `format_client_list`
above); `clients.json` must be recaptured with
`cargo test --features serve capture_web_invoicing_fixtures -- --ignored`, and
the non-ignored guard test will fail until it is.

## Coordination: migration numbers

The latest migration is **v5**. Three of TASK-86's streams may add one:

- Stream 1, TASK-70 — a `UNIQUE` index on `clients.name`, if the decision goes
  that way.
- This PR (3b) — `archived_at` on `clients`.
- PR-3c, TASK-77 — the `client_contacts` table.

All three touch `clients`, and any two of them appended to `MIGRATIONS`
independently produce the same version number and a guaranteed conflict in the
same array. The resolution is mechanical (renumber, bump the description) but it
is silent if two databases have already run the same version number with
different SQL, which is a corruption class, not a merge annoyance.

**Proposed allocation, for the orchestrator to confirm before any of the three
starts:** Stream 1 takes **v6**, this PR takes **v7**, PR-3c takes **v8**. Each
stream hard-codes its number and its plan's first task asserts
`LATEST_VERSION == <n>`; whoever merges second rebases rather than renumbering
after the fact. If Stream 1 decides against the index (TASK-70's AC allows
"document the advisory behaviour as deliberate"), v6 goes unused rather than
being reclaimed — a gap in the sequence costs nothing and a reused number costs
a database.

## Open questions for the orchestrator

1. **`archived_at` timestamp versus an `archived` boolean.** Timestamp chosen for
   consistency with `voided_at`/`published_at`. It means the CLI's `--all` list
   shows a date, which is more information than "yes".
2. **TUI keys `x` and `A`.** Both are free on that screen. `x` was chosen over
   `r` (reads as "rename") and `h` (reads as "help"). Say if you want different
   letters — it is a match arm and a footer string.
3. **`nigel client list --all` versus `--include-archived`.** `--all` is short
   and unambiguous on a command whose only list is clients; the longer name
   matches the API parameter. Pick one and both sides will use it.
4. **Archive is not confirmed anywhere.** One keystroke on the TUI, one click on
   the web, no `--yes` on the CLI. It is reversible, so this seems right; delete
   is confirmed everywhere.
5. **Should a blocked delete suggest archiving?** Currently no — the refusal says
   what it always said. Adding "you may want to archive it instead" makes the
   pairing discoverable, at the cost of putting a second idea in a refusal.
