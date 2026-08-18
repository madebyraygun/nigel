---
id: decision-4
title: 'nigel-core is an internal crate, not a published library'
date: '2026-08-18 03:05'
status: accepted
---
## Context

The workspace split moved the implementation into `nigel-core` and left `nigel` holding the
terminal UI. Crossing a crate line meant widening items that had been `pub(crate)`, and
`src/lib.rs` publishes every module, so the widening looked like a promise to a library
consumer. That raised a question nobody had answered: what does this crate promise, and to
whom?

The facts settle it. Nothing publishes `nigel-core` — no `cargo publish` in CI, no mention in
the docs, no version metadata beyond the workspace's own. It has exactly two consumers,
`nigel` and `nigel-desktop`, both in this repository. Of 525 distinct `pub` names, 247 are
referenced by neither.

Read as a library, that surface is 247 items of unearned promise. Read as an internal crate,
it is unremarkable: `pub` is what the language offers for "another crate in this workspace may
call this", and there is no third party for it to mislead.

## Decision

**`nigel-core` is internal. `pub` means "reachable across the workspace's crate line" and
carries no compatibility promise.** `publish = false` in its manifest makes that mechanical
rather than a convention, so the surface cannot quietly become a semver obligation.

**A wide surface is therefore not a defect, and narrowing an unused item is not a breaking
change.** Neither needs a deprecation cycle, a version bump, or a discussion. Narrow when it
clarifies; leave it when it does not.

**What is not licensed by any of this is a type that lies.** A crate with no external
consumers still has two internal ones, and a constructor that validates is worth nothing if a
caller can assemble the same type by hand. Where a type has an invariant, the invariant is
enforced in the type system, not asserted in a doc comment:

- `SendClients` and `CompanyProfile` have private fields; `build_clients` and
  `company_profile` are the only constructors. `build_clients` is where the public base URL is
  checked for a working link and the from address is checked as a bare, header-safe value — a
  hand-built set skips all of it and mails from whatever it was handed.
- `CategorySelection::Named` is `#[non_exhaustive]`, so only `RegisterFilters::resolve` — which
  reads the id and the display name out of one row — can produce one. `RegisterFilters` has
  private fields, and the two constructors a production build offers both carry a category the
  database agreed to: `resolve` reads one out, `for_account` sets none.
- Test-only constructors live behind `#[cfg(any(test, feature = "testutil"))]`, which is how a
  dependent's tests build one without the door being open in a normal build. `with_category`
  and `CategorySelection::named_for_test` are both there, because no production caller wants
  either: the CLI resolves its filters from user input, and the HTTP routes refuse category
  filters by name.

**A helper reaches the public surface when a cross-crate caller needs it, and not before.**
`updater::http_client` had gone straight from private to `pub` for one caller that wanted a
long timeout, putting an unvalidated `timeout_secs` on the surface. `updater::download_release`
replaces it: the timeout and the truncation floor are properties of downloading a release, so
they live with it, and `http_client` is private again — matching `invoicing::http_client`,
which never left the module.

## Consequences

- Every item the boundary move widened has a live cross-crate caller today, so nothing was
  narrowed for its own sake. That check is worth repeating whenever the split moves again.
- `cargo publish` on this crate now fails. If Nigel is ever distributed as a library, this
  decision is what has to be revisited first, and the 247 unreferenced names are the work item
  it implies.
- `nigel` is untouched by this and stays publishable; it is the crate a `cargo install` would
  name.
- The `testutil` feature grows as invariants tighten. Anything added there is compiled out of a
  normal build, and a constructor that belongs behind it must not acquire a production caller.
