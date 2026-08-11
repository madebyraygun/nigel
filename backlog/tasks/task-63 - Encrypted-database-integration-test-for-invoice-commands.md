---
id: TASK-63
title: Encrypted-database integration test for invoice commands
status: In Progress
assignee:
  - '@stream-1'
created_date: '2026-08-07 21:53'
updated_date: '2026-08-11 21:25'
labels:
  - invoicing
  - testing
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/pull/172'
documentation:
  - >-
    docs/superpowers/specs/2026-08-11-task-69-71-63-70-invoice-correctness-design.md
  - docs/superpowers/plans/2026-08-11-task-69-71-63-70-invoice-correctness.md
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Invoice and client commands unlock encrypted databases through the shared prompt_password_if_needed path, which since PR #178 also reads NIGEL_DB_PASSWORD. That combination is untested: the existing encrypted-db integration tests cover recategorize and core commands, but no test drives an invoice command (e.g. invoice list, invoice new, client add) against an encrypted database via NIGEL_DB_PASSWORD. Same follow-up PR #180 recorded for itself after #178 landed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 An integration test runs at least one invoice/client command against an encrypted database unlocked via NIGEL_DB_PASSWORD
- [x] #2 A wrong NIGEL_DB_PASSWORD on an invoice command fails with the documented error rather than hanging on a prompt
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Two integration tests in tests/cli_dispatch.rs, beside recategorize_works_on_encrypted_db_via_env_password. Nothing in src/ changed for this task — invoice/client were never in main.rs's needs_password exclusion list, so the path already worked; the tests are what stop it changing unnoticed.

The happy path asserts a read (invoice list prints 1248 and Acme Co) and a write (client add Globex, then client list shows both). The write matters: unlocking for a SELECT and unlocking for an INSERT are the same key, but a test that only lists would not notice a regression leaving the connection read-only.

The wrong-password test asserts NIGEL_DB_PASSWORD appears on stderr rather than settling for .failure(). Reaching the rpassword prompt with no tty errors with ENXIO, which satisfies a bare .failure() and would let a real hang-or-prompt regression pass — this is backup_fails_fast_on_wrong_env_password's reasoning reused. TEST_TIMEOUT (60s) is the backstop for a run that inherits a tty.

TestEnv::cmd clears all nine NIGEL_* invoicing variables, so the launch sync cannot reach Stripe.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Encrypted-database coverage for the invoice and client commands.

AC #1: invoice_and_client_commands_work_on_encrypted_db_via_env_password — a read and a write against a database encrypted by TestEnv::encrypt, unlocked via NIGEL_DB_PASSWORD.
AC #2: invoice_list_fails_fast_on_wrong_env_password — a wrong password fails with the documented error, asserted by the variable name on stderr so the test cannot be satisfied by an ENXIO from a prompt with no terminal.

Both passed on first run, as expected for characterization tests; no production code changed.
<!-- SECTION:FINAL_SUMMARY:END -->
