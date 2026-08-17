---
id: TASK-33.12
title: 'Exercise the desktop shell on Windows and Linux, and its importers'
status: To Do
assignee: []
created_date: '2026-08-17 19:28'
labels:
  - tauri
  - qa
dependencies: []
parent_task_id: TASK-33
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The desktop shell is verified on macOS by hand and on Linux only through tests that drive the router without a window. Windows has neither, and no import has been run in the shell on any platform.

Windows matters most because its transport differs: it serves the app from http://nigel.localhost, a real HTTP origin that carries a Host header, where the custom scheme on macOS and Linux sends none. Both defects that made the shell unusable on macOS — the missing Host header and the unrecognised custom-scheme origin — could not occur there, and a Windows-only defect would be invisible to every check made so far.

Imports are unexercised because the shell took nigel-core without the gusto feature until that was corrected alongside the missing PDF support. The feature is on now and has never been run.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The shell launches, browses and unlocks an encrypted database on Windows
- [ ] #2 The shell launches, browses and unlocks an encrypted database on a Linux desktop
- [ ] #3 A report text export, a report PDF export and an invoice PDF each save through the native dialog on both platforms, with bytes matching what nigel serve produces for the same report
- [ ] #4 A CSV or XLSX import runs to completion in the shell on at least one platform, including the gusto path the crate now enables
- [ ] #5 Anything found is fixed with a regression test, or filed with the platform and the reproduction recorded
<!-- AC:END -->
