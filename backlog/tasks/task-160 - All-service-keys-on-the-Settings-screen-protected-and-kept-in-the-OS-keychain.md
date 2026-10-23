---
id: TASK-160
title: 'All service keys on the Settings screen, protected and kept in the OS keychain'
status: To Do
assignee: []
created_date: '2026-10-23 16:34'
labels:
  - settings
  - security
  - web
  - desktop
milestone: m-3
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The keys for Mailgun (or an SMTP server), Cloudflare R2 and the response Worker, and Stripe live in settings.json today and can only be set by editing the file or through NIGEL_* environment variables. Put them all on the Settings screen, and store the secret ones in the operating system's credential store instead of plain text.

Scope: Mail (Mailgun API key, domain, from/reply-to; or SMTP host, port, user, password as an alternative delivery), Publishing (R2 account, keys, buckets, public and documents base URLs, response Worker URL), Payments (Stripe secret key). Non-secret values may stay in settings.json; secrets go to the macOS Keychain, Windows Credential Manager or the Linux Secret Service. A headless server with no credential store (nigel serve on a VPS, SRV task) keeps env vars or a 0600 file as the documented fallback. Ties in with the Swift shell's Keychain work (TASK-144.13) and the store build's sandbox (TASK-150.1).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Settings has sections for Mail, Publishing and Payments covering every key the CLI reads today, each with validation matching the CLI's (including TASK-149's URL checks) and a test-connection action
- [ ] #2 Secret values are written to the OS credential store, never to settings.json; the UI shows them masked, never returns them over the API once saved, and replacing one requires typing the new value
- [ ] #3 Changing or viewing service keys requires re-entering the books password (or OS authentication in the desktop app) when the books are encrypted
- [ ] #4 Existing settings.json secrets migrate to the credential store on first run, with the plain-text copies removed, and env vars keep overriding as today
- [ ] #5 A headless server with no credential store falls back to env vars or a 0600 file, and says so in Settings and docs
- [ ] #6 docs/invoicing.md and docs/design-constraints.md describe where each secret lives
<!-- AC:END -->
