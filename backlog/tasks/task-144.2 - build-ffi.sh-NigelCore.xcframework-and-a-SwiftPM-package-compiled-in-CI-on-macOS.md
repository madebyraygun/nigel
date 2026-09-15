---
id: TASK-144.2
title: >-
  build-ffi.sh: NigelCore.xcframework and a SwiftPM package, compiled in CI on
  macOS
status: To Do
assignee: []
created_date: '2026-09-15 16:06'
labels:
  - swift
  - rust
  - macos
  - ci
dependencies:
  - TASK-144.1
documentation:
  - docs/superpowers/specs/2026-09-14-swift-shell-design.md
parent_task_id: TASK-144
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 1. scripts/build-ffi.sh runs the chain in order: npm run build in web/, cargo build of nigel-ffi for aarch64-apple-darwin, uniffi-bindgen for Swift, xcodebuild -create-xcframework into apps/Nigel/Packages/NigelCore, with --check to verify the xcframework is current and --test to run the Rust and Swift suites. CI compiles the crate and the Swift package on a macOS runner and, per decision-3, signs, notarizes and publishes nothing.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 scripts/build-ffi.sh produces apps/Nigel/Packages/NigelCore from a clean checkout on a Mac, and --check and --test do what they say
- [ ] #2 A macOS CI job builds nigel-ffi and the Swift package on every push and pull request; no signing identity, notarization step or update feed enters the repository
- [ ] #3 docs/desktop.md documents the build order and the dev loop for the Swift shell
<!-- AC:END -->
