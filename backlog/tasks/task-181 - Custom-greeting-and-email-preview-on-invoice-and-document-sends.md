---
id: TASK-181
title: Custom greeting and email preview on invoice and document sends
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-18 21:36'
updated_date: '2026-11-18 21:41'
labels: []
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Let the operator open an outgoing invoice or document email with their own words. A default greeting is kept in Settings, one for invoices and one for documents. Each send can edit it, or clear it, before the email goes out. Scheduled invoice runs use the invoice default.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Settings stores an invoice email greeting and a document email greeting. Both are empty by default and editable in web Settings and through the settings API.
- [ ] #2 The invoice and document send dialogs in the web UI show a message box pre-filled with the matching default. What is in the box when Send is pressed is what goes out, and an empty box sends no greeting.
- [ ] #3 `nigel invoice send` and `nigel document send` accept --message <text>, which overrides the default. --no-message sends without one.
- [ ] #4 `invoice schedule run` uses the invoice default greeting.
- [ ] #5 The greeting is the opening paragraph of the plain-text email body. Documents keep their 'Hello {name},' salutation above it. Nothing else in either body changes.
- [ ] #6 The greeting is length-limited and trimmed, and it never appears on the published page or the PDF.
- [ ] #7 The greeting actually sent is stored with the invoice send or document version, and `show` displays it.
- [ ] #8 Tests cover default, override, cleared and scheduled sends for both kinds. docs/invoicing.md, docs/api.md and docs/commands.md are updated.
- [ ] #9 The send dialog shows a live preview of the email that will go out: recipient(s), subject, plain-text body including the greeting, and attachment names. It updates as the greeting is edited.
- [ ] #10 The preview comes from a server endpoint (POST /api/invoices/{id}/email-preview, POST /api/documents/{id}/email-preview, taking the draft greeting and recipients) that calls the same body and subject functions as the real send. It publishes, charges and emails nothing.
- [ ] #11 Links that only exist after a send, such as the invoice's Pay now URL and each document recipient's personal link, are shown as a clearly marked placeholder in the preview.
- [ ] #12 `nigel invoice send` and `nigel document send` print the email preview before their confirmation prompt.
<!-- AC:END -->
