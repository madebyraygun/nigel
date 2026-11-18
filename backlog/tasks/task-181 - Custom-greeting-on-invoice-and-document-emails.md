---
id: TASK-181
title: Custom greeting on invoice and document emails
status: To Do
assignee: []
created_date: '2026-11-18 21:36'
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
<!-- AC:END -->
