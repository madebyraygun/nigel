---
id: TASK-161
title: Password recovery codes for encrypted books
status: To Do
assignee: []
created_date: '2026-10-23 16:43'
labels:
  - security
  - encryption
  - settings
milestone: m-3
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Losing the books password loses the books. Add a recovery path that does not weaken encryption: envelope encryption with a printed recovery code.

Design (to be written up as a spec under docs/superpowers/specs/ and reviewed before any code):
- The database is keyed with a random 256-bit data key (SQLCipher raw key), not a password-derived key.
- The data key is stored wrapped twice in a small key file beside the database: once under a key derived from the password (Argon2id), once under a key derived from a recovery code. Either unwraps it.
- The recovery code is generated when encryption is turned on: 128 bits of randomness shown in a readable grouped form, offered as a printable recovery sheet, and confirmed by typing part of it back. Regenerating it invalidates the old one.
- Recovering: enter the code, open the books, set a new password. Changing the password re-wraps the data key only; the database is not rewritten.
- The key file travels with the books: nigel backup and restore include it, and its contents are safe to store because both copies are wrapped.
- Existing encrypted books migrate once (backup, then rekey from the password-derived key to the data key).

The spec decides: KDF and parameters, key-file format and versioning, where the file lives relative to the books (see the books-package question), behaviour if the key file is missing or corrupt, the threat model (the code is equivalent to the password), and how the desktop Keychain (TASK-144.13) and the service-key work (TASK-160) fit.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A spec covering the design points above is written, reviewed and approved before implementation starts
- [ ] #2 Turning encryption on generates a recovery code, offers a printable sheet, and requires confirming part of the code before finishing
- [ ] #3 Forgetting the password: the books open with the recovery code and a new password can be set; the old password stops working
- [ ] #4 Changing the password does not rewrite the database; regenerating the recovery code invalidates the previous code
- [ ] #5 Backups and restores carry the key file; a missing or corrupt key file is reported clearly and never results in a new or unencrypted database
- [ ] #6 Existing encrypted books migrate with a backup first; tests cover wrap/unwrap, recovery, password change, code regeneration, migration and the failure cases
- [ ] #7 Settings > Security and docs (design-constraints, README) explain the code is as powerful as the password and how to store it
<!-- AC:END -->
