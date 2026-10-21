---
id: TASK-155
title: An unreadable settings.json is replaced by defaults on the next save
status: To Do
assignee: []
created_date: '2026-10-21 14:37'
labels:
  - settings
  - bug
  - data-integrity
milestone: m-3
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel/pull/60'
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
load_settings returns the defaults when settings.json fails to parse, without telling anyone. The next save — including the updater's automatic last_update_check timestamp — then writes the defaults over the file, erasing every configured key (Mailgun, R2, Stripe, data_dir). One stray character in a hand edit costs the whole configuration, and the operator learns it when a send fails.

Found while auditing settings writers for TASK-149 (PR #60).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A settings.json that fails to parse is never overwritten: every save path refuses with a sentence naming the file and the parse error
- [ ] #2 Commands that only read settings report the parse error once and carry on with defaults only where that cannot lose data
- [ ] #3 The web UI and API surface the broken file as an error, not as an unconfigured install
- [ ] #4 Tests cover a malformed file surviving the updater's timestamp write and every other writer byte for byte
<!-- AC:END -->
