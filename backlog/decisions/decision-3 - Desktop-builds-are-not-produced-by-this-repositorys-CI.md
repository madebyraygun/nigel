---
id: decision-3
title: Desktop builds are not produced by this repository's CI
date: '2026-08-18 02:02'
status: accepted
---
## Context

The desktop client goes behind a paywall. The source stays MIT, so anyone may build and
redistribute it; what is sold is the signed, notarized, auto-updating build and the feed it
updates from. TASK-115 states that position — *the paywall is on the artifact, not the
software* — and this decision records what it means for this repository.

Two tasks in the Tauri epic were written before that position existed and assume the
opposite. TASK-33.6 says "CI matrix builds on tag" and asks for tagged releases to produce
installers for all three platforms. TASK-33.5 asks for "one release pipeline" publishing CLI
artifacts, desktop bundles and the update manifest together, fed from GitHub Releases. An
agent picking either of those up would build the selling machinery in the public repository
and would look, by the task's own criteria, to have finished correctly.

## Decision

**This repository's CI does not build, sign, notarize or publish desktop installers, and
does not publish an update manifest.** Those artifacts are the thing being sold; producing
them here would put the packaging, the signing identities and the update feed in a public
repository, and would make every fork a distributor of something indistinguishable from the
paid build.

**CI does keep compiling and testing `crates/nigel-desktop`** on Linux and macOS, from its
own workspace, as it does today. The desktop code is MIT like the rest and must not be
allowed to rot: a change that breaks the shell should fail a pull request here, even though
no installer is produced.

**The CLI is unaffected.** `nigel` remains built and released from this repository for all
platforms, and its `self_replace` update path is unchanged. TASK-33.1's criterion that
release CI still produces the existing CLI binaries stands as written.

Where the paid pipeline lives, what signs it, and what the licensed update feed is are not
settled here. They belong with the licensing work in TASK-115.2, which owns the merchant of
record, the keys and the feed.

## Consequences

- TASK-33.5 and TASK-33.6 are rewritten against this: the updater points at a licensed feed
  rather than GitHub Releases, and packaging is defined as work that happens outside this
  repository's CI. Their acceptance criteria no longer ask this repository to publish
  installers.
- Anyone can still build the desktop app from source, and `docs/desktop.md` documents how.
  That is what MIT means here, and it is not a fallback or a courtesy — it is the position.
- A contributor cannot verify a packaging change through a pull request in this repository,
  because no packaging runs here. Whatever produces the paid artifacts needs its own
  verification, and TASK-33.12's cross-platform checks stay manual until it exists.
- Nothing in the desktop crate may depend on being built by the paid pipeline. It has to
  build and run from a plain `cargo run` in a checkout, which is also how it is developed.
