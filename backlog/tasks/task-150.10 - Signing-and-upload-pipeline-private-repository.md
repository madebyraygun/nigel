---
id: TASK-150.10
title: Signing and upload pipeline (private repository)
status: To Do
assignee: []
created_date: '2026-10-07 18:46'
labels:
  - appstore
  - release
milestone: m-3
dependencies: []
parent_task_id: TASK-150
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Per decision-3, nothing in this repository builds or signs. This task tracks that the private pipeline exists: build from a tag of this repository, sign with Apple Distribution and Mac Installer Distribution identities, upload to App Store Connect, distribute through TestFlight for Mac. Bundle ID com.madebyraygun.nigel; it cannot change once the App Store Connect record exists.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A TestFlight build of the store flavor installs and runs on a second Mac
<!-- AC:END -->
