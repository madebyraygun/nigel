---
id: TASK-142
title: Mark an invoice sent when it went out some other way
status: To Do
assignee: []
created_date: '2026-09-11 16:42'
labels:
  - invoicing
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Some clients do not take an invoice by email. Theirs arrives through a procurement portal: download the PDF, upload it there, and the invoice is genuinely out — but Nigel still calls it a draft, so it never ages, never appears as outstanding, and never reads overdue.

`nigel invoice send` is the only path from draft to sent, and it does four things: creates a Stripe payment link, renders HTML and PDF, uploads both to R2, and emails the client. Only the fourth is wrong for this case. What the other three should do is the question this task has to settle, and the answer is not obvious:

- **Publish to R2?** Probably yes — TASK-138 wants the page to offer the PDF, and a published page is where a payment link can live. But a client whose procurement system is the channel may never be given the URL, in which case publishing is dead weight.
- **Create a Stripe payment link?** Only if they can actually use it. A client paying through procurement is likely paying by ACH or check against a PO, so a payment link may be noise — or may be the thing that gets you paid faster. It should probably be the operator's call, not a fixed answer.
- **What is recorded?** A sent invoice that never went through the mailer needs to say so, otherwise the invoice history claims an email that does not exist.

The transition itself is small: send flips draft to sent, and a send that fails leaves it draft. This wants the same flip with the email step skipped, reachable from the CLI, the web invoice actions and the TUI.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An invoice can be marked sent without any email being sent
- [ ] #2 A marked-sent invoice ages, reads overdue past its due date, and counts as outstanding exactly as an emailed one does
- [ ] #3 Whether it publishes and whether it gets a payment link are decided, documented, and under the operator's control where that is the right answer
- [ ] #4 The invoice records that it was marked sent rather than emailed, so its history does not claim an email that never happened
- [ ] #5 An invoice already sent, void, or belonging to an archived client refuses the way send refuses
- [ ] #6 Available from the CLI, the web invoice actions and the TUI invoice detail
- [ ] #7 docs/invoicing.md covers when to use it and how it differs from send
<!-- AC:END -->
