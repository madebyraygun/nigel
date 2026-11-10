---
id: TASK-175
title: 'Settings derives Debug, so stored secrets would print in clear'
status: To Do
assignee: []
created_date: '2026-11-10 02:07'
labels:
  - security
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`Settings` (settings.rs) derives `Debug`, so `simplefin_access_url`, `stripe_secret_key`, `mailgun_api_key` and the R2 keys would print in clear if a `Settings` value were ever Debug-formatted (a log line, an `{:?}` in an error, a panic message). Nothing does that today; the SimpleFIN task promised Debug redaction.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Debug-formatting `Settings` shows every secret field as redacted
- [ ] #2 A test formats a `Settings` holding each secret and asserts none appears
<!-- AC:END -->
