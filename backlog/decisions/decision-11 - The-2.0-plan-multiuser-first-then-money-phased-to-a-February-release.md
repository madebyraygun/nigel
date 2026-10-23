---
id: decision-11
title: 'The 2.0 plan: multiuser first, then money, phased to a February release'
date: '2026-10-23 17:17'
status: accepted
---
## Context

The 2.0 milestone holds 111 open tasks across foundations, retiring the TUI dashboard
(decision-8), the web app refresh (epic 154), invoices and contracts, server/client
(epic 32), the Swift shell (epic 144) and the Mac App Store (epic 150, decision-9).
Contracts (epic 109's core), the refresh's foundations and screens, documents polish
and settings hardening shipped in October.

Three needs set the order:

1. **A second person needs their own login to the web app now.** That needs multiuser
   (users, sessions, roles, a login screen) and a public address with TLS. Exposing
   `nigel serve` beyond loopback before there are per-user logins is not an option: today
   one per-run token is the whole of authentication.
2. **The Mac app must talk to a remote server**, not only to local books (TASK-144.16),
   so the Mac and the browser are both clients of one `nigel serve`.
3. **Sharing with a bookkeeper through Nigel Cloud must not duplicate the database.**
   Hosted live books are a non-goal (docs/product/foundation.md), and the encrypted
   backup and sync work (2.3) copies the file by design.

Two constraints shape what runs in parallel. Database migrations land in order, so the
work that adds one (integer money, users and sessions, attribution, the books package)
runs one at a time. Integer money (TASK-59) and write attribution (TASK-32.4) both touch
every write path, so they do not run side by side. The pace is set less by code than by
the operator's review time, time at a Mac for the Swift shell, and App Review.

## Decision

Multiuser comes before integer money, in the smallest form that gives each person their
own login, and the Mac app's remote mode follows it directly. The rest of 2.0 runs in four
phases toward a release in early February 2027, with about four parallel tracks (core,
web, invoices and contracts, specs and docs) and the operator owning the Mac track.

**Phase 1, now to mid-November — logins and a public address.**
- Multiuser core: users and sessions (32.1), auth and login/logout (32.2), roles (32.3),
  the SPA login and user screens (32.8), and the deployment guide with a public
  address, TLS and a trusted host (32.9). The operator's server on bee is the first
  deployment.
- Swift shell first phase (150.1 sandbox, 144.1–144.3), then remote mode (144.16) as
  soon as 32.2 and 32.9 land, ahead of the native-chrome phases.
- Alongside, without migrations: the rest of the refresh (154.11 in a few PRs, 154.14,
  154.17, 154.10, 124), small safety fixes (155, 53, 54, 55), test reliability (42, 56,
  121, 127, 156), and specs for the books package (163) and recovery codes (161).

**Phase 2, mid-November to early December — money and the TUI's retirement.**
- Integer money (TASK-59), alone on the data layer.
- Retiring the TUI dashboard: 128–130 and 123, then 131 → 132 → 133 → 134, and 116.
- Invoices: schedules on the web (140.1–140.4), 157, 146, 147, 136–138, 142, 37, and the
  merge automation (158).
- Swift shell second phase (native sidebar, menus, settings window).

**Phase 3, December — the rest of multiuser, storage, contracts, delivery.**
- Multiuser completion: attribution (32.4), audit log (32.5), import history (32.6),
  optimistic concurrency (32.7), and server packaging (151).
- Storage and secrets in order: books package (163), recovery codes (161), service keys
  in the keychain (160); the Mac app's books location (150.2) after 163.
- Contracts drafting (109.9, 109.10) and 109.14, 109.15, 109.8; local delivery
  (114.1–114.5); Nigel Cloud delivery (115.3); bookkeeper access to a self-hosted server
  (164). Nigel Cloud's role in bookkeeper sharing is 3.0 (decision-12).
- Swift shell third phase, and store work 150.4–150.7.

**Phase 4, January 2027 — launch.** The accessibility pass (58), the first-run guide
(154.18), the logo (154.13, when the art lands), nigel.works (115.1), the docs pass (152),
the App Review kit and TestFlight (150.8–150.10) with two to three weeks allowed for review,
and the release (153) in early February.

## Consequences

- Integer money moves from first to second phase. Decision-7 still holds: it lands before
  the public launch. Its migration takes the number after the users-and-sessions migration.
- Attribution, the audit log and optimistic concurrency arrive after integer money, so the
  first multiuser deployment records who is logged in but not yet who changed each row.
  That is acceptable for a household with two trusted users and is not acceptable for a
  bookkeeper; bookkeeper access waits for phase 3.
- Exposing a server publicly is gated on 32.2 and 32.9: until per-user login exists, the
  server stays on loopback or a private network.
- Remote mode on the Mac depends on 32.2 and 32.9, so the Swift shell's first phase is
  scheduled to finish when they do, and its native-chrome phases slide later.
- The dates are estimates for planning order, not commitments; the order is the decision.
