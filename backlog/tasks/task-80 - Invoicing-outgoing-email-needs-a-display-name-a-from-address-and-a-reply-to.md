---
id: TASK-80
title: 'Invoicing: outgoing email needs a display name, a from address and a reply-to'
status: To Do
assignee: []
created_date: '2026-08-10 21:48'
updated_date: '2026-08-11 20:03'
labels:
  - enhancement
  - invoicing
  - email
dependencies: []
documentation:
  - docs/superpowers/specs/2026-08-11-task-80-email-envelope-design.md
  - docs/superpowers/plans/2026-08-11-task-80-email-envelope.md
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
mailgun::message_fields takes from, to, subject and html, and that is the whole message. The From is a bare address, so an invoice arrives from billing@example.com with no business name beside it, and there is no Reply-To at all — a client hitting reply writes to whatever address happens to be sending.

Three things are wanted and they are not the same field: a display name (the business name the client sees), the from address (what the envelope and Mailgun use, constrained to the verified mailgun_domain), and a reply-to (which is unconstrained and is often a person rather than a billing alias).

There is an existing conflation to resolve. from_email is doing two unrelated jobs: it is the Mailgun From, and it is also the direct-deposit contact line printed on the published invoice page — cli/invoice.rs passes it in as the CONTACT value, and warns and prints a placeholder when it is unset. Those two want to diverge as soon as a reply-to exists, so the settings need to say which is which rather than reusing one key for both.

Note that mailgun_domain constrains the from address but not the reply-to, so validation differs between the two, and a display name containing a comma or a quote has to be encoded rather than concatenated into the header.

New settings need the full treatment the existing nine get: a key in settings.json, a matching NIGEL_* variable, resolution through invoicing_config with env winning, a name in the missing list that send_not_configured reports, and a row in docs/invoicing.md. Whether any of them are required or all optional is part of the decision — today from_email is required for send.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A sent invoice arrives with the business display name beside the from address
- [ ] #2 A reply-to address can be set and is honoured, independently of the from address
- [ ] #3 The published page contact line and the email from address are separately configurable — from_email no longer does both jobs implicitly
- [ ] #4 A display name containing a comma or a quote is encoded correctly in the header
- [ ] #5 The from address is validated against mailgun_domain; the reply-to is not
- [ ] #6 Any new key resolves from its NIGEL_* variable first, appears by name in send_not_configured when required and unset, and is documented in docs/invoicing.md
- [ ] #7 No response or log carries a configured value, only key names — the existing status contract holds
<!-- AC:END -->
