---
id: TASK-104
title: perform_pending_republish should take its config and data dir as arguments
status: To Do
assignee: []
created_date: '2026-08-13 20:06'
labels:
  - refactor
  - invoicing
  - testing
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`InvoiceManager::perform_pending_republish` resolves the invoicing config and the data directory from ambient settings — `cli::invoice::republish_after_payment` calls `invoicing_config()` and `get_data_dir()` internally — while its sibling `begin_send` takes `cfg` and `data_dir` as parameters precisely so a test can inject them.

That inconsistency is not theoretical. TASK-103 was a test that omitted the `TempConfigDir` guard and therefore answered from the developer's real `~/.config/nigel/settings.json`. With R2 configured, the republish succeeded instead of being skipped, so no stale-page warning was produced and the assertion failed — and every local `cargo test` uploaded a fabricated invoice page to a live bucket. A guard fixes that one test; it does not stop the next one being written the same way.

Making the seam explicit removes the failure mode rather than documenting it: a test that does not pass a config cannot reach one, and the compiler says so.

Relevant code: `src/cli/invoice_manager.rs` (`perform_pending_republish`, `perform_pending`), `src/cli/invoice.rs` (`republish_after_payment`, `republish_all`), and the `begin_send`/`send_with` pattern this should mirror.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `republish_after_payment` and `republish_all` take the invoicing config and data directory as arguments rather than resolving them from settings
- [ ] #2 `perform_pending_republish` receives them the way `begin_send` already receives `cfg` and `data_dir`, with the CLI and TUI resolving settings at their own call sites
- [ ] #3 No test in `cli::invoice_manager` or `cli::invoice` can reach ambient settings or the network without passing a config explicitly
- [ ] #4 The TASK-103 guard becomes unnecessary for this reason and is removed, or is kept only where it still isolates something else
- [ ] #5 Update test coverage
- [ ] #6 Create or update documentation, making sure to remove any out of date information
- [ ] #7 All linting checks pass
<!-- AC:END -->
