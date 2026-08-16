---
id: TASK-110
title: Boundary cleanups left after the core move
status: To Do
assignee: []
created_date: '2026-08-16 12:58'
labels:
  - backend
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Small things a five-agent review of the boundary-move PR surfaced. None blocks anything; grouped so they cost one pass rather than four.

- `src/accounts.rs` and `src/categories.rs` carry section headers reading 'Data-layer functions for TUI account/category management'. Both are now wrong twice over: the HTTP API uses these too, and the whole file is data layer, so there is nothing left for the header to separate them from. Delete rather than reword.
- `cli/report/mod.rs` opens a connection to resolve register filters and drops it, then `reports::text::register` opens a second one for the query. `cli/browse.rs` still does both on one connection. Same file, no bug, but the two paths disagree.
- `cli/invoice.rs` re-exports `invoicing::wiring::*` with a glob. An explicit list would say what the CLI actually uses and would not silently absorb a future name collision.
- `tests/layering.rs` matches `crate::cli::` as a raw substring, so it flags the string in a comment as readily as in code. That already shaped the tree once: an implementer reworded a doc comment to avoid tripping it. False positives only, never false negatives.
- The rule that nothing under `src/invoicing/` reads settings is enforced by a comment and by review. The layering guard already walks that directory for one forbidden string; a second string would cost almost nothing and would make the rule as checkable as the CLI boundary is.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The stale TUI-only section headers are gone from accounts.rs and categories.rs
- [ ] #2 Register filter resolution and the register query agree on how many connections they open
- [ ] #3 The wiring re-export names what it re-exports
- [ ] #4 The invoicing-never-reads-settings rule is enforced by a test rather than by review
<!-- AC:END -->
