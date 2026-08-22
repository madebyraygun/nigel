---
id: TASK-114.1
title: Local delivery config and the LocalDirPublisher
status: To Do
assignee: []
created_date: '2026-08-17 13:27'
updated_date: '2026-08-21 00:21'
labels:
  - invoicing
  - desktop
milestone: m-0
dependencies: []
parent_task_id: TASK-114
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The mode switch and the disk half of delivery.

- A `delivery = "hosted" | "local"` setting, parsed and validated with the config machinery invoicing already uses; unset means hosted, so existing books change nothing. In hosted mode every current behavior — the missing-keys refusal included — is untouched.
- `LocalDirPublisher` implements `AssetPublisher` by writing objects under the outbox directory (default under the data dir, overridable in config), mirroring the publish layout (`outbox/i/<token>/…`, `outbox/d/<token>/…`) so the pure `object_key` functions serve both modes with no second naming scheme.
- The send pipeline stays generic: mode selection happens once at wiring (`wiring.rs` precedent), and nothing above the traits inspects the mode.
- Local publish makes no network call of any kind; the send trace shows the same publish step with the outbox path where the URL would be.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 delivery is parsed and validated; unset behaves as hosted, and hosted behavior including the missing-keys refusal is pinned unchanged by tests
- [ ] #2 LocalDirPublisher writes the page and PDF under the outbox mirroring the publish key layout, exercised through the same traced send the R2 publisher uses
- [ ] #3 No code above the AssetPublisher/Mailer traits branches on the mode, pinned by the wiring
- [ ] #4 A local send makes no network call up to the mail step, proven by the fake-only test rule
<!-- AC:END -->
