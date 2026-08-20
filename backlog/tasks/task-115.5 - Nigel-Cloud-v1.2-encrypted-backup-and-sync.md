---
id: TASK-115.5
title: 'Nigel Cloud v1.2: encrypted backup and sync'
status: To Do
assignee: []
created_date: '2026-08-17 15:27'
labels:
  - product
  - backend
milestone: m-2
dependencies: []
parent_task_id: TASK-115
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Zero-knowledge protection for the file that is the product: snapshot backup and device-to-device sync of the SQLCipher database, ciphertext only.

- `nigel backup now` / scheduled backup pushes the encrypted database file (plus a manifest: schema version, snapshot time, content hash) to the operator's tenant space. The service never receives the key, and the docs and the UI say exactly that: we store what we cannot read.
- Restore pulls a chosen snapshot and opens it with the operator's password locally; a restore is a new file beside the current one, never an in-place overwrite.
- Sync between devices is snapshot-based with last-writer-wins and *surfaced* conflicts: when two devices diverge, both snapshots are kept and the operator chooses — no silent merge, because SQLite is single-writer and the books are not a CRDT.
- Retention and quota are stated per plan on the website; the client shows what is stored and lets the operator delete any snapshot.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Backup uploads ciphertext and a manifest only; a test pins that no plaintext and no key material crosses the wire
- [ ] #2 Restore lands beside the live database, never over it, and requires the operator's password locally
- [ ] #3 Divergent snapshots from two devices are both retained and surfaced for an explicit choice; no code path merges silently
- [ ] #4 The operator can list and delete stored snapshots from every surface, and the zero-knowledge statement appears in the docs and the UI
<!-- AC:END -->
