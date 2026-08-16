---
id: TASK-33.9
title: >-
  Trial workspace split in CI, to catch the coupling the layering guard cannot
  see
status: Done
assignee: []
created_date: '2026-08-16 12:58'
updated_date: '2026-08-16 23:45'
labels:
  - tauri
  - ci
  - backend
dependencies:
  - TASK-33.1
parent_task_id: TASK-33
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
tests/layering.rs greps the core paths for `crate::cli::`. That catches a path, and nothing else. Coupling that arrives without the literal string is invisible to it: an inherent impl left behind by a moved type, a trait impl satisfying a core-defined trait from the CLI side, a closure the CLI hands in that core merely stores and calls, a re-export alias that hides the path.

This is not hypothetical. The boundary move's own final review found exactly one: `impl CompanyProfile` stayed in `cli/invoice.rs` while its struct moved to `invoicing::wiring`. Rust requires an inherent impl in the type's defining crate, so the split would have left `src/server` and `src/invoicing` unable to call `branding()` without the CLI — while the guard reported a clean zero. A human caught it; the check could not.

The compiler is the only thing that sees this class of defect, so the check is a real crate boundary. Add a CI job that builds the core paths as their own crate — a throwaway manifest is enough, it does not have to be the permanent workspace layout of task-33.1 — and fails when that crate cannot compile without the CLI. Then the text guard becomes the fast local signal and CI holds the actual line.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 CI fails when a core-side module depends on src/cli/ by any mechanism, including one that names no crate::cli:: path
- [ ] #2 The check is a compile of the core paths without the CLI, not a second text scan
- [ ] #3 A deliberately reintroduced inherent-impl split — the defect this is built for — is verified to fail it
- [ ] #4 tests/layering.rs stays as the fast local check, with its limits stated where a reader of it will find them
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Superseded by TASK-33.1. The trial split existed to make the compiler, rather than a text scan, the check on core-to-CLI coupling. The workspace split makes that permanent: nigel-core is its own crate and does not depend on nigel, so coupling that names no crate::cli:: path — an inherent impl left behind by a moved type, a trait impl from the CLI side, a stored closure — is a compile error rather than something a grep must be taught to see. CI builds nigel-core on its own via cargo test -p nigel-core. A throwaway trial manifest would now be a weaker copy of the real layout.
<!-- SECTION:NOTES:END -->
