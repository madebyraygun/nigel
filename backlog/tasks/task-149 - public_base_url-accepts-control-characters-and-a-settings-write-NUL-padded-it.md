---
id: TASK-149
title: 'public_base_url accepts control characters, and a settings write NUL-padded it'
status: To Do
assignee: []
created_date: '2026-10-07 18:32'
updated_date: '2026-10-07 18:32'
labels:
  - invoicing
  - settings
  - bug
milestone: m-0
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel/issues/48'
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Two defects around one setting, found running the invoice send cycle end to end.

1. validate_public_base_url (crates/nigel-core/src/invoicing/r2.rs) accepts control characters. A stored value of a bare host followed by NUL bytes passed once a scheme was prefixed, and a real email went out whose link embedded the NULs and 404'd. Values composed into a URL should be trimmed and refused if control characters remain — the standard validate_header_value (invoicing/mailgun.rs) already applies, naming the setting.

2. Something rewrote the stored setting in place, NUL-padded to the original byte length: scheme and /i suffix gone, the remainder padded with NUL bytes. That is a fixed-width in-place write, not a hand edit. settings.json and its .bak carried the same corruption, so it predates the backup. Candidates: the CLI settings commands, the dashboard settings screen, the web settings endpoint.

Repro of 1: set public_base_url to a valid URL followed by NUL or space characters, run nigel invoice send.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 public_base_url containing control characters, including trailing NULs, is refused by name before any network call
- [ ] #2 Leading and trailing whitespace is trimmed rather than composed into published links
- [ ] #3 The other URL-valued settings get the same check
- [ ] #4 The writer that NUL-pads values is found and fixed, with a byte-for-byte settings round-trip test
- [ ] #5 Tests pass; docs describe the refusal
<!-- AC:END -->
