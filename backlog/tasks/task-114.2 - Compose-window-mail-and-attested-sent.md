---
id: TASK-114.2
title: Compose-window mail and attested sent
status: To Do
assignee: []
created_date: '2026-08-17 13:27'
labels:
  - invoicing
  - desktop
dependencies: []
parent_task_id: TASK-114
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The mail half: open the operator's own client with the message ready, then record what Nigel can honestly claim.

- A compose `Mailer` that opens the default mail client pre-addressed (To the billing contact, cc the rest), subject and short body pre-filled, and the PDF attached where the platform allows: MAPI on Windows, `NSSharingService` on macOS, `xdg-email --attach` on Linux — reached natively from the desktop shell.
- Where only `mailto:` exists (plain CLI on a bare box), the fallback is honest: compose opens pre-addressed with a body short enough for URL limits, and the PDF is revealed in the file manager beside it so attaching is one drag. mailto cannot attach — the docs say so rather than pretending.
- The trace ends at a `compose opened` step, followed by an explicit "mark as sent?" confirmation on every surface. Only that confirmation writes `sent_at`, and the row records the delivery method (the acceptance-record precedent). Declining leaves a draft — including the frozen document version rule from epic 109: no attested send, no dangling version.
- Docs state plainly that a local `sent_at` is operator-attested, not observed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Compose opens with recipients, subject, body and — via the OS path — the PDF attached; the mailto fallback opens compose and reveals the PDF beside it
- [ ] #2 The local send trace ends at compose opened; sent_at is written only by the explicit confirmation and records the delivery method
- [ ] #3 Declining the confirmation leaves the invoice or document a draft with no dangling version
- [ ] #4 Platform compose paths are behind one seam with a fake per platform; no test opens a real mail client
<!-- AC:END -->
