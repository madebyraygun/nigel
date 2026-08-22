---
id: decision-8
title: >-
  Deprecate the TUI dashboard: web and desktop lead, the CLI keeps its TUI
  subscreens
date: '2026-08-22 14:05'
status: accepted
---
## Context

Nigel maintains three interactive surfaces: the TUI dashboard (bare `nigel`), the CLI, and
the web SPA that also ships as the Tauri desktop shell. That is too many to keep at parity —
every feature pays for itself three times, and the invariants in `docs/design-constraints.md`
are written as three-way parity claims that each change has to re-prove.

The SPA reached parity or better with the dashboard on nearly every screen: accounts,
categories, clients, invoices (fixture-pinned), rules, review, imports, reconcile, undo,
settings, onboarding, and Snake. The dashboard-only TUI code — `dashboard.rs` plus the
thirteen screens reachable only from it, with splash, goodbye, onboarding and effects —
is ~12,300 lines carrying ~58% of the crate's unit tests, none of it reachable from any
CLI subcommand. The TUI screens that CLI subcommands *do* invoke are a separate ~3,600
lines: the report viewers (`nigel report <kind>` on a TTY), the register browser
(`nigel browse register`), and the reviewer (`nigel review`).

## Decision

**The TUI dashboard and its dashboard-only screens are deprecated and will be removed.
The web SPA and desktop shell are the interactive surface; the CLI keeps the TUI
subscreens its subcommands invoke.**

- **Bare `nigel` becomes `nigel serve`**: it starts the server and opens the browser with
  the session link, the way `nigel serve` does today. First run lands on the web setup
  screen; `nigel init` remains the terminal path.
- **The report viewers, register browser, and reviewer stay.** `ratatui`/`crossterm`
  remain dependencies; there is no TUI feature flag to flip.
- **Settings keeps a terminal path via a new `nigel settings` subcommand** that reuses
  `settings_manager.rs` (and `password_manager.rs` behind it) with a small standalone
  driver on the `review.rs` pattern. It is the one manager whose function has no CLI
  equivalent, and it is self-contained: its only dependencies are `tui.rs` styles,
  `password_manager`, and `nigel-core`.
- **The web gaps close before removal**: TASK-128 (Export All Reports + aging export),
  TASK-129 (register category/uncategorized filters + export), TASK-130 (A/R dashboard
  card), with TASK-123 (import rejects detail) and TASK-116 (Nigel's voice) already filed.

## Consequences

- ~12,300 lines and ~283 inline tests are deleted; `effects.rs`, `splash.rs`,
  `goodbye.rs`, `onboarding.rs` and `snake.rs` go with the dashboard. The terminal loses
  its animated brand moments — the gradient logo and conversational voice obligations
  move entirely to the SPA (TASK-116).
- The three-way parity invariants in `docs/design-constraints.md` become two-way
  (CLI/API); `docs/walkthrough.md` is rewritten around the web UI; `docs/architecture.md`,
  `README.md`, `docs/invoicing.md`, `docs/api.md` and the site drop their dashboard
  sections and screenshots.
- Dashboard-only bugs close by deletion (TASK-82, TASK-83, TASK-85, TASK-37, TASK-97,
  TASK-127); planned work loses its TUI leg (TASK-9.9, TASK-19, TASK-20, TASK-21,
  TASK-109.6, TASK-119, TASK-120, TASK-122, TASK-17, TASK-18).
- Client and invoice management in the terminal falls back to the `nigel client` /
  `nigel invoice` CLI; the two-week-old manager screens (TASK-68.4) are deleted with
  the rest — the web invoicing screen is the replacement and is fixture-pinned.
- Non-terminal users without the paid desktop build still reach the app through bare
  `nigel` opening the browser (decision-3 is unchanged).
