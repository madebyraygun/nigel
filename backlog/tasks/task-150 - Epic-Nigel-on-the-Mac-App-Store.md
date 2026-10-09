---
id: TASK-150
title: 'Epic: Nigel on the Mac App Store'
status: To Do
assignee: []
created_date: '2026-10-07 18:45'
labels:
  - epic
  - appstore
  - macos
milestone: m-3
dependencies:
  - TASK-144
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Ship the Swift shell (TASK-144) through the Mac App Store. The store build is the same app with three differences: it runs in the App Sandbox, it has no update mechanism of its own, and it asks for no license key. Packaging, signing and upload happen outside this repository (decision-3); this repository carries the sandbox-safe code, the build flavor, and the docs. The store is the 2.0 merchant and updater, paid up front (decision-9).

Bundle ID: com.madebyraygun.nigel. Intended store name: "Nigel: Small business accounting, invoices, and contracts" — App Store names and subtitles are each capped at 30 characters, so AS.8 settles the split.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A TestFlight build of the store flavor installs and runs on a second Mac
- [ ] #2 The store build passes App Review and is live on the Mac App Store
<!-- AC:END -->
