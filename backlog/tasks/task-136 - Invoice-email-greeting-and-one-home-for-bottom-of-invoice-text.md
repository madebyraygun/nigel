---
id: TASK-136
title: 'Invoice email greeting, and one home for bottom-of-invoice text'
status: To Do
assignee: []
created_date: '2026-08-25 18:12'
labels: []
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Two related gaps in invoicing content fields.

1. Payment Instructions and Notes both render at the bottom of the invoice and read as duplicate functionality. Decide whether they collapse into one field or stay distinct — and if they stay, document what each is for so the choice is obvious when writing an invoice.

2. The email has no greeting. The plain-text body should be able to open with a line like "Hi Acme, your invoice is ready to view." The greeting could be generated automatically from the contacts on the invoice, or entered as free text. It must be optional: nothing configured means the email renders exactly as it does today, with no greeting block.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Decide merge-or-differentiate for Payment Instructions vs Notes; docs/invoicing.md states what each surviving field is for
- [ ] #2 An optional greeting can be set on an invoice email as free text, or generated from the invoice's contacts
- [ ] #3 With no greeting set, the email body is unchanged from today — no empty greeting block
- [ ] #4 Tests cover greeting present, greeting absent, and the auto-generated form
<!-- AC:END -->
