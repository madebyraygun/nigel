---
id: TASK-144.1
title: >-
  nigel-ffi: a UniFFI staticlib answering scheme requests from the desktop
  router
status: To Do
assignee: []
created_date: '2026-09-15 16:06'
labels:
  - swift
  - rust
  - macos
dependencies: []
documentation:
  - docs/superpowers/specs/2026-09-14-swift-shell-design.md
parent_task_id: TASK-144
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 1. A crates/nigel-ffi staticlib exporting one object, NigelHost: new(data_dir) builds AppState and build_desktop_router; async request(method, path, headers, body) is crates/nigel-desktop/src/transport.rs::answer with the Tauri types swapped for plain records (carry the authority as Host, oneshot the router, collect the body under the same 64 MB cap); stage_file(path) is imports.rs::stage_file unchanged; data_dir(). The scheme_url and trusted_origins helpers move to a module both shells share so the origin cannot drift between them. The SPA stays embedded through rust-embed over web/dist inside the staticlib. The desktop feature keeps the router session-free, as build_desktop_router's doc comment requires. Excluded from the root workspace like nigel-desktop is, so default features stay what nigel serve builds.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 NigelHost::request answers status, headers and body for GET, JSON POST and multipart, driven in tests against testutil-seeded databases on the pattern of crates/nigel-desktop/tests/desktop_router.rs
- [ ] #2 stage_file produces the same StagedUpload a browser upload would, and the origin helpers are shared with nigel-desktop with their drift tests intact
- [ ] #3 The crate builds for aarch64-apple-darwin with SQLCipher's vendored OpenSSL statically linked and reqwest on native-tls; the root workspace's default features are unchanged
- [ ] #4 cargo test in the crate passes; cargo fmt --check passes
<!-- AC:END -->
