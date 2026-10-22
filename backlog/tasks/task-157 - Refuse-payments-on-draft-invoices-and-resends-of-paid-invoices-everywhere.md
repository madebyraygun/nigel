---
id: TASK-157
title: Refuse payments on draft invoices and resends of paid invoices everywhere
status: To Do
assignee: []
created_date: '2026-10-21 20:42'
labels:
  - invoicing
  - cli
  - api
milestone: m-3
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel/pull/67'
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Product decision (PR #67 review): a draft invoice cannot take a payment, and a paid invoice is not resent. The refreshed web Invoices screen already enforces both. The server and CLI still allow them: the detail API sets canPay on a draft and canSend on a paid invoice, `nigel invoice pay` can drive an unsent draft to paid (ensure_editable documents this), and `nigel invoice send` resends a paid invoice. Make the core rules match, so every surface refuses the same way.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 nigel invoice pay on a draft is refused with a sentence saying to send the invoice first; the API's payment route refuses the same way and canPay is false on a draft
- [ ] #2 nigel invoice send on a paid invoice is refused with a sentence; the API refuses the same way and canSend is false on a paid invoice
- [ ] #3 Recurring-schedule autosend and any other internal path agree with the rules
- [ ] #4 Tests cover the CLI, the API flags and the refusals; docs/invoicing.md and docs/api.md describe the rules
<!-- AC:END -->
