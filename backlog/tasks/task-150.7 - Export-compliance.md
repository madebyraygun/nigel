---
id: TASK-150.7
title: Export compliance
status: To Do
assignee: []
created_date: '2026-10-07 18:46'
labels:
  - appstore
  - docs
milestone: m-3
dependencies: []
parent_task_id: TASK-150
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Nigel ships non-Apple encryption (SQLCipher with vendored OpenSSL) and TLS. Answer App Store Connect's encryption questions once, set ITSAppUsesNonExemptEncryption in Info.plist to match, and record the answer and its basis in docs/ so it is not re-argued every release. Any government filing is an operator task outside the repo.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Info.plist sets ITSAppUsesNonExemptEncryption to match the recorded answer
- [ ] #2 docs/ records the answer and its basis
<!-- AC:END -->
