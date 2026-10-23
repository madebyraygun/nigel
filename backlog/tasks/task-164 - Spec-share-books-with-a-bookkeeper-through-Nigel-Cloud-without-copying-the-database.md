---
id: TASK-164
title: >-
  Spec: share books with a bookkeeper through Nigel Cloud without copying the
  database
status: To Do
assignee: []
created_date: '2026-10-23 17:18'
labels:
  - cloud
  - multiuser
  - spec
milestone: m-3
dependencies:
  - TASK-32.2
  - TASK-32.9
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A bookkeeper should be able to log in to the operator's books from anywhere without the operator sending a copy of the database or running their own public server. Hosted live books are a non-goal (docs/product/foundation.md) and encrypted sync (2.3) copies the file, so neither fits.

Leading option to evaluate: Nigel Cloud as a relay. The operator's nigel serve opens an outbound, authenticated tunnel to Nigel Cloud; Nigel Cloud gives it a stable HTTPS address and forwards requests to it; the bookkeeper logs in with their own Nigel user (epic 32, a read-only or bookkeeper role), and the books never leave the operator's machine. Trade-off: the books are reachable only while the operator's server is running.

The spec compares the relay with alternatives (self-hosted tunnel such as Cloudflare Tunnel or Tailscale Funnel, a read-only published snapshot, invitations), and settles: who terminates TLS and whether Nigel Cloud can read traffic (end-to-end encryption between browser and server, or relay-visible), invitation and account flow, which roles a remote user can hold, what the operator sees and can revoke, availability when the server sleeps, abuse and rate limits, pricing hooks for Nigel Cloud, and how the Mac app's remote mode (TASK-144.16) uses the same address.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A spec under docs/superpowers/specs/ compares the relay with at least two alternatives and recommends one, with the security model (what Nigel Cloud can see, how access is revoked) stated
- [ ] #2 The spec defines the invitation flow, remote roles, availability behaviour and the operator's controls, and lists the implementation tasks it implies
- [ ] #3 The operator reviews and approves the spec; follow-up tasks are filed against the right milestone
<!-- AC:END -->
