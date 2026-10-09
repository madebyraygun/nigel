---
id: TASK-146
title: 'Invoice send history: when it was sent and what Mailgun answered'
status: To Do
assignee: []
created_date: '2026-09-30 18:11'
updated_date: '2026-10-07 18:44'
labels:
  - invoicing
milestone: m-3
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The invoice page says an invoice was sent but not when, how many times, or whether Mailgun accepted it. MailgunClient::send returns Result<()> and discards the response, so nothing is recorded today. Record each send attempt and show it on the invoice as a history, with a new entry every time the invoice is resent.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every send and resend records its timestamp, recipient(s), and the Mailgun response (HTTP status, message id, and message text) — including a rejected send
- [ ] #2 The web invoice detail shows the history, newest first, one line per send
- [ ] #3 Resending adds a new entry; earlier entries are kept
- [ ] #4 `nigel invoice show` prints the same history
- [ ] #5 The history is in the invoice API response and documented in docs/api.md
- [ ] #6 An invoice sent before this change shows no history rather than an error
<!-- AC:END -->
