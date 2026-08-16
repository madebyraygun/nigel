---
id: TASK-113
title: Tidy three loose ends from the workspace split
status: To Do
assignee: []
created_date: '2026-08-16 23:44'
labels:
  - tauri
  - backend
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Three Minors the final review of PR #24 raised and the branch deliberately did not fix. None blocks anything; they are the kind of thing that becomes invisible if not written down.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 invoicing::wiring::contact_address is gated #[cfg(any(test, feature = "testutil"))] pub, matching Branding::with_template, since its only caller outside the crate is a cfg(test) module in cli/invoice.rs
- [ ] #2 nigel-core's build.rs no longer reaches outside its package root for web/dist, or the constraint that every consumer is a workspace member is written down where a future vendored or published nigel-core would hit it
- [ ] #3 The manifest guard's failure message in crates/nigel/tests/layering.rs names a line number as well as the offending table
<!-- AC:END -->
