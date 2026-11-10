# Books as a package, and single-file backups — design

Covers TASK-163. A set of books becomes one thing with a fixed shape: a **books package**
(`Acme Consulting.nigel`), a directory that the CLI, the server and both desktop shells
open by path, and that macOS shows as a single document. Its portable form is one
**backup archive** (`Acme Consulting-20261023-101500.nigelbackup`) that holds the database
snapshot, the key file and the filed documents with a manifest and checksums, and that
restore verifies in full before it touches the current books.

TASK-161 (recovery codes) puts its key file at the place reserved here. TASK-154.12 (book
switching) and TASK-150.2 (the Mac app's books location) point at packages. Decision-12
needs each set of books to carry a stable identity and a reserved place for a sync log, so
3.0's end-to-end sync can adopt the package without a format change.

## Where things stand

A set of books is a **data directory** named by `data_dir` in
`~/.config/nigel/settings.json` (default `~/Documents/nigel/`). It holds `nigel.db` and,
while it is open, `nigel.db-wal` and `nigel.db-shm`; `documents/<id>/v<n>.pdf`;
`templates/invoice.html`; `snapshots/` (pre-import snapshots); `backups/`; `exports/`;
`previews/`; `tmp/uploads/` (the browser upload spool); and, after `nigel demo`, a nested
`demo/` data directory. Nothing in the directory says which books it holds: the company
name lives in the `metadata` table, inside the database, which may be encrypted.

`nigel backup` writes `nigel-<stamp>.db` with SQLite's online-backup API, preserving
encryption, and copies `documents/` beside it as `nigel-<stamp>.documents/`. `nigel
restore` validates the `.db`, takes a safety backup, copies the backup over `nigel.db`,
then swaps in the `.documents` folder when there is one. The two halves travel separately,
and the database is replaced before the documents are checked.

## Decisions

| Question | Decision | Rejected |
|---|---|---|
| Working form | A directory with a fixed layout and the `.nigel` extension | A single SQLite file (no room for documents, key file, sync log); running from inside an archive (SQLite needs random access to a real file) |
| What makes a directory a package | A `book.json` at its root. The extension is how Finder and file pickers recognise it, not what Nigel checks | Requiring the extension (breaks adopted directories and every non-Mac path) |
| Identity | A random UUID (`bookId`), stored in the database's `metadata` table and mirrored in `book.json` | Path-based identity (moves and renames break it); a hash of the company name (not stable, leaks the name) |
| Key file | `nigel.keys` at the package root, beside `nigel.db`; format owned by TASK-161 | Inside the database (cannot be read before the database is unlocked); in the OS keychain only (does not travel with the books) |
| Sync log | `sync/` reserved at the package root; 2.x never creates it | A table inside `nigel.db` (decision-12 asks for it beside the database, so it can be shipped and compacted on its own) |
| Converting existing data directories | **Adopt in place** on first open, after a backup: write `book.json`, record the `bookId`, move nothing. Renaming to `<Name>.nigel` is a separate, offered step | Copying into a new package (doubles disk use and leaves two live copies that can drift) |
| Default backup destination | Outside the package: `~/Documents/Nigel Backups/` | `backups/` inside the package (a backup that is lost with the books it backs up, and that bloats every copy and, later, every sync) |
| Archive container | A pax `tar` stream compressed with **zstd** | zip (per-entry deflate is slower and weaker; the central directory at the end gives nothing a streamed manifest does not); gzip (slower, worse ratio); a bespoke container (no recovery without Nigel) |
| Archive integrity | A header entry first, a manifest entry last with the size and SHA-256 of every entry, plus zstd's frame checksum | Per-entry CRC only (zip); trusting the container to report truncation |
| Archive encryption | None of its own in format v1: the archive is exactly as protected as the package. The header reserves a field for payload encryption | Encrypting the archive under the books password now (a second scheme to keep in step with TASK-161's envelope keys) |
| Restore | Unpack into staging inside the target package, verify everything, then swap item by item under a journal that a crash rolls back | Overwriting `nigel.db` in place (today's order: the database is replaced before the documents are known to be good) |

## The package

### Layout

```
Acme Consulting.nigel/
  book.json              identity and format version (unencrypted, no book data)
  nigel.db               the database (SQLCipher when encrypted)
  nigel.db-wal           SQLite's, while open
  nigel.db-shm           SQLite's, while open
  nigel.keys             wrapped data keys (TASK-161); present only for envelope-encrypted books
  documents/<id>/v<n>.pdf   every version of every filed document, immutable once written
  documents/<id>/...        signing records beside the versions they belong to
  templates/invoice.html    the custom invoice page, when there is one
  snapshots/             pre-import snapshots (automatic, per book)
  exports/               default destination of `nigel export` and report downloads
  previews/              output of `invoice preview` and `document preview`
  tmp/                   scratch: uploads/, restore staging, temporary snapshots
  .nigel.lock            advisory lock file (see Opening)
  sync/                  reserved: 3.0's end-to-end sync log
  outbox/                reserved: local delivery's queued packages (epic 114)
```

Paths stored in the database stay relative to the package root
(`document_versions.file_path = "documents/12/v1.pdf"`), so moving or renaming a package
changes nothing inside it.

**Reserved names.** Nigel owns every name in the layout above, plus `backups/` and
`demo/`, which appear only in adopted directories (see Adopting). A file or folder the
operator puts at the root under any other name is left alone: never deleted, never
renamed, carried into backups, and listed in `nigel books info` so it is not a surprise.
Names beginning with `.nigel` are reserved for future use by Nigel.

**Permissions.** As today: directories 0700, files 0600, applied by Nigel on create and on
restore whatever the archive or the source says. On Windows these calls are no-ops.

**Nesting is refused.** A package is never created inside another package or inside a
legacy data directory, and nothing that walks a package (backup, restore, info) descends
into a nested `.nigel` or a nested directory holding a `nigel.db`. This matters on macOS,
where `~/Documents/nigel` and `~/Documents/Nigel` are the same folder on a
case-insensitive volume.

### `book.json`

```json
{
  "format": "nigel-books",
  "formatVersion": 1,
  "minReaderVersion": 1,
  "bookId": "6f1c2a9e-3b7d-4e4a-9c52-0d8e5b1f7a34",
  "createdAt": "2026-10-23T17:15:00Z",
  "createdBy": "nigel 2.0.0"
}
```

It holds no book data: no company name, no profile, nothing a reader of an encrypted
package could not already infer from its size. The display name comes from the folder
name and, once unlocked, from `metadata.company_name`; the recent-books registry
(TASK-154.12) caches what it needs to show a locked set of books.

`book.json` is written atomically: a temporary file in `tmp/`, flushed, renamed over the
old one, with the directory flushed on Unix, the same way `settings.json` is written.

### Identity

`bookId` is a random (v4) UUID, generated once when the books are created or adopted, and
written in two places in one step: `metadata.book_id` inside the database first, then
`book.json`.

- **The database is the authority.** The copy in `book.json` exists so a host can identify
  a package without unlocking it. On open, if the two disagree (someone dropped a different
  `nigel.db` into the package), the database wins, `book.json` is rewritten, and Nigel says
  so. A `book.json` that is unreadable but whose database carries an id is rewritten from
  the database the same way.
- **Restoring keeps the identity.** An archive restored over the current books, or into a
  new place, carries its `bookId`: it is the same books.
- **A copy shares the identity.** Duplicating a package in Finder yields two packages with
  the same `bookId`. In 2.x that is harmless. The registry notices two paths with one id and
  offers to make the copy separate books; `nigel books new-identity <path>` does the same
  from the terminal by writing a fresh id to both places. 3.0's sync treats a shared id as
  one lineage and decides forks under its own spec.

The id is not a secret and authorizes nothing. It names a lineage of books so a registry,
a backup directory or a future sync relay can tell sets of books apart.

### The key file

`nigel.keys` sits beside `nigel.db`. TASK-161 owns its format: a versioned list of wrapped
copies of the data key (one under the password, one under the recovery code, room for one
per future member under decision-12). This spec fixes only its name and place, and that it
travels with the books: backups carry it, restore puts it back, and a package's encryption
state is read from the database header as today. A package with an envelope-encrypted
database and no readable key file is reported as such by TASK-161's rules and is never
"repaired" into a new or unencrypted database.

### The sync log

`sync/` is reserved and 2.x never creates or reads it. 3.0 decides its contents. Because
it is a sibling of `nigel.db` rather than a table inside it, sync can append to it, ship it
and compact it without touching the database file, and a backup carries it like any other
per-book state.

### Versioning and forward compatibility

`book.json` carries two integers:

- `formatVersion` — the layout version the package was last written under. It starts at 1.
- `minReaderVersion` — the lowest layout version a Nigel must understand to **write** to
  this package safely.

A Nigel that understands layout versions up to N:

| Package | Behaviour |
|---|---|
| `formatVersion ≤ N` | Opens normally |
| `formatVersion > N`, `minReaderVersion ≤ N` | Opens normally; the newer layout's additions are entries this Nigel leaves alone and carries into backups |
| `minReaderVersion > N` | Refuses to open: "These books were last written by a newer Nigel. Update Nigel to open them." Nothing is written, not even the lock file |

An additive change (a new optional folder) raises `formatVersion` and leaves
`minReaderVersion` alone. A change an older writer would corrupt raises both. The case
this is built for is 3.0: once sync is on for a set of books, every write has to be
logged, so a sync-enabled package raises `minReaderVersion` to keep a 2.x writer out,
while a package that never turned sync on stays readable by 2.x.

The layout version is independent of the database's schema version. Migrations keep
running in the dispatch pre-flight as today, and a database newer than the binary is
refused as it is today.

## Extension and document types

- **`.nigel`** names a books package. **`.nigelbackup`** names a backup archive.
- On **macOS**, the app exports two uniform type identifiers in its `Info.plist`:

  | UTI | Conforms to | Extension | Role |
  |---|---|---|---|
  | `com.madebyraygun.nigel.books` | `com.apple.package`, `public.composite-content` | `nigel` | Editor, `LSHandlerRank` Owner, `LSTypeIsPackage` |
  | `com.madebyraygun.nigel.backup` | `public.data`, `public.archive` | `nigelbackup` | Viewer, `LSHandlerRank` Owner |

  Conforming to `com.apple.package` is what makes Finder show the folder as one file;
  double-clicking a package opens it in Nigel, and double-clicking an archive opens the
  restore flow. Finder applies this only on a Mac where the app is installed: on a
  CLI-only Mac, a package is a folder named `Acme Consulting.nigel`, and that is fine.
- **Show in Finder** reveals the package selected in its parent folder (`open -R`, or
  `NSWorkspace.activateFileViewerSelecting`). Opening it would launch Nigel on itself.
  The Tauri `reveal_data_dir` command changes to the select form on macOS.
- On **Windows and Linux** a package is a folder whose name ends in `.nigel`. Folders cannot
  carry a file association, so books are opened from the app's "Open books…" folder picker
  or by path. The Tauri bundle registers `.nigelbackup` (Windows registry association,
  Linux desktop entry with a shared-mime-info type `application/x-nigel-backup`) so a
  double-clicked archive opens the restore flow.

## Opening a package

### One type, every front end

A new `nigel_core::books` module owns the package. `Books::open(path)`:

1. Resolves the path. A path whose `book.json` is present is a package. A path holding a
   `nigel.db` and no `book.json` is a legacy data directory and goes through adoption. A
   path that is neither is refused: opening never creates books.
2. Recovers an interrupted restore if `tmp/restore.journal` is present (see Restore).
3. Reads `book.json` and applies the version rules.
4. Takes a shared advisory lock on `.nigel.lock` for as long as the books are open.
5. Hands back accessors — `db_path()`, `key_file()`, `documents_dir()`,
   `templates_dir()`, `snapshots_dir()`, `exports_dir()`, `previews_dir()`, `tmp_dir()`,
   `book_id()` — so no caller joins path strings onto a directory again.

`setup::run` creates books through `Books::create(path, …)`, which builds the layout,
writes the database (password global set first, as today), records the `bookId` and
writes `book.json` last, so a crash part-way leaves a directory without `book.json`,
which open refuses rather than half-adopts.

### How `data_dir` maps to a package

`data_dir` keeps its name in `settings.json` and its role: it is the path of the current
books, which is now a package path. Every existing `data_dir.join("nigel.db")` remains
correct, and call sites move onto the `Books` accessors as they are touched. No settings
key is renamed, so an older `settings.json` needs no migration.

| Front end | How it picks the books |
|---|---|
| CLI | `--books <path>` on any command, or `NIGEL_BOOKS`, overrides `data_dir` for that invocation; `nigel load <path>` changes `data_dir`; `nigel init [--books <path>]` creates (`--data-dir` stays as an alias) |
| Server | `nigel serve --books <path>`, else `data_dir`; `POST /api/settings/data-dir` accepts a package path (or a legacy directory, adopted after unlock) and rebinds as today |
| Tauri desktop | "Open books…" picks a folder; on Windows and Linux any folder holding `book.json` or `nigel.db` is accepted |
| Swift shell | `NSOpenPanel` filtered to the books UTI (with directories allowed, for unconverted folders), Finder double-click through `application(_:open:)`, and an app-scoped security-scoped bookmark **to the package**; `NigelHost::new(package_path)` |

`--books` lets one cron job back up several sets of books without rewriting
`settings.json` between them.

**Default locations.** `nigel init` with no path creates `~/Documents/My Books.nigel`.
Onboarding, which knows the company name, proposes `~/Documents/<Company Name>.nigel`. The
demo moves out of the user's books into `<config_dir>/Demo.nigel`, which is regenerable and
no longer nested inside real books. In the sandboxed store build the save panel is
pre-pointed at `~/Documents`, superseding the `~/Documents/Nigel` folder suggestion in the
Swift shell spec, because the package itself is now the unit the shell bookmarks.

### The advisory lock

Every process that opens the books holds a shared lock on `.nigel.lock` (std's
`File::lock_shared`; the lock file is created on first open) while it has them open: a CLI
command for its run, `nigel serve` and the desktop apps for as long as the books are bound.
Operations that move or replace the package's contents — restore's swap, `nigel books
rename`, `nigel books new-identity` — drop their own process's shared lock, take the lock
exclusively **without waiting**, and refuse with "These books are open in
another Nigel (the desktop app or `nigel serve`). Close it and try again." SQLite's own
locking still governs concurrent reads and writes, so a cron backup while the desktop app
is open keeps working as it does today. Inside one process, the server's `db_gate` write
lock still serialises a switch or restore against its own requests; the file lock only
keeps other processes out. Advisory locks are best effort on network file systems, and the
docs say so.

## Adopting an existing data directory

A legacy data directory is adopted **in place** the first time a package-aware Nigel opens
it, once it is unlocked: in the CLI's dispatch pre-flight after the password prompt, and
in the server and desktop apps after unlock.

1. **Backup first.** A `.nigelbackup` of the directory goes to the backup destination as
   `<Name>-pre-adoption-<stamp>.nigelbackup`. If it cannot be written, adoption stops and the
   books are not opened for writing.
2. **Identity.** `metadata.book_id` is written if absent (an id already there, from a
   restored archive, is kept), then `book.json`.
3. **Nothing moves.** `documents/`, `templates/`, `snapshots/`, `exports/`, `previews/` and
   `tmp/` are already where the layout puts them. `backups/` and `demo/` stay as they are:
   they are reserved legacy names, never written by Nigel again, excluded from new backups,
   and still restorable from (`nigel restore <dir>/backups/nigel-<stamp>.db`).
4. **`settings.json` is unchanged.** `data_dir` still names the same path, so scripts and
   cron jobs that name it keep working.

The adopted directory is a package by every rule Nigel checks. It lacks only the
extension, which Finder needs to show it as a single file. **Renaming** is offered, not
forced:

- `nigel books rename [<new name>]` renames the package within its parent folder, to
  `<Company Name>.nigel` by default, under the exclusive lock, as one `rename(2)`. It then
  updates `data_dir` in `settings.json` (when it named the old path) and the recent-books
  registry. It refuses a target that exists and a move across volumes (back up and
  restore instead).
- The desktop apps offer the same rename once, from Settings > Location, with "Not now"
  remembered per book.
- The sandboxed store app cannot rename a folder whose parent it has no access to. It
  explains how to add `.nigel` in Finder instead. Security-scoped bookmarks follow a rename
  on the same volume, so the books reopen without a prompt afterwards.

## The backup archive

### Container and compression

A `.nigelbackup` file is a **pax-format tar stream compressed with zstd** (level 3, frame
content checksum on, single frame). The reasons:

- **Streams both ways.** The writer never seeks, so `nigel backup --output -` can pipe into
  any uploader. The reader verifies in one forward pass, so a restore can reject an archive
  before it has written anything outside its staging folder.
- **Fits the data.** An unencrypted SQLite database compresses several-fold under zstd.
  An encrypted database and most PDFs are already incompressible, and zstd passes them
  through at close to copy speed instead of burning CPU on them as deflate would.
- **Recoverable without Nigel.** `zstd -d < file.nigelbackup | tar -x` unpacks it with
  stock tools, which is what an operator needs on the day Nigel itself will not start.
- **Mature crates.** `tar` and `zstd` (bundled libzstd, which builds for every target the
  xcframework and Tauri need) are widely used. `sha2` is already a dependency.

Nigel writes the tar headers itself rather than copying file metadata: mode 0600/0700,
uid and gid 0, no user or group names, and the file's modification time. A backup does not
record the operator's local account name.

### Contents and order

```
nigelbackup.json                 header — always the first entry
books/book.json
books/nigel.keys                 when present
books/nigel.db                   an online-backup snapshot, never the live file
books/documents/12/v1.pdf
books/templates/invoice.html
books/<other entries>            anything else at the package root that is not excluded
manifest.json                    always the last entry
```

Package contents sit under `books/` so archive metadata can never collide with a package
file of the same name.

**Excluded:** the live `nigel.db`, `-wal`, `-shm` and `-journal` (the snapshot replaces
them), `.nigel.lock`, `tmp/`, `snapshots/`, `exports/`, `previews/`, the legacy `backups/`
and `demo/`, OS litter (`.DS_Store`, `._*`, `Thumbs.db`, `desktop.ini`), and any nested
package. Everything else is included, which is what lets a package written by a newer
Nigel, or a future `sync/` or `outbox/`, travel without this writer knowing about it. The
backup's summary names any included entry outside the known layout.

**Links.** A `documents/` or `templates/` that is itself a symlink is followed, as the
documents copy does today. Any other symlink is skipped with a warning. The archive holds
regular files and directories only.

### Header

```json
{
  "format": "nigel-backup",
  "formatVersion": 1,
  "minReaderVersion": 1,
  "createdAt": "2026-10-23T17:15:00Z",
  "createdBy": "nigel 2.0.0",
  "bookId": "6f1c2a9e-3b7d-4e4a-9c52-0d8e5b1f7a34",
  "packageFormatVersion": 1,
  "schemaVersion": 16,
  "encrypted": true,
  "keyFile": true,
  "payloadEncryption": "none"
}
```

The header is readable without a password and holds no book data. It is enough to list
archives by book, to refuse an archive this Nigel cannot read (`minReaderVersion`, or a
`schemaVersion` above the binary's latest migration) before unpacking anything, and to
know whether a password will be needed. `payloadEncryption` is always `"none"` in format
v1; it is the field a later encrypted variant uses.

### Manifest

```json
{
  "headerSha256": "…",
  "entries": [
    { "path": "books/book.json", "size": 196, "sha256": "…" },
    { "path": "books/nigel.db", "size": 1835008, "sha256": "…" },
    { "path": "books/documents/12/v1.pdf", "size": 48211, "sha256": "…" }
  ]
}
```

The writer hashes each entry as it streams it and writes the manifest last. The reader
hashes each entry as it unpacks it and, at the end, requires all of these:

- the manifest is present and is the final entry, followed by tar's end-of-archive blocks
  and the end of the zstd frame, with no trailing bytes;
- the zstd frame checksum matches;
- the header's hash matches `headerSha256`;
- every manifest entry was seen once, with the stated size and hash, and no entry was seen
  that the manifest does not list.

A truncated archive fails the first test, which is how a partial file is told from a whole
one even when it was streamed to stdout and never renamed.

The SHA-256 sums detect corruption, not tampering: anyone who can rewrite the file can
rewrite the manifest. For encrypted books SQLCipher's per-page HMAC still rejects a
modified database. Signing archives is out of scope.

### Writing

`nigel backup [--books <path>] [--output <file>|-] [--keep <n>]`:

1. Opens the books (shared lock) and unlocks them. As today, an encrypted database needs
   its password: the terminal prompts, and scheduled jobs set `NIGEL_DB_PASSWORD`.
2. Snapshots the database with the online-backup API into a temporary file **beside the
   output** (`<output>.db.partial`), so a backup works from a package on a read-only
   volume and the snapshot is on the same disk as the result. The snapshot keeps the
   source's encryption; for an encrypted package there is no plaintext copy at any point.
3. Reads `nigel.keys` after the snapshot. Under envelope encryption every version of the key
   file unwraps the same data key, so a password change during the backup cannot produce
   a pair that does not open. Rotating the data key itself (3.0's revocation) will have
   to take the exclusive lock.
4. Streams header, package entries and manifest into `<output>.partial`, flushes it, and
   renames it to `<output>`. The temporary snapshot is deleted on success and on failure.
5. With `--keep <n>`, deletes the oldest archives in the destination folder whose header
   carries this `bookId`, keeping the newest `n`. Archives for other books and files that
   are not archives are never touched.

Documents are archived after the snapshot. A version's file is written before its row
commits and is never rewritten or deleted afterwards, so every file the snapshot refers
to is on disk; a document filed during the backup lands in the archive without a row,
which is harmless.

The default output is `<backup destination>/<Name>-<YYYYMMDD-HHMMSS>.nigelbackup`, where
`<Name>` is the package name without its extension. The backup destination is
`backup_dir` in `settings.json`, defaulting to `~/Documents/Nigel Backups/`. A destination
inside the package is refused, generalising today's refusal of a destination inside
`documents/`. In the store build the destination is a folder the operator picks the first
time, held by its own security-scoped bookmark.

`nigel backup verify <file>` runs the reader's checks without restoring anything and
exits non-zero on any failure, so a scheduled job can check what it wrote. Without a
password it checks the container (header, manifest, every hash); given one, by prompt or
`NIGEL_DB_PASSWORD`, it also opens the database and runs `integrity_check`.

Memory stays bounded by a fixed buffer whatever the size of the books. Disk use during a
backup is one database-sized temporary snapshot plus the archive.

### Encryption at rest

The archive is exactly as protected as the package it came from:

| Part | Unencrypted books | Encrypted books |
|---|---|---|
| `nigel.db` | Plaintext | SQLCipher, same key as the live database |
| `nigel.keys` | Absent | Wrapped data keys; safe to store, per TASK-161 |
| `documents/`, `templates/` | Plaintext | **Plaintext**, as they sit in the package |
| Header, manifest | No book data | No book data (sizes and file count only) |

This keeps the existing rule that filed PDFs are not encrypted, and the docs keep saying
so next to every backup instruction. Encrypting the payload properly belongs after
TASK-161: with a data key in place, an archive of encrypted books could carry the key file
in the clear and encrypt everything after it under a key derived from the data key, so the
same password or recovery code opens both. The header's `payloadEncryption` field is the
room left for that variant, and the decision whether to do it is an open question below.

## Restore

### What restore guarantees

- Nothing in the current books changes until the archive has been read to the end and
  every check has passed, the restored database has opened with its key and passed
  `PRAGMA integrity_check`, and a safety backup of the current books exists.
- A damaged, partial, foreign or too-new archive is refused with a message that names
  what failed: the entry, the check, or the version.
- A crash or power loss during the swap leaves books that the next open rolls back to
  exactly what was there before.

### Steps

`nigel restore <archive> [--books <path> | --to <new path>] [--yes]`:

1. **Read the header.** Refuse a file that is neither a `.nigelbackup` (zstd magic and a
   `nigelbackup.json` first entry) nor a legacy `.db` (see Legacy backups). Refuse a
   `minReaderVersion` or `schemaVersion` this Nigel cannot handle.
2. **Choose the target.** By default, the current books, which are replaced. With `--to`,
   a new package at that path, which must not exist. With no current books, `--to` is
   required; the desktop apps ask for the place with a save panel.
3. **Stage.** Unpack into `<target>/tmp/restore-<stamp>/` (for a new target, the target
   directory is created holding only `tmp/`). Staging inside the target keeps the swap on
   one volume and inside the one location a sandboxed app has access to. Entry paths are
   checked as they arrive: relative, normalised, under `books/`, no `..`, no absolute
   paths, regular files and directories only. Anything else is refused as a damaged
   archive. Files are created 0600 and directories 0700.
4. **Verify.** Apply every manifest check. Open the staged database with the password (or,
   with TASK-161, the key unwrapped from the staged `nigel.keys`, which also proves the
   two belong together), run `integrity_check`, confirm the core tables, and run
   migrations on the staged copy, so a migration failure also leaves the current books
   alone. Compare each staged document against its `document_versions.checksum`; a
   mismatch or a missing file is reported but does not block the restore, because it
   describes the books as they were backed up, and the existing `file_missing` handling
   covers it afterwards.
5. **Confirm.** Show what will be replaced and with what. When the archive's `bookId`
   differs from the current books', say so plainly ("This backup is a different set of
   books: Globex, not Acme Consulting"). `--yes` answers for a script; a different `bookId`
   also needs `--replace-different-books`.
6. **Safety backup.** Write `<Name>-pre-restore-<stamp>.nigelbackup` of the current books to
   the backup destination. If that fails, stop.
7. **Swap.** Take the exclusive lock. Write `tmp/restore.journal` with the list of items
   and the phase `swapping`, flushed. For each item in a fixed order (`nigel.db` with its
   sidecars, `nigel.keys`, `documents/`, `templates/`, other entries, `book.json` last), move
   the live item to `tmp/replaced-<stamp>/` and the staged item into place, each as one
   same-volume rename. Mark the journal `done`, delete the replaced items and the staging
   folder, delete the journal.
8. **Rebind.** A running server or desktop app that did the restore reopens the books; a
   restored encrypted package comes up locked, as a data-directory switch does today.

A `documents/` that is a symlink is swapped at the link's target, staged beside the
target, so the link survives as it does today.

### Recovery

`Books::open` checks for `tmp/restore.journal` before anything else, holding the lock
exclusively while it does:

- Phase `swapping`: roll back. Every item still in `tmp/replaced-<stamp>/` goes back to
  its place, and anything staged that had already moved in goes to a discard folder that
  is then deleted. Rolling back rather than forward is deliberate: the operator was never
  told the restore finished, and the current books are the state they last saw. The
  message says the restore did not complete and can be run again.
- Phase `done`: only cleanup remained, so it finishes it.
- Staging folders under `tmp/` with no journal are leftovers from a restore that never
  reached the swap, and are removed.

### Legacy backups

`nigel restore` still takes a `.db` file. It is recognised by content (an SQLite header,
or a file that opens with the supplied password), not by extension. It goes through the
same staging, verification, safety backup and journaled swap:

- the `.db` is copied into staging with the online-backup API, and its `metadata.book_id`
  is set to the current books' id when it has none (it is a backup of these books);
- the sibling `<name>.documents/` folder, when present, is staged as `documents/`. When it
  is absent, the current `documents/` is kept, as today, so restoring a database-only
  backup never deletes PDFs;
- `templates/` and `nigel.keys` are kept from the current books. A legacy database that is
  password-keyed while the current books use TASK-161's data key goes through TASK-161's
  one-time migration in staging, before the swap.

Restoring a legacy `.db` into a new place (`--to`) creates a package around it with a new
`bookId`.

## Automated backups

The README's "Automated backups" section and `docs/commands.md` move to the archive, and
gain worked examples for both schedulers. Each reads the password from a secret store in a
wrapper script, never from the plist or crontab:

```bash
#!/bin/sh
# ~/bin/nigel-backup
NIGEL_DB_PASSWORD="$(security find-generic-password -s nigel-db -w)" \
  exec nigel backup --books "$HOME/Documents/Acme Consulting.nigel" --keep 30
```

```xml
<!-- ~/Library/LaunchAgents/works.nigel.backup.plist: daily at 02:30 -->
<key>ProgramArguments</key><array><string>/Users/you/bin/nigel-backup</string></array>
<key>StartCalendarInterval</key><dict><key>Hour</key><integer>2</integer><key>Minute</key><integer>30</integer></dict>
```

```cron
30 2 * * *  $HOME/bin/nigel-backup && nigel backup verify "$(ls -t "$HOME/Documents/Nigel Backups/"*.nigelbackup | head -1)"
```

The examples use the fixture books' name and an invented schedule. The desktop apps' own
"Back up now" uses the same writer and destination.

## Windows and Linux

- A package is a folder. Everything in this spec works there unchanged except Finder's
  single-file display and double-click on a package.
- Directory renames on Windows fail while any handle is open inside the folder. The
  exclusive lock keeps other Nigel processes out; an indexer or antivirus scanner can still
  hold a handle, so each rename in the swap is retried briefly and then fails. A failed
  rename rolls the journal back, never leaving the books half-swapped.
- The lock uses `LockFileEx` through std on Windows and `flock` on Linux and macOS.
- Paths stay short: the deepest path inside a package is
  `documents/<id>/v<n>.pdf` or a signing record beside it, well inside Windows' legacy
  260-character limit for any reasonable parent folder. Nigel does not opt in to long paths.
- Permissions calls are no-ops on Windows, as today.

## Failure modes

| Situation | What happens |
|---|---|
| Path is neither a package nor a legacy directory | Refused; nothing is created. The registry offers to forget a missing or moved package, never to recreate it |
| `book.json` from a newer Nigel (`minReaderVersion` too high) | Refused with "update Nigel"; nothing written |
| `book.json` unreadable | Rewritten from `metadata.book_id` when the database has one; otherwise the directory is treated as legacy and adopted, with a backup first |
| `book.json` and database disagree on `bookId` | The database wins; `book.json` is rewritten; a warning is printed |
| Two packages share a `bookId` | Both open; the registry offers to make one separate; `nigel books new-identity` |
| Pre-adoption or pre-restore backup cannot be written | Adoption or restore stops; the books are not changed |
| Disk full while backing up | The `.partial` archive and temporary snapshot are removed; no file carries the final name |
| Disk full while staging a restore | Staging is removed; the current books are untouched |
| Archive truncated, corrupt, or with a bad checksum | Refused, naming the entry and the check; current books untouched |
| Archive entry outside `books/`, with `..`, absolute, or a link or device | Refused as damaged |
| Wrong password, or key file that does not unwrap the archive's database | Refused before the swap |
| Archive or database from a newer Nigel | Refused before staging (header) or before the swap (database) |
| Crash during the swap | Next open rolls back from the journal |
| Restore or rename while another Nigel has the books open | Refused: "close it and try again" |
| Package on a read-only volume | Opens for reading where SQLite allows, without the lock (nothing can write to it); adoption, restore and rename are refused; backup works, because its temporary snapshot is written beside the output |
| Package in iCloud Drive with "Optimize Mac Storage", or in another sync folder | Evicted files fail to open with the OS error; the docs advise keeping live books out of sync folders, as for any SQLite database |
| Backup destination inside the package | Refused |
| Package created inside another package or legacy directory | Refused |
| Unknown entries at the package root | Left alone, carried into backups, named in the backup summary and `nigel books info` |

## Testing

All Rust tests run serially (`--test-threads=1`) as the password global requires. Fixture
books are the Acme Consulting and Globex cast with invented figures.

- **`books` module.** Create builds the layout and writes the id to both places; open
  accepts packages and refuses everything else; each row of the version table; id
  mismatch and unreadable `book.json`; nesting refused; shared lock held while open and
  exclusive operations refused while another handle holds it.
- **Adoption.** A legacy directory with documents, templates, snapshots, `backups/` and
  `demo/` is adopted in place: pre-adoption archive written and verifiable, `book.json`
  and `metadata.book_id` agree, every other file byte-identical, `settings.json` unchanged.
  Adoption stops when the backup destination is unwritable. Rename moves the package,
  updates `settings.json`, and refuses an existing target.
- **Archive round trip.** Unencrypted and encrypted packages (password-keyed now, and the
  envelope-keyed case once TASK-161 lands) back up and restore to byte-identical
  documents, templates and key file and an equivalent database; unknown root entries
  survive; excluded entries are absent; headers carry no local user name.
- **Corrupt archives.** For a reference archive: truncation at a sample of offsets
  (inside the header, inside a document, inside the database, just before the manifest,
  inside the manifest), a flipped byte in each entry, a missing manifest, an unlisted
  extra entry, a duplicated entry, a `..` path, an absolute path, a symlink entry, trailing
  garbage, a newer `minReaderVersion`, a newer `schemaVersion`, and a wrong password. Each
  is refused and the current books are byte-identical afterwards.
- **Swap and recovery.** A test-only fault hook fails the swap after each step in turn;
  the next open rolls back to the original books every time, and a `done` journal is
  cleaned up.
- **Legacy restore.** `.db` alone keeps current documents; `.db` with `.documents/`
  replaces them; the id is carried over; a legacy `.db` restored with `--to` gets a new id.
- **Format stability.** A small Acme archive in format v1 is committed as a test fixture,
  and every later Nigel must verify and restore it. A change that cannot is a format
  change and raises the version.
- **Front ends.** CLI integration tests for `--books`, `NIGEL_BOOKS`, `backup`, `backup
  verify`, `--keep`, `restore` and `books rename|info|new-identity`; server route tests for
  switching to a package and to a legacy directory; the macOS CI runner checks the
  shell's `Info.plist` declares both UTIs (decision-3: it compiles and tests, it does not
  sign). Finder behaviour and double-click are checked by hand on a Mac, on the
  operator's Mac track.

## Scope

In: the package layout, identity, versioning, adoption and rename, the archive format,
backup, verify, restore (new and legacy), the lock, the CLI, server and desktop open paths,
the UTI declarations, and the docs.

Out: the key file's format and recovery flow (TASK-161); encrypting the archive payload
or documents at rest (an open question below, and a follow-up if accepted); the sync log's
contents (3.0); the outbox's contents (epic 114); a restore screen in the web UI (the
desktop apps call the same code through their existing commands); signing archives.

## Implementation tasks

Proposed for filing once this spec is approved, in dependency order. Sizes: **S** about a
day, **M** two to four days, **L** about a week, each including its tests and docs.

1. **The `books` module (M).** `book.json` read/write, version rules, `bookId` in
   `metadata` and `book.json`, `Books::open` and `Books::create`, accessors, the advisory
   lock, nesting checks; `setup::run` creates packages. This is the layout's migration
   slot in decision-11's one-at-a-time order (it adds a metadata key, not a schema change).
2. **The archive writer and reader (M).** tar+zstd, header, manifest, streaming hashes,
   every reader check, `nigel backup` writing the new format, `nigel backup verify`,
   `--keep`, `backup_dir`, the committed v1 fixture.
3. **Adoption and rename (S).** Adopt-on-open with the pre-adoption archive;
   `nigel books rename|info|new-identity`; demo to `<config_dir>/Demo.nigel`. Depends on 1
   and 2.
4. **Restore (L).** Staging, verification, safety archive, the journaled swap and its
   recovery, `--to`, `--yes`, different-books confirmation, legacy `.db` (+ `.documents`)
   restore through the same path. Depends on 2.
5. **Open by path everywhere (M).** `--books` and `NIGEL_BOOKS`, `nigel init`/`load`
   on packages, `nigel serve --books`, the data-dir route accepting packages and adopting
   legacy directories, the Tauri open-books picker, reveal-selects on macOS, and the
   `.nigelbackup` file association on Windows and Linux. Depends on 1 and 3.
6. **macOS document types (S, operator's Mac track).** Both UTIs and document types in the
   Swift shell's `Info.plist`, `application(_:open:)` for packages and archives, the
   bookmark to the package, the backup-folder bookmark in the store build. Folds into
   TASK-150.2's acceptance criteria. Depends on 5.
7. **Docs (S).** README (Quick Start, Features, Automated backups with the launchd and cron
   examples), `docs/architecture.md` (the package replaces the data-directory tree),
   `docs/design-constraints.md` (package, archive, restore and lock rules replace the
   `.documents` rule), `docs/commands.md`.

TASK-161 can start once task 1 has landed, since it needs only the key file's place and
the `Books` accessor.

## Open questions for the operator

1. **Backup destination.** Default to `~/Documents/Nigel Backups/` outside the package (this
   spec), or keep `backups/` inside it as today? Outside survives losing the package and
   keeps copies and future syncs small; inside needs no second folder grant in the store
   build.
2. **Encrypting the archive and documents.** Should encrypted books' archives also encrypt
   the PDFs and templates (payload encryption keyed from TASK-161's data key), and should
   `documents/` be encrypted at rest in the package too? If yes, it is a follow-up after
   TASK-161, and archive format v1 leaves the field for it.
3. **Adoption.** Adopt automatically on first open, after the backup, with a one-line
   notice (this spec), or only when the operator runs `nigel books adopt`?
4. **Renaming.** Offer the rename to `<Company Name>.nigel` once from the desktop apps
   (this spec), or rename automatically during adoption when the folder still has a default
   name such as `~/Documents/nigel`?
5. **Exports and previews.** Keep them inside the package (this spec: no change for the
   CLI, the apps already use save panels), or move the CLI's default to a visible folder,
   since Finder hides a package's contents?
6. **Names.** `.nigel` and `.nigelbackup` as the extensions, `My Books.nigel` as the
   default for `nigel init` without a path, and `<config_dir>/Demo.nigel` for the demo:
   confirm or rename.
7. **Retention.** Ship `nigel backup --keep` with TASK-163, or leave pruning to the
   scheduler's own scripts?
