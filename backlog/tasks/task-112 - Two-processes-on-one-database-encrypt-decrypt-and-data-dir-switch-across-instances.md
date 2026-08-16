---
id: TASK-112
title: >-
  Two processes on one database: encrypt, decrypt and data-dir switch across
  instances
status: To Do
assignee: []
created_date: '2026-08-16 18:40'
labels:
  - backend
  - db
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Nothing stops two Nigel processes opening the same nigel.db. It is reachable today — `nigel serve` in one terminal and `nigel review` in another — and the desktop client (task-33) makes it ordinary rather than unusual, since the app is a window somebody leaves open.

Routine concurrency is already handled: `db.rs` sets `journal_mode=WAL` and a 5s `busy_timeout`, so concurrent readers and an occasional writer behave. The hazard is narrower and sharper than that.

`AppState.db_gate` is an `Arc<TokioRwLock<()>>` — an **in-process** lock. Readers hold it while a connection is open so that encrypt, decrypt and the data-directory switch can rewrite the database file exclusively. Those three finish by renaming the database file and deleting the `-wal`/`-shm` sidecars, which a live connection in another process would not survive, and which that process has no way to learn about. A second instance can be mid-report while the first decides the file it is reading no longer exists under that name.

Settle the stance and make it true: either a second instance is safe (a cross-process lock the three rewriting operations take, and every connection respects), or it is refused up front with a sentence naming what already holds the database. Silently proceeding is the one option that is not acceptable, because the operations at risk are the ones that rewrite the whole file.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The stance on a second process opening the same database is decided and documented in CLAUDE.md
- [ ] #2 Encrypt, decrypt and data-dir switch cannot rename the database or delete its sidecars while another process holds a connection
- [ ] #3 A refused second instance says what already holds the database rather than failing obscurely
- [ ] #4 The CLI and the desktop app behave the same way, since either can be the first or the second process
<!-- AC:END -->
