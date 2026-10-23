---
id: TASK-162
title: 'Books as a package, and single-file backups'
status: To Do
assignee: []
created_date: '2026-10-23 16:45'
labels:
  - storage
  - backup
  - desktop
  - macos
milestone: m-3
dependencies:
  - TASK-161
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A set of books is one database plus the files that belong to it: SQLite's -wal and -shm while open, filed document PDFs (documents/), and with TASK-161 the key file. Today they are loose in a data directory, and a backup is a .db file plus a sibling .documents folder (PR #55). Make the books one portable thing.

Working form: a books package, a directory with a fixed layout and an extension (e.g. 'Acme Consulting.nigel') containing nigel.db, the key file, documents/ and any other per-book state. On macOS it is declared as a document package so Finder shows it as a single file that opens Nigel on double-click; elsewhere it is a folder. It stays a real directory because SQLite needs random access to a file on disk (no running from inside an archive).

Portable form: nigel backup writes one compressed file (e.g. .nigelbackup: a versioned archive of the database snapshot, key file and documents with a manifest and checksums); restore unpacks it into a package. This replaces the .db + .documents pair.

Settles where TASK-161's key file lives, and is what book switching (TASK-154.12) and the Mac app's folder bookmark (TASK-150.2) point at. Order with TASK-161: the package layout is decided first or together with the recovery spec.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A spec defines the package layout, extension, versioning and the backup archive format (compression, manifest, checksums), reviewed before implementation
- [ ] #2 New books are created as a package; existing data directories are converted (or opened in place and adopted) with a backup first, and the CLI, server and desktop all open a package by path
- [ ] #3 On macOS the package is a declared document type: Finder shows one file, double-click opens it in Nigel, and the app's folder bookmark points at it
- [ ] #4 nigel backup writes a single compressed file containing the database snapshot, key file and documents with a manifest; restore verifies checksums and unpacks it, refusing a damaged or partial archive without touching the current books
- [ ] #5 Old .db + .documents backups still restore; automated backups (launchd/cron examples) and docs use the new format
- [ ] #6 Tests cover create, convert, open, backup, restore, a corrupt archive and an encrypted package
<!-- AC:END -->
