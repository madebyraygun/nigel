---
id: TASK-104
title: perform_pending_republish should take its config and data dir as arguments
status: Done
assignee: []
created_date: '2026-08-13 20:06'
updated_date: '2026-08-14 05:20'
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
- [x] #1 `republish_after_payment` and `republish_all` take the invoicing config and data directory as arguments rather than resolving them from settings
- [x] #2 `perform_pending_republish` receives them the way `begin_send` already receives `cfg` and `data_dir`, with the CLI and TUI resolving settings at their own call sites
- [x] #3 No test in `cli::invoice_manager` or `cli::invoice` can reach ambient settings or the network without passing a config explicitly
- [x] #4 The TASK-103 guard becomes unnecessary for this reason and is removed, or is kept only where it still isolates something else
- [x] #5 Update test coverage
- [x] #6 Create or update documentation, making sure to remove any out of date information
- [x] #7 All linting checks pass
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
`republish_after_payment(conn, invoice_id, cfg, data_dir)` and `republish_all(conn, numbers, cfg, data_dir)` now take what they used to resolve. `republish_with` is the same body with the publisher injected, so the HTTP layer stopped keeping its own copy of the resolution and of the two warning sentences (`server::routes::invoices::republish_page` is gone; `pay_with` takes `cfg`/`data_dir` too). `perform_pending_with` hands the TUI's republish the config and data directory it already had for send and void.

`InvoicingConfig` derives `Default` — an installation with nothing configured is a real state, and it is how a test says "reach nothing".

Both `isolated()` guards (TASK-103's) are deleted: a test that does not pass a config now fails to compile rather than answering from `~/.config/nigel/settings.json`. The server's `TempConfig` stays where it still isolates something else — `detail_for` reads `public_base_url` for `publicUrl`.
<!-- SECTION:NOTES:END -->
