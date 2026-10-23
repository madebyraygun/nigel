---
id: TASK-154.17
title: 'Settings: rewrite the help text in plain language'
status: To Do
assignee: []
created_date: '2026-10-23 16:25'
labels:
  - web
  - copy
milestone: m-3
dependencies: []
parent_task_id: TASK-154
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Settings help text reads like implementation notes. Examples from the Letterhead section:
- Payment instructions: 'Printed on both the invoice page and the PDF, or on neither. Leave it empty if you do not take bank transfers.'
- Logo: 'Up to 128 KiB. It is embedded in the PDF and inlined in the page; Gmail readers see your name in the email body and the logo on the attachment.'

Rewrite every description and help line on the Settings screen (Letterhead, Security, Location, Appearance, Updates) to say what the setting does for the user in short, plain sentences, dropping mechanism details (embedded, inlined, Gmail rendering) unless the user needs them to make a choice.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every section description and field help line on Settings is rewritten in short plain language that says what the setting does, not how it works
- [ ] #2 Limits users must know stay (e.g. the logo's file types and size), stated simply
- [ ] #3 Tests that pin the old wording are updated; docs that quote it match
<!-- AC:END -->
