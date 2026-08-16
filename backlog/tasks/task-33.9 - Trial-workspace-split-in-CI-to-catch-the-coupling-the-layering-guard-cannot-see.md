---
id: TASK-33.9
title: >-
  Trial workspace split in CI, to catch the coupling the layering guard cannot
  see
status: To Do
assignee: []
created_date: '2026-08-16 12:58'
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
