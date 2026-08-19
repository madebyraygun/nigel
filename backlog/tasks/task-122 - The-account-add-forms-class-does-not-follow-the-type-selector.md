---
id: TASK-122
title: The account add form's class does not follow the type selector
status: To Do
assignee: []
created_date: '2026-08-19 18:39'
updated_date: '2026-08-19 19:58'
labels:
  - tui
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
In the TUI account add form (TASK-9.1), the class field defaults to asset and stays there when the operator changes the account type — pick credit_card and the class still reads asset unless moved by hand. The defaulting the API applies (class from account_type when unspecified) is exactly the behaviour the form should mirror: until the operator touches the class field explicitly, it should track the type selection. This is the most likely source of a mis-classified account in day-to-day use.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Changing the type in the add form moves an untouched class field to that type's default class
- [ ] #2 A class the operator explicitly set is never overridden by a later type change
- [ ] #3 Covered by TUI form tests
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The web account create form has the same gap: EMPTY_ACCOUNT_FORM defaults class to asset and always sends it, so the server's account-type derivation never runs — a credit card added through the web stores as asset until the operator changes it. Fix both surfaces together: an untouched class control tracks the type selection; an explicitly chosen class is never overridden.
<!-- SECTION:NOTES:END -->
