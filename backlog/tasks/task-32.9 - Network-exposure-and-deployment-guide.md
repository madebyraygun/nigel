---
id: TASK-32.9
title: Network exposure and deployment guide
status: To Do
assignee: []
created_date: '2026-08-06 16:28'
updated_date: '2026-10-26 02:59'
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
- [ ] #1 Security headers are set on all responses
- [ ] #2 Deployment docs cover Tailscale, reverse proxy TLS, and VPS setups
- [ ] #3 Backup, restore, systemd, and update guidance for server operators is written
- [ ] #4 Shared mode on unencrypted books shows a clear warning recommending encryption in setup and Settings
- [ ] #5 nigel serve always binds loopback; a server_public_url setting (scheme + host, env NIGEL_SERVER_PUBLIC_URL) names the one non-loopback host accepted in shared mode, and is refused in local mode
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
First deployment (decision-11, phase 1): the operator's server on bee, reachable at a public HTTPS address so a second household user can log in from anywhere. The guide covers a public address in front of nigel serve (reverse proxy with TLS, or a tunnel such as Cloudflare Tunnel or Tailscale Funnel), the trusted-host and origin settings that replace today's loopback-only rule, and that the server is never exposed before per-user login (32.2) is live.

Design: multiuser phase 1 spec (PR #74). No --bind flag: tunnels and reverse proxies connect to loopback. Content-Security-Policy is its own task and must land before public exposure.

Design: docs/superpowers/specs/2026-10-23-multiuser-phase-1-design.md (approved, PR #74). Implementation sequence in §17.

From multiuser PR 2 review: the deployment guide must require the tunnel or reverse proxy to set Host to the public name (server_public_url's host). A request forwarded with Host: 127.0.0.1 is answered as a loopback request. Cloudflare Tunnel routes by hostname; for Caddy/nginx document that the site block matches only the public name. X-Forwarded-For is read as one trusted hop.
<!-- SECTION:NOTES:END -->
