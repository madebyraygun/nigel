---
id: TASK-33.6
title: 'Packaging, signing, and release CI for desktop'
status: To Do
assignee: []
created_date: '2026-08-06 16:29'
updated_date: '2026-08-18 02:03'
labels:
  - tauri
  - ci
milestone: m-0
dependencies:
  - TASK-33.5
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Produce installable, trusted artifacts for the desktop client: a macOS universal build with notarization, a Windows installer with code signing or a documented interim unsigned stance, and a Linux AppImage and deb, versioned in step with the CLI.

That packaging does not run in this repository's CI. The desktop build is the thing being sold, and decision-3 records why producing it here would put the packaging, the signing identities and the update feed in a public repository. This repository keeps compiling and testing crates/nigel-desktop so the shell cannot rot; it publishes no installer.

So this task is the definition and the operation of that pipeline wherever it lives, not a workflow file added here. It depends on the licensing work in TASK-115.2, which owns the merchant of record, the keys and the feed.

Building from source stays supported and documented for anyone who would rather do that — the source is MIT, and docs/desktop.md carries the instructions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Desktop and CLI report the same version for a given release
- [ ] #2 README documents desktop installation per platform
- [ ] #3 A tagged release produces installers for macOS (notarized universal), Windows and Linux from a pipeline outside this repository
- [ ] #4 This repository's CI publishes no desktop installer and no update manifest, and still compiles and tests crates/nigel-desktop on every pull request
- [ ] #5 Building the desktop app from a source checkout stays supported, and docs/desktop.md says how
<!-- AC:END -->
