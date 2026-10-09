---
id: TASK-150.6
title: 'Privacy: label, policy and manifest'
status: To Do
assignee: []
created_date: '2026-10-07 18:46'
labels:
  - appstore
  - privacy
  - docs
milestone: m-3
dependencies: []
parent_task_id: TASK-150
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Audit and write down what leaves the machine: only operator-configured services (Stripe, Mailgun, R2, Nigel Cloud, the operator's own server) and, in direct builds only, update checks. If confirmed, the App Store privacy label is Data Not Collected. Commit a PrivacyInfo.xcprivacy listing the required-reason APIs the shell and the Rust staticlib use. The privacy policy page lives on nigel.works (TASK-115.1).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The network audit is in docs/ and matches the code
- [ ] #2 The privacy manifest is committed
- [ ] #3 The privacy policy URL is live
<!-- AC:END -->
