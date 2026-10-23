---
id: TASK-32.9
title: Network exposure and deployment guide
status: To Do
assignee: []
created_date: '2026-08-06 16:28'
updated_date: '2026-10-23 17:18'
labels:
  - multiuser
  - backend
  - docs
milestone: m-3
dependencies:
  - TASK-32.2
parent_task_id: TASK-32
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Serving beyond loopback is an explicit opt-in: a --bind flag that refuses non-loopback addresses unless multiuser auth is enabled and the operator acknowledges the exposure. Add sensible security headers. Write the operator guide: Tailscale serve recipe (recommended for tiny teams), reverse proxy with TLS (Caddy example), VPS checklist, off-machine backup guidance using the existing backup command, a systemd unit example, and update strategy.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 --bind with a non-loopback address refuses to start unless multiuser mode is enabled plus explicit acknowledgment
- [ ] #2 Security headers are set on all responses
- [ ] #3 Deployment docs cover Tailscale, reverse proxy TLS, and VPS setups
- [ ] #4 Backup, restore, systemd, and update guidance for server operators is written
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
First deployment (decision-11, phase 1): the operator's server on bee, reachable at a public HTTPS address so a second household user can log in from anywhere. The guide covers a public address in front of nigel serve (reverse proxy with TLS, or a tunnel such as Cloudflare Tunnel or Tailscale Funnel), the trusted-host and origin settings that replace today's loopback-only rule, and that the server is never exposed before per-user login (32.2) is live.
<!-- SECTION:NOTES:END -->
