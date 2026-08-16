---
id: TASK-33.10
title: Decide the core crate's public API surface at the split
status: To Do
assignee: []
created_date: '2026-08-16 12:58'
labels:
  - tauri
  - backend
dependencies:
  - TASK-33.1
parent_task_id: TASK-33
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The boundary move widened several items from `pub(crate)` to `pub` because the crate split will need them reachable across a crate line. Today they are wider than anything requires, and `src/lib.rs` publishes them, so the widening is already a promise to anyone depending on the library.

Three things to settle when the split actually happens, together rather than one at a time:

- `CompanyProfile` and `SendClients` (`invoicing/wiring.rs`) are `pub` with `pub` fields and no enforcement. Their doc says the fields are only ever correct together, and the constructors — `company_profile`, `build_clients` — are the single read points that make that true. As `pub(crate)` that was a convention held by everyone who could see it; as `pub` it is a promise nothing backs, and a desktop client can hand-build one with mismatched fields. Either give them private fields and accessors, or say plainly in the docs that they are DTOs with no invariant.
- `RegisterFilters`/`CategorySelection::Named` (`reports/mod.rs`) has the same shape and predates the move: `pub` fields let a caller construct a mismatched id/name pair and bypass `resolve()`'s database check entirely. It became the boundary API when `reports::text::register` started taking it.
- `updater::http_client` went private straight to `pub`, skipping `pub(crate)`, and now exposes an unvalidated `timeout_secs` on the public surface. Its sibling `invoicing::http_client` stayed `pub(crate)`, so the two disagree for no reason.

Decide these as one question — what the core crate promises — rather than piecemeal, since every answer is a different shape of the same trade.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every pub item on the core side is either required by a cross-crate caller or narrowed back
- [ ] #2 CompanyProfile and SendClients either enforce their invariant or document that they do not
- [ ] #3 RegisterFilters cannot be constructed in a state resolve() would have rejected, or the docs say why that is acceptable
- [ ] #4 updater::http_client and invoicing::http_client agree on visibility
<!-- AC:END -->
